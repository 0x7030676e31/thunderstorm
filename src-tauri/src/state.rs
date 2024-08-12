use crate::reader::{Cluster, Reader, THREADS};
use crate::{api, AppState};

use std::collections::VecDeque;
use std::fmt::Display;
use std::path::Path;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Instant;
use std::{env, fs, io, ptr};

use futures::stream::{self, StreamExt, TryStreamExt};
use futures::{future, Future};
use rand::Rng;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};
use tauri::{AppHandle, Manager};
use tokio::sync::{mpsc, oneshot};

use tokio::select;
use tokio::task::{JoinError, JoinHandle};

fn path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| match env::consts::OS {
        "linux" => format!(
            "{}/.config/thunderstorm",
            env::var("HOME").expect("HOME not set")
        ),
        "windows" => format!(
            "{}/thunderstorm",
            env::var("LOCALAPPDATA").expect("LOCALAPPDATA not set")
        ),
        "macos" => format!(
            "{}/Library/Application Support/thunderstorm",
            env::var("HOME").expect("HOME not set")
        ),
        _ => panic!("unsupported OS"),
    })
}

pub trait Flatten<T, E1, E2>
where
    Self: Future<Output = Result<Result<T, E1>, E2>>,
    E1: Default,
{
    async fn flatten(self) -> Result<T, E1>;
}

impl<T, E> Flatten<T, E, JoinError> for JoinHandle<Result<T, E>>
where
    E: Default,
{
    async fn flatten(self) -> Result<T, E> {
        match self.await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(err)) => Err(err),
            Err(_) => Err(E::default()),
        }
    }
}

#[derive(Debug)]
pub enum UploadError {
    Io(io::Error),
    Reqwest(reqwest::Error),
    Unauthorized, // 401
    Forbidden,    // 403
    NotFound,     // 404
    Unknown((u16, String)),
    JoinError,
}

impl From<reqwest::Error> for UploadError {
    fn from(value: reqwest::Error) -> Self {
        Self::Reqwest(value)
    }
}

impl Serialize for UploadError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("UploadError", 2)?;
        match *self {
            UploadError::Io(ref err) => {
                state.serialize_field("type", "Io")?;
                state.serialize_field("message", &err.to_string())?;
            }
            UploadError::Reqwest(ref err) => 'reqwest: {
                state.serialize_field("type", "Reqwest")?;
                if err.is_connect() {
                    state.serialize_field("message", "Could not open a connection. Check your internet connection and try again.")?;
                    break 'reqwest;
                }

                if err.is_timeout() {
                    state.serialize_field(
                        "message",
                        "Connection timed out. Check your internet connection and try again.",
                    )?;
                    break 'reqwest;
                }

                state.serialize_field("message", &err.to_string())?;
            }
            UploadError::Unauthorized => {
                state.serialize_field("type", "Unauthorized")?;
                state.serialize_field("message", "")?;
            }
            UploadError::Forbidden => {
                state.serialize_field("type", "Forbidden")?;
                state.serialize_field("message", "")?;
            }
            UploadError::NotFound => {
                state.serialize_field("type", "NotFound")?;
                state.serialize_field("message", "")?;
            }
            UploadError::Unknown((code, ref message)) => {
                state.serialize_field("type", "")?;
                state.serialize_field("message", &format!("{} {}", code, message))?;
            }
            UploadError::JoinError => {
                state.serialize_field("type", "JoinError")?;
                state.serialize_field("message", "")?;
            }
        }
        state.end()
    }
}

impl Default for UploadError {
    fn default() -> Self {
        Self::JoinError
    }
}

impl Display for UploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "Io: {}", err),
            Self::Reqwest(err) => write!(f, "Reqwest: {}", err),
            Self::Unauthorized => write!(f, "Unauthorized"),
            Self::Forbidden => write!(f, "Forbidden"),
            Self::NotFound => write!(f, "Not Found"),
            Self::Unknown((status, message)) => write!(f, "Unknown: {} - {}", status, message),
            Self::JoinError => write!(f, "Join Error"),
        }
    }
}

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

    pub fn take(&mut self) -> Self {
        std::mem::replace(self, Self::Idle)
    }
}

#[derive(Debug)]
pub struct RtState {
    pub this: *const AppState,
    pub app_handle: *const AppHandle,
    pub queue: VecDeque<String>,
    pub job: Job,
}

impl Default for RtState {
    fn default() -> Self {
        Self {
            this: ptr::null(),
            app_handle: ptr::null(),
            queue: VecDeque::new(),
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
    pub files: Vec<File>,

    #[serde(with = "serde_bytes")]
    pub encryption_key: [u8; 32],
    #[serde(skip)]
    pub rt: RtState,
}

unsafe impl Send for State {}
unsafe impl Sync for State {}

impl Default for State {
    fn default() -> Self {
        let mut rng = rand::thread_rng();
        let mut key = [0; 32];
        rng.fill(&mut key[..]);

        Self {
            next_id: 1,
            channel_id: String::new(),
            guild_id: String::new(),
            token: String::new(),
            files: Vec::new(),
            encryption_key: key,
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

    pub fn extend_queue(&mut self, files: Vec<String>) {
        if !self.rt.job.is_upload_extendable() {
            log::warn!("Not uploading, ignoring files");
            return;
        }

        let mut queue = Vec::with_capacity(files.len());
        let handle = unsafe { self.rt.app_handle.as_ref().unwrap() };

        for file in files {
            let meta = match fs::metadata(&file) {
                Ok(file) => file,
                Err(err) => {
                    log::error!("failed to get file metadata: {}", err);
                    handle
                        .emit_all("upload_error", &UploadError::Io(err))
                        .expect("failed to emit fileError");

                    return;
                }
            };

            let len = meta.len();
            if len == 0 {
                log::warn!("Skipping empty file: {}", file);
                continue;
            }

            if meta.is_file() {
                queue.push((file.clone(), len));
            }
        }

        if queue.is_empty() {
            log::warn!("No files to upload");
            return;
        }

        handle
            .emit_all("extend_upload_queue", &queue)
            .expect("failed to emit addFiles");

        log::info!("Extending the queue with {} files", queue.len());
        self.rt
            .queue
            .extend(queue.into_iter().map(|(file, _)| file));

        if self.rt.job == Job::Idle {
            log::info!("Starting uploading {} files", self.rt.queue.len());
            self.upload();
        }
    }

    fn upload(&mut self) {
        let file = match self.rt.queue.pop_front() {
            Some(file) => file,
            None => {
                log::info!("No more files to upload, stopping");

                self.rt.job = Job::Idle;
                return;
            }
        };

        log::info!("Uploading file: {}", file);
        let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
        self.rt.job = Job::Upload { cancel_tx };

        let (tx, mut rx) = mpsc::channel::<usize>(10);
        let handle = unsafe { self.rt.app_handle.as_ref().unwrap() };

        tokio::spawn(async move {
            let mut bytes = 0;
            while let Some(read) = rx.recv().await {
                bytes += read;
                handle
                    .emit_all("upload_progress", bytes)
                    .expect("failed to emit uploadProgress");
            }
        });

        let mut reader = match Reader::new(&file, self.encryption_key, tx) {
            Ok(reader) => reader,
            Err(err) => {
                log::error!("failed to open file: {}", file);
                handle
                    .emit_all("upload_error", &UploadError::Io(err))
                    .expect("failed to emit fileError");

                self.rt.job = Job::Idle;
                self.rt.queue.clear();
                return;
            }
        };

        let clusters = reader.clusters as usize;
        let file_size = reader.file_size;

        // Channel ID, cluster index
        type Sender = (u64, usize);
        // Upload details, current cluster, finish sender
        type OneShot = (Vec<api::UploadDetailsInner>, Cluster, mpsc::Sender<Sender>);

        let (tx, mut rx) = mpsc::channel::<Sender>(THREADS);

        let mut senders = Vec::with_capacity(clusters);
        let mut receivers = Vec::with_capacity(clusters);

        for _ in 0..clusters {
            let (sender, receiver) = oneshot::channel::<OneShot>();
            senders.push(sender);
            receivers.push(receiver);
        }

        let token = Arc::new(self.token.clone());
        let channel = Arc::new(self.channel_id.clone());

        let token2 = token.clone();
        let channel2 = channel.clone();

        let stream = stream::iter(receivers);
        let uploaders = stream.map(Ok).try_for_each_concurrent(THREADS, move |rx| {
            let token2 = Arc::clone(&token2);
            let channel2 = Arc::clone(&channel2);

            async move {
                let (details, cluster, sender) = match rx.await {
                    Ok(result) => result,
                    Err(_) => return Ok(()), // TODO: comment why returning Ok(()) is actually ok
                };

                let index = cluster.index as usize;
                api::upload(&details, cluster).await?;

                let id = api::finalize(&token2, &channel2, &details).await?;
                sender
                    .send((id, index))
                    .await
                    .expect("failed to send finish signal");
                Ok::<(), UploadError>(())
            }
        });

        let futures = tokio::spawn(async move {
            let mut ids = vec![0; clusters];
            while let Some((id, index)) = rx.recv().await {
                ids[index] = id;
            }

            Ok::<_, UploadError>(ids)
        });

        let token = Arc::clone(&token);
        let channel = Arc::clone(&channel);
        let preuploads = tokio::spawn(async move {
            while let Some(cluster) = reader.next_cluster() {
                let details = api::preupload(&token, &channel, cluster.get_size()).await;
                let details = match details {
                    Ok(details) => details,
                    Err(err) => return Err(err),
                };

                let sender = senders.pop().unwrap();

                // When the receiver is dropped, uploading was canceled
                if sender.send((details, cluster, tx.clone())).is_err() {
                    break;
                }
            }

            Ok(())
        });

        let state = unsafe { &*self.rt.this };
        tokio::spawn(async move {
            let now = Instant::now();

            let futures = future::try_join3(
                Flatten::flatten(futures),
                uploaders,
                Flatten::flatten(preuploads),
            );
            let futures = select! {
                futures = futures => futures,
                _ = cancel_rx => {
                    log::debug!("Upload canceled");
                    return;
                }
            };

            let mut state = state.write().await;
            let handle = unsafe { state.rt.app_handle.as_ref().unwrap() };
            let ids = match futures {
                Ok((ids, _, _)) => ids,
                Err(err) => {
                    log::error!("Failed to upload file, reason: {}", err);

                    handle
                        .emit_all("upload_error", &err)
                        .expect("failed to emit upload error");

                    state.rt.queue.clear();
                    state.rt.job = Job::Idle;

                    return;
                }
            };

            let took = now.elapsed().as_secs_f64();
            log::info!("Uploaded {} cluster(s) in {:.2}s", clusters, took);

            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("failed to get timestamp")
                .as_secs();

            let file = File {
                id: state.next_id(),
                path: file,
                size: file_size,
                download_ids: ids,
                created_at: timestamp,
                updated_at: timestamp,
                crc32: 0,
            };

            handle
                .emit_all("file_uploaded", &file)
                .expect("failed to emit upload");

            state.files.push(file);

            state.write();
            state.upload();
        });
    }

    pub async fn cancel(&mut self) {
        match self.rt.job.take() {
            Job::Idle => {
                log::warn!("No job to cancel");
                return;
            }
            Job::Upload { cancel_tx } => {
                log::info!("Canceling upload job");

                self.rt.queue.clear();
                if cancel_tx.send(()).is_err() {
                    log::error!("failed to send cancel signal");
                }
            }
            Job::Download { cancel_tx } => {
                log::info!("Canceling download job");
                if cancel_tx.send(()).is_err() {
                    log::error!("failed to send cancel signal");
                }
            }
        }

        let handle = unsafe { self.rt.app_handle.as_ref().unwrap() };
        handle
            .emit_all("upload_canceled", ())
            .expect("failed to emit uploadCanceled");
    }
}
