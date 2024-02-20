#![allow(dead_code)]
use std::io::{Read, Seek, SeekFrom};
use std::time::{SystemTime, UNIX_EPOCH};
use std::fs::{self, File};

use futures::future;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use futures::stream;
use tokio::sync::mpsc;

pub const MAX_CONCURRENCY: usize = 8;
pub const SLICE_SIZE: usize = 1024 * 1024 * 25;
pub const CLUSTER_CAP: usize = 10;
pub const CLUSTER_SIZE: usize = SLICE_SIZE * CLUSTER_CAP;
pub const BUFFER_SIZE: usize = 1024 * 1024 * 2;

const EPOCH: u64 = 1420070400000;

#[derive(Debug)]
pub struct Reader {
  file: File,
  slices: usize,
  pub tx: mpsc::Sender<usize>,
  pub clusters: usize,
  pub size: usize,
}

impl Reader {
  pub fn new(path: &str, tx: mpsc::Sender<usize>) -> Self {
    let file = File::open(path).unwrap();
    let size = fs::metadata(path).unwrap().len() as usize;

    let slices = (size + SLICE_SIZE - 1) / SLICE_SIZE;
    let clusters = (slices + CLUSTER_CAP - 1) / CLUSTER_CAP;

    Self {
      file,
      slices,
      tx,
      clusters,
      size,
    }
  }

  pub fn next_cluster(&self, idx: usize) -> Option<Cluster> {
    if idx == self.clusters {
      return None;
    }

    let tail = self.slices - idx * CLUSTER_CAP;
    let offset = idx * CLUSTER_SIZE;
    let cluster = Cluster {
      file: self.file.try_clone().unwrap(),
      size: self.size,
      offset,
      slices: std::cmp::min(CLUSTER_CAP, tail),
      current_slice: 0,
      tx: self.tx.clone(),
    };

    Some(cluster)
  }
}

#[derive(Debug)]
pub struct Cluster {
  file: File,
  size: usize,
  offset: usize,
  slices: usize,
  current_slice: usize,
  tx: mpsc::Sender<usize>,
}

impl Cluster {
  pub fn next_slice(&mut self) -> Option<Slice> {
    if self.current_slice == self.slices {
      return None;
    }

    let slice = Slice {
      file: self.file.try_clone().unwrap(),
      size: self.size,
      offset: self.offset + self.current_slice * SLICE_SIZE,
      read: 0,
      tx: self.tx.clone(),
    };

    self.current_slice += 1;
    Some(slice)
  }
}

pub struct Slice {
  file: File,
  size: usize,
  offset: usize,
  read: usize,
  tx: mpsc::Sender<usize>,
}

impl Iterator for Slice {
  type Item = Result<Vec<u8>, std::io::Error>;

  fn next(&mut self) -> Option<Self::Item> {
    if self.read == SLICE_SIZE {
      return None;
    }

    let size = std::cmp::min(SLICE_SIZE - self.read, BUFFER_SIZE);
    let size = std::cmp::min(size, self.size - self.read);
    let mut buffer = vec![0; size];

    self.file.seek(SeekFrom::Start((self.offset + self.read) as u64)).unwrap();
    let read = self.file.read(&mut buffer).unwrap();

    if read == 0 {
      return None;
    }

    self.read += read;
    let tx = self.tx.clone();
    tokio::spawn(async move {
      tx.send(read).await.unwrap();
    });

    Some(Ok(buffer))
  }
}

#[derive(Debug, Deserialize)]
pub struct UploadDetails {
  upload_url: String,
  upload_filename: String,
}

#[derive(Debug, Deserialize)]
struct UploadDetailsResp {
  attachments: Vec<UploadDetails>,
}

#[derive(Debug, Deserialize)]
struct RateLimit {
  retry_after: f32,
}

pub async fn upload(auth: &str, details: &Vec<UploadDetails>, mut cluster: Cluster) {
  let client = Client::new();
  
  let futures = details.iter().map(|detail| {
    let stream = stream::iter(cluster.next_slice().unwrap());
    client.put(&detail.upload_url)
      .header("Content-Type", "application/octet-stream")
      .header("Authorization", auth)
      .body(reqwest::Body::wrap_stream(stream))
      .send()
  });

  future::join_all(futures).await;
}

fn get_nonce() -> String {
  let now = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap()
    .as_millis() as u64;

  let nonce = (now - EPOCH) << 22;
  nonce.to_string()
}

#[derive(Debug, Deserialize)]
struct Message {
  id: String,
}

pub async fn finalize(auth: &str, channel: &str, attachments: &[UploadDetails], idx: usize) -> Result<String, f32> {
  let client = Client::new();
  
  let offset = idx * CLUSTER_CAP;
  let attachments = attachments.iter().enumerate().map(|(i, attachment)| {
    let name = offset + i;
    format!(r#"{{"filename":"{}","uploaded_filename":"{}","id":"{}"}}"#, name, attachment.upload_filename, i)
  });

  let body = format!(r#"{{"attachments":[{}],"channel_id":"{}","nonce":"{}","content":"","type":0,"sticker_ids":[]}}"#, attachments.collect::<Vec<_>>().join(","), channel, get_nonce());
  
  let req = client.post(format!("https://discord.com/api/v9/channels/{}/messages", channel))
    .header("Content-Type", "application/json")
    .header("Authorization", auth)
    .body(body)
    .send()
    .await
    .unwrap();

  match req.status() {
    StatusCode::TOO_MANY_REQUESTS => {
      let rate_limit: RateLimit = req.json().await.unwrap();
      Err(rate_limit.retry_after)
    }
    StatusCode::OK => {
      let message: Message = req.json().await.unwrap();
      Ok(message.id)
    }
    _ => {
      panic!("Unexpected status code: {}", req.status());
    }
  }
}

pub async fn preupload(auth: &str, channel: &str, idx: usize, size: usize) -> Result<Vec<UploadDetails>, f32> {
  let slices = (size + SLICE_SIZE - 1) / SLICE_SIZE;
  let slices = (0..slices).map(|i| {
    let size = std::cmp::min(size - i * SLICE_SIZE, SLICE_SIZE);
    let name = idx * slices + i;
  
    format!(r#"{{"file_size":{},"filename":"{}","id":"0","is_clip":false}}"#, size, name)
  });

  let body = format!(r#"{{"files":[{}]}}"#, slices.collect::<Vec<_>>().join(","));
  
  let client = Client::new();
  let req = client.post(format!("https://discord.com/api/v9/channels/{}/attachments", channel))
    .header("Content-Type", "application/json")
    .header("Authorization", auth)
    .body(body)
    .send()
    .await
    .unwrap();

  match req.status() {
    StatusCode::TOO_MANY_REQUESTS => {
      let rate_limit: RateLimit = req.json().await.unwrap();
      Err(rate_limit.retry_after)
    }
    StatusCode::OK => {
      let details: UploadDetailsResp = req.json().await.unwrap();
      Ok(details.attachments)
    }
    _ => {
      panic!("Unexpected status code: {}", req.status());
    }
  }
}

