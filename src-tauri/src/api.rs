use crate::reader::{Cluster, SLICE_SIZE};

use std::cmp;
use std::sync::Arc;

use futures::{future, stream};
use serde::Deserialize;
use reqwest::{Client, StatusCode};
use tokio::time;

#[derive(Debug, Deserialize)]
pub struct UploadDetailsInner {
  upload_url: String,
  upload_filename: String,
}

#[derive(Debug, Deserialize)]
struct UploadDetails {
  attachments: Vec<UploadDetailsInner>,
}

#[derive(Debug, Deserialize)]
struct RateLimit {
  retry_after: f32,
}

pub async fn preupload(token: &Arc<String>, channel: &Arc<String>, size: u64) -> Vec<UploadDetailsInner> {
  let slices = (size + SLICE_SIZE - 1) / SLICE_SIZE;
  let slices = (0..slices).map(|slice| {
    let size = cmp::min(SLICE_SIZE, size - slice * SLICE_SIZE);
    format!(r#"{{"file_size":{},"filename":"-","id":0,"is_clip":false}}"#, size)
  });

  let body = format!(r#"{{"files":[{}]}}"#, slices.collect::<Vec<_>>().join(","));
  let client = Client::new();

  loop {
    let req = client
      .post(&format!("https://discord.com/api/v9/channels/{}/attachments", channel))
      .header("Authorization", token.as_str())
      .header("Content-Type", "application/json")
      .body(body.clone())
      .send()
      .await
      .expect("Failed to send request");

    match req.status() {
      StatusCode::TOO_MANY_REQUESTS => {
        let rate_limit: RateLimit = req.json().await.expect("Failed to parse rate limit");
        time::sleep(time::Duration::from_secs_f32(rate_limit.retry_after)).await;
      },
      StatusCode::OK => {
        let details: UploadDetails = req.json().await.expect("Failed to parse upload details");
        return details.attachments;
      },
      _ => panic!("Failed to send request: {:?}", req.status()),
    }
  }
}

pub async fn upload(details: &Vec<UploadDetailsInner>, mut cluster: Cluster) {
  let client = Client::new();
  let futures = details.iter().map(|detail| {
    let stream = stream::iter(cluster.next_slice().unwrap());
    client
      .put(&detail.upload_url)
      .header("Content-Type", "application/octet-stream")
      .body(reqwest::Body::wrap_stream(stream))
      .send()
  });

  future::join_all(futures).await;
}

#[derive(Deserialize)]
struct Message {
  id: String,
}

pub async fn finalize(token: &Arc<String>, channel: &Arc<String>, details: &Vec<UploadDetailsInner>) -> u64 {
  let client = Client::new();
  let attachments = details.iter().map(|detail| {
    format!(r#"{{"filename":"-","uploaded_filename":"{}","id":"0"}}"#, detail.upload_filename)
  });

  let body = format!(r#"{{"attachments":[{}],"channel_id":"{}","content":"","type":0,"sticker_ids":[]}}"#, attachments.collect::<Vec<_>>().join(","), channel);
  let url = format!("https://discord.com/api/v9/channels/{}/messages", channel);

  loop {
    let req = client
      .post(&url)
      .header("Authorization", token.as_str())
      .header("Content-Type", "application/json")
      .body(body.clone())
      .send()
      .await
      .expect("Failed to send request");

    match req.status() {
      StatusCode::TOO_MANY_REQUESTS => {
        let rate_limit: RateLimit = req.json().await.expect("Failed to parse rate limit");
        time::sleep(time::Duration::from_secs_f32(rate_limit.retry_after)).await;
      },
      StatusCode::OK => {
        let message: Message = req.json().await.expect("Failed to parse message");
        return message.id.parse().expect("Failed to parse message ID");
      },
      _ => panic!("Failed to send request: {:?}", req.status()),
    }
  }
}