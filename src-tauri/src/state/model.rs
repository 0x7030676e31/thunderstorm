use crate::utils::path;
use crate::AppState;

use std::collections::VecDeque;
use std::path::Path;
use std::{fs, ptr};

use rand::Rng;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tokio::sync::oneshot;

#[derive(Debug)]
#[allow(dead_code)]
pub enum Job {
    Idle,
    Upload { cancel_tx: oneshot::Sender<()> },
    Download { cancel_tx: oneshot::Sender<()> },
}

impl Default for Job {
    fn default() -> Self {
        Self::Idle
    }
}

impl PartialEq for Job {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Idle, Self::Idle)
                | (Self::Upload { .. }, Self::Upload { .. })
                | (Self::Download { .. }, Self::Download { .. })
        )
    }
}

impl Job {
    pub fn is_upload_extendable(&self) -> bool {
        match self {
            Self::Idle => true,
            Self::Upload { .. } => true,
            Self::Download { .. } => false,
        }
    }

    pub fn is_download_extendable(&self) -> bool {
        match self {
            Self::Idle => true,
            Self::Upload { .. } => false,
            Self::Download { .. } => true,
        }
    }

    pub fn take(&mut self) -> Self {
        std::mem::replace(self, Self::Idle)
    }
}

#[derive(Debug)]
pub struct RtState {
    pub this: *const AppState,
    pub app_handle: *const AppHandle,
    pub upload_queue: VecDeque<String>,
    pub download_queue: VecDeque<u32>,
    pub job: Job,
}

impl Default for RtState {
    fn default() -> Self {
        Self {
            this: ptr::null(),
            app_handle: ptr::null(),
            upload_queue: VecDeque::new(),
            download_queue: VecDeque::new(),
            job: Job::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct State {
    pub next_id: u32,
    pub channel_id: String,
    pub guild_id: String,
    pub token: String,
    pub do_encrypt: bool,
    pub do_checksum: bool,
    pub files: Vec<File>,
    #[serde(skip)]
    pub rt: RtState,
}

unsafe impl Send for State {}
unsafe impl Sync for State {}

impl Default for State {
    fn default() -> Self {
        Self {
            next_id: 1,
            channel_id: String::new(),
            guild_id: String::new(),
            token: String::new(),
            do_encrypt: true,
            do_checksum: true,
            files: Vec::new(),
            rt: RtState::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct File {
    pub id: u32,
    pub path: String,
    pub size: u64,
    pub download_ids: Vec<u64>,
    pub created_at: u64,
    pub updated_at: u64,
    pub crc32: u32,
    #[serde(with = "serde_bytes")]
    pub encryption_key: Option<[u8; 32]>,
}

impl State {
    pub fn new() -> Self {
        let app_data = path();
        if !Path::new(app_data).exists() {
            log::info!("App data directory not found, creating");
            fs::create_dir_all(app_data).expect("failed to create app data directory");
        }

        let state_file = format!("{}/state.bin", app_data);
        if !Path::new(&state_file).exists() {
            log::info!("State file not found, launching with default state");
            return Self::default();
        }

        let file = match fs::read(&state_file) {
            Ok(file) => file,
            Err(e) => {
                log::error!(
                    "failed to read state file, launching with default state: {}",
                    e
                );
                return Self::default();
            }
        };

        match bincode::deserialize(&file) {
            Ok(state) => {
                log::info!("State file loaded, initializing...");
                state
            }
            Err(e) => {
                log::error!(
                    "failed to deserialize state file, launching with default state: {}",
                    e
                );
                Self::default()
            }
        }
    }

    pub fn write(&self) {
        let state_file = format!("{}/state.bin", path());
        let state = match bincode::serialize(&self) {
            Ok(state) => state,
            Err(e) => {
                log::error!("failed to serialize state, not writing state file: {}", e);
                return;
            }
        };

        match fs::write(&state_file, &state) {
            Ok(_) => log::debug!("State file written: {} bytes", state.len()),
            Err(err) => log::error!("failed to write state file: {}", err),
        }
    }

    pub fn next_id(&mut self) -> u32 {
        let id = self.next_id;
        log::debug!("Next ID: {}", id);

        self.next_id += 1;
        id
    }

    pub fn aes_key(&self) -> [u8; 32] {
        let mut rng = rand::thread_rng();
        let mut key = [0; 32];
        rng.fill(&mut key[..]);

        key
    }

    pub async fn cancel(&mut self) {
        match self.rt.job.take() {
            Job::Idle => {
                log::warn!("No job to cancel");
                return;
            }
            Job::Upload { cancel_tx } => {
                log::info!("Canceling upload job");

                self.rt.upload_queue.clear();
                if cancel_tx.send(()).is_err() {
                    log::error!("failed to send cancel signal");
                }
            }
            Job::Download { cancel_tx } => {
                log::info!("Canceling download job");

                self.rt.download_queue.clear();
                if cancel_tx.send(()).is_err() {
                    log::error!("failed to send cancel signal");
                }
            }
        }

        let handle = unsafe { self.rt.app_handle.as_ref().unwrap() };
        handle
            .emit_all("job_canceled", ())
            .expect("failed to emit job_canceled");
    }
}
