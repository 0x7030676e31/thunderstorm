use crate::reader::Reader;
use crate::AppState;

use std::collections::VecDeque;
use std::path::Path;
use std::sync::OnceLock;
use std::{env, fs, ptr};

use rand::Rng;
use tauri::AppHandle;
use serde::{Serialize, Deserialize};

fn path() -> &'static str {
  static PATH: OnceLock<String> = OnceLock::new();
  PATH.get_or_init(|| {
    match env::consts::OS {
      "linux" => format!("{}/.config/thunderstorm", env::var("HOME").expect("HOME not set")),
      "windows" => format!("{}/thunderstorm", env::var("LOCALAPPDATA").expect("LOCALAPPDATA not set")),
      "macos" => format!("{}/Library/Application Support/thunderstorm", env::var("HOME").expect("HOME not set")),
      _ => panic!("unsupported OS"),
    }
  })
}

#[derive(Debug)]
pub struct RtState {
  pub this: *const AppState,
  pub app_handle: *const AppHandle,
  pub queue: VecDeque<String>,
  pub is_uploading: bool,
}

impl Default for RtState {
  fn default() -> Self {
    Self {
      this: ptr::null(),
      app_handle: ptr::null(),
      queue: VecDeque::new(),
      is_uploading: false,
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
  pub name: String,
  pub size: u64,
  pub download_ids: Vec<String>,
  pub created_at: String,
  pub updated_at: String,
  pub crc32: u32,
}

impl State {
  pub fn new() -> Self {
    let app_data = path();
    if !Path::new(app_data).exists() {
      fs::create_dir_all(app_data).expect("failed to create app data directory");
    }

    let state_file = format!("{}/state.bin", app_data);
    if !Path::new(&state_file).exists() {
      return Self::default();
    }

    let file = match fs::read(&state_file) {
      Ok(file) => file,
      Err(e) => {
        log::error!("failed to read state file, launching with default state: {}", e);
        return Self::default();
      }
    };

    match bincode::deserialize(&file) {
      Ok(state) => state,
      Err(e) => {
        log::error!("failed to deserialize state file, launching with default state: {}", e);
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

    if let Err(e) = fs::write(&state_file, state) {
      log::error!("failed to write state file: {}", e);
    }
  }

  pub fn get_app_handle(&self) -> &AppHandle {
    unsafe {
      self.rt.app_handle.as_ref().unwrap()
    }
  }

  pub fn extend_queue(&mut self, files: Vec<String>) {
    self.rt.queue.extend(files);
    if !self.rt.is_uploading {
      self.upload();
    }
  }

  fn upload(&mut self) {
    let file = match self.rt.queue.pop_front() {
      Some(file) => file,
      None => {
        self.rt.is_uploading = false;
        return;
      }
    };

    log::debug!("Uploading file: {}", file);
    self.rt.is_uploading = true;

    let mut reader = match Reader::new(&file, self.encryption_key) {
      Some(reader) => reader,
      None => {
        log::error!("failed to open file: {}", file);
        self.upload();
        return;
      }
    };

    println!("file: {:?}", reader);
  }
}
