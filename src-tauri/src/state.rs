use crate::stream::{Reader, Cluster, UploadDetails, MAX_CONCURRENCY, CLUSTER_SIZE, upload, preupload, finalize};

use std::collections::VecDeque;
use std::sync::OnceLock;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs, cmp};

use futures::future;
use tauri::Manager;
use tokio::time;
use tokio::sync::{Mutex, mpsc, oneshot};
use serde::{Serialize, Deserialize};
use futures::{stream, StreamExt};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct File {
  pub name: String,
  pub size: u64,
  pub clusters: Vec<String>,
  pub created: u64,

  #[serde(skip)]
  pub being_deleted: bool,
}

#[derive(Default, Serialize, Deserialize)]
pub struct State {
  pub token: Option<String>,
  pub storage_channel: Option<String>,
  pub files: Vec<File>,

  #[serde(skip)]
  pub state: Option<Arc<Mutex<Self>>>,
  #[serde(skip)]
  pub upload_queue: VecDeque<String>,
  #[serde(skip)]
  pub is_uploading: bool,
  #[serde(skip)]
  pub app_handle: Option<tauri::AppHandle>,
}

fn path() -> &'static str {
  static PATH: OnceLock<String> = OnceLock::new();
  PATH.get_or_init(|| {
    match env::consts::OS {
      "linux" => format!("{}/.thunderstorm", env::var("HOME").unwrap()),
      "windows" => format!("{}\\Thunderstorm.json", env::var("APPDATA").unwrap()),
      _ => panic!("Unsupported OS"),
    }
  })
}

impl State {
  pub fn new() -> Self {
    if fs::metadata(path()).is_err() {
      return Self::default();
    }

    let data = match fs::read_to_string(path()) {
      Ok(data) => data,
      Err(e) => {
        log::error!("Error reading state from disk: {}", e);
        return Self::default();
      }
    };

    match serde_json::from_str(&data) {
      Ok(state) => state,
      Err(e) => {
        log::error!("Error parsing state from disk: {}", e);
        Self::default()
      }
    }
  }

  pub fn write(&self) {
    let data = serde_json::to_string(self).unwrap();
    match fs::write(path(), data) {
      Ok(_) => log::debug!("State written to disk"),
      Err(e) => log::error!("Error writing state to disk: {}", e),
    }
  }

  pub fn schedule_upload(&mut self, file_path: String) {
    self.upload_queue.push_back(file_path);
    if !self.is_uploading {
      self.upload();
    }
  }

  fn upload(&mut self) {
    let token = self.token.clone().unwrap();
    let storage_channel = self.storage_channel.clone().unwrap();
    let file = self.upload_queue.pop_front().unwrap();

    let (tx, mut rx) = mpsc::channel::<usize>(16);
    let reader = Reader::new(&file, tx);

    self.is_uploading = true;
    let handle = self.app_handle.as_ref().unwrap();
    handle.emit_all("uploading", reader.size).unwrap();

    let handle = handle.clone();
    tokio::spawn(async move {
      let mut uploaded = 0;
      while let Some(read) = rx.recv().await {
        uploaded += read;
        handle.emit_all("progress", uploaded).unwrap();
      }
    });

    type Sender = oneshot::Sender<(Vec<UploadDetails>, Cluster, mpsc::Sender<(Vec<UploadDetails>, usize)>)>;
    
    let mut senders: Vec<Sender> = Vec::with_capacity(reader.clusters);
    let mut receivers = Vec::with_capacity(reader.clusters);

    for _ in 0..reader.clusters {
      let (sender, receiver) = oneshot::channel();
      senders.push(sender);
      receivers.push(receiver);
    }

    let token_clone = token.clone();
    let stream = stream::iter(receivers).enumerate();
    let fut = stream.for_each_concurrent(MAX_CONCURRENCY, move |(idx, receiver)| {
      let token_clone = token_clone.clone();
      async move {
        let (details, cluster, tx) = receiver.await.unwrap();
        upload(&token_clone, &details, cluster).await;
        tx.send((details, idx)).await.unwrap();
      }
    });

    senders.reverse();
    let (tx, mut rx) = mpsc::channel::<(Vec<UploadDetails>, usize)>(MAX_CONCURRENCY);

    let clusters = reader.clusters;
    let size = reader.size;

    let mut idx = 0;
    let token_clone = token.clone();
    let channel_clone = storage_channel.clone();
    tokio::spawn(async move {
      while let Some(cluster) = reader.next_cluster(idx) {
        let size = cmp::min(reader.size - idx * CLUSTER_SIZE, CLUSTER_SIZE);
        match preupload(&token_clone, &channel_clone, idx, size).await {
          Ok(details) => {
            senders.pop().unwrap().send((details, cluster, tx.clone())).unwrap();
            idx += 1;
          },
          Err(retry_after) => {
            log::debug!("Preupload failed, retrying in {} seconds", retry_after);
            time::sleep(time::Duration::from_secs_f32(retry_after)).await;
            continue;
          }
        }
      }
    });

    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
      let mut ids = vec![String::new(); clusters];

      while let Some((details, idx)) = rx.recv().await {
        let message_id = loop {
          match finalize(&token_clone, &storage_channel, &details, idx).await {
            Ok(id) => break id,
            Err(retry_after) => {
              log::debug!("Finalize failed, retrying in {} seconds", retry_after);
              time::sleep(time::Duration::from_secs_f32(retry_after)).await;
              continue;
            }
          }
        };
        
        log::debug!("Cluster {} uploaded", idx);
        ids[idx] = message_id;
      }

      ids
    });

    let state = self.state.clone().unwrap();
    tokio::spawn(async move {
      log::info!("Starting upload of {}", file);
      
      let (ids, _) = future::join(handle, fut).await;
      let mut state = state.lock().await;

      let file_name = file.split("/").last().unwrap().to_string();
      let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;

      state.files.push(File {
        name: file_name,
        size: size as u64,
        clusters: ids.unwrap(),
        created: now,
        being_deleted: false,
      });

      state.write();
      
      let handle = state.app_handle.as_ref().unwrap();
      handle.emit_all("uploaded", ()).unwrap();
      
      state.is_uploading = false;
      if !state.upload_queue.is_empty() {
        state.upload();
      }
    });
  }
}
