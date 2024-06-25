use crate::reader::{Cluster, Reader, THREADS};
use crate::{AppState, api};

use std::collections::VecDeque;
use std::path::Path;
use std::sync::OnceLock;
use std::{env, fs, ptr};
use std::sync::Arc;

use rand::Rng;
use tauri::{AppHandle, Manager};
use serde::{Serialize, Deserialize};
use tokio::sync::{mpsc, oneshot};
use futures::{future, stream, StreamExt};

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
  pub download_ids: Vec<u64>,
  pub created_at: u64,
  pub updated_at: u64,
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

  pub fn next_id(&mut self) -> u32 {
    let id = self.next_id;
    self.next_id += 1;
    id
  }

  pub fn extend_queue(&mut self, files: Vec<String>) {
    let mut queue = Vec::with_capacity(files.len());
    for file in files.iter() {
      let size = fs::metadata(&file).expect("failed to get file metadata").len();
      queue.push((file, size));
    }

    let handle = unsafe { self.rt.app_handle.as_ref().unwrap() };
    handle.emit_all("queue", queue).expect("failed to emit addFiles");
    
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

    let (tx, mut rx) = mpsc::channel::<usize>(10);
    let handle = unsafe { self.rt.app_handle.as_ref().unwrap() };
    
    tokio::spawn(async move {
      let mut bytes = 0;
      while let Some(read) = rx.recv().await {
        bytes += read;
        handle.emit_all("progress", bytes).expect("failed to emit uploadProgress");
      }
    });
    
    let mut reader = match Reader::new(&file, self.encryption_key, tx) {
      Some(reader) => reader,
      None => {
        log::error!("failed to open file: {}", file);
        self.upload();
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
    let uploaders = stream.for_each_concurrent(THREADS, move |rx| {
      let token2 = Arc::clone(&token2);
      let channel2 = Arc::clone(&channel2);

      async move {
        let (details, cluster, sender) = rx.await.expect("failed to receive upload details");
        let index = cluster.index as usize;
        api::upload(&details, cluster).await;

        let id = api::finalize(&token2, &channel2, &details).await;
        sender.send((id, index)).await.expect("failed to send finish signal");
      }
    });

    let futures = tokio::spawn(async move {
      let mut ids = vec![0; clusters];
      while let Some((id, index)) = rx.recv().await {
        ids[index] = id;
      }

      ids
    });

    let token = Arc::clone(&token);
    let channel = Arc::clone(&channel);
    let preuploads = tokio::spawn(async move {
      while let Some(cluster) = reader.next_cluster() {
        let details = api::preupload(&token, &channel, cluster.get_size()).await;
        let sender = senders.pop().unwrap();
        sender.send((details, cluster, tx.clone())).expect("failed to send upload details");
      }
    });

    let state = unsafe { &*self.rt.this };
    tokio::spawn(async move {
      let (ids, _, _) = future::join3(futures, uploaders, preuploads).await;
      let ids = ids.expect("failed to get message IDs");

      let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("failed to get timestamp")
        .as_secs();

      let mut state = state.write().await;
      let handle = unsafe { state.rt.app_handle.as_ref().unwrap() };
      handle.emit_all("uploaded", ()).expect("failed to emit upload");

      let file = File {
        id: state.next_id(),
        name: file,
        size: file_size,
        download_ids: ids,
        created_at: timestamp,
        updated_at: timestamp,
        crc32: 0,
      };

      state.files.push(file);
      
      state.write();
      state.upload();
    });
  }
}
