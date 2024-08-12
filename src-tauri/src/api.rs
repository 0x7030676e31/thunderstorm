use crate::reader::{Cluster, SLICE_SIZE};
use crate::state::UploadError;

use std::cmp;
use std::sync::Arc;
use std::time::Duration;

use futures::{future, stream};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tokio::time;

const READ_TIMEOUT: Duration = Duration::from_secs(20);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

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

pub async fn preupload<'a>(
    token: &Arc<String>,
    channel: &Arc<String>,
    size: u64,
) -> Result<Vec<UploadDetailsInner>, UploadError> {
    let slices = (size + SLICE_SIZE - 1) / SLICE_SIZE;
    let slices = (0..slices).map(|slice| {
        let size = cmp::min(SLICE_SIZE, size - slice * SLICE_SIZE);
        format!(
            r#"{{"file_size":{},"filename":"-","id":0,"is_clip":false}}"#,
            size
        )
    });

    let body = format!(r#"{{"files":[{}]}}"#, slices.collect::<Vec<_>>().join(","));
    let client = Client::builder()
        .read_timeout(READ_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .map_err(UploadError::from)?;

    loop {
        let req = client
            .post(&format!(
                "https://discord.com/api/v9/channels/{}/attachments",
                channel
            ))
            .header("Authorization", token.as_str())
            .header("Content-Type", "application/json")
            .body(body.clone())
            .send()
            .await
            .map_err(UploadError::from)?;

        let status = req.status();
        match status {
            StatusCode::UNAUTHORIZED => return Err(UploadError::Unauthorized),
            StatusCode::FORBIDDEN => return Err(UploadError::Forbidden),
            StatusCode::NOT_FOUND => return Err(UploadError::NotFound),
            StatusCode::TOO_MANY_REQUESTS => {
                let rate_limit: RateLimit = req.json().await.map_err(UploadError::from)?;

                time::sleep(time::Duration::from_secs_f32(rate_limit.retry_after)).await;
            }
            StatusCode::OK => {
                let details: UploadDetails = req.json().await.map_err(UploadError::from)?;

                return Ok(details.attachments);
            }
            _ => return Err(UploadError::Unknown((status.as_u16(), status.to_string()))),
        }
    }
}

pub async fn upload(
    details: &[UploadDetailsInner],
    mut cluster: Cluster,
) -> Result<(), UploadError> {
    let client = Client::builder()
        .read_timeout(READ_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .map_err(UploadError::from)?;

    let futures = details.iter().map(|detail| {
        let stream = stream::iter(cluster.next_slice().unwrap());
        client
            .put(&detail.upload_url)
            .header("Content-Type", "application/octet-stream")
            .body(reqwest::Body::wrap_stream(stream))
            .send()
    });

    let responses = future::join_all(futures).await;
    for response in responses {
        let response = response.map_err(UploadError::Reqwest)?;
        let status = response.status();
        if !status.is_success() {
            return Err(UploadError::Unknown((status.as_u16(), status.to_string())));
        }
    }

    Ok(())
}

#[derive(Deserialize)]
struct Message {
    id: String,
}

pub async fn finalize(
    token: &Arc<String>,
    channel: &Arc<String>,
    details: &[UploadDetailsInner],
) -> Result<u64, UploadError> {
    let client = Client::builder()
        .read_timeout(READ_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .map_err(UploadError::from)?;

    let attachments = details.iter().map(|detail| {
        format!(
            r#"{{"filename":"-","uploaded_filename":"{}","id":"0"}}"#,
            detail.upload_filename
        )
    });

    let body = format!(
        r#"{{"attachments":[{}],"channel_id":"{}","content":"","type":0,"sticker_ids":[]}}"#,
        attachments.collect::<Vec<_>>().join(","),
        channel
    );
    let url = format!("https://discord.com/api/v9/channels/{}/messages", channel);

    loop {
        let req = client
            .post(&url)
            .header("Authorization", token.as_str())
            .header("Content-Type", "application/json")
            .body(body.clone())
            .send()
            .await
            .map_err(UploadError::from)?;

        let status = req.status();
        match status {
            StatusCode::UNAUTHORIZED => return Err(UploadError::Unauthorized),
            StatusCode::FORBIDDEN => return Err(UploadError::Forbidden),
            StatusCode::NOT_FOUND => return Err(UploadError::NotFound),
            StatusCode::TOO_MANY_REQUESTS => {
                let rate_limit: RateLimit = req.json().await.map_err(UploadError::from)?;

                time::sleep(time::Duration::from_secs_f32(rate_limit.retry_after)).await;
            }
            StatusCode::OK => {
                let message: Message = req.json().await.map_err(UploadError::from)?;

                // This should never fail
                return Ok(message.id.parse().expect("Failed to parse message ID"));
            }
            _ => return Err(UploadError::Unknown((status.as_u16(), status.to_string()))),
        }
    }
}
