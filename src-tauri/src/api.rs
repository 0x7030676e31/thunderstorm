use crate::errors::DownloadError;
use crate::errors::UploadError;
use crate::io::consts::SLICE_SIZE;
use crate::io::Cluster;

use std::cmp;
use std::sync::Arc;
use std::time::Duration;

use futures::{future, stream};
use reqwest::{Client, Response, StatusCode};
use serde::Deserialize;
use tokio::time;

const READ_TIMEOUT: Duration = Duration::from_secs(20);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

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
                log::warn!(
                    "Resource preupload rate limited, retrying in {} seconds",
                    rate_limit.retry_after
                );

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

pub async fn secure_upload<T>(
    details: &[UploadDetailsInner],
    mut cluster: T,
) -> Result<(), UploadError>
where
    T: Cluster + Send + Sync,
    <T as Cluster>::Iter: Send + Sync + 'static,
{
    let client = Client::builder()
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
                log::warn!(
                    "Resource upload rate limited, retrying in {} seconds",
                    rate_limit.retry_after
                );

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

#[derive(Deserialize)]
pub struct Attachment {
    pub url: String,
}

pub trait Take {
    fn take(&mut self) -> Vec<String>;
}

impl Take for Vec<Attachment> {
    fn take(&mut self) -> Vec<String> {
        self.drain(..).map(|attachment| attachment.url).collect()
    }
}

#[derive(Deserialize)]
pub struct MessageFull {
    pub attachments: Vec<Attachment>,
    pub id: String,
}

pub async fn fetch_messages(
    token: &Arc<String>,
    channel: &Arc<String>,
    id: u64,
    limit: usize,
) -> Result<Vec<MessageFull>, DownloadError> {
    let client = Client::builder()
        .read_timeout(READ_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .map_err(DownloadError::from)?;

    let url = format!(
        "https://discord.com/api/v9/channels/{}/messages?limit={}&around={}",
        channel, limit, id
    );

    loop {
        let req = client
            .get(&url)
            .header("Authorization", token.as_str())
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(DownloadError::from)?;

        let status = req.status();
        match status {
            StatusCode::UNAUTHORIZED => return Err(DownloadError::Unauthorized),
            StatusCode::FORBIDDEN => return Err(DownloadError::Forbidden),
            StatusCode::NOT_FOUND => return Err(DownloadError::NotFound),
            StatusCode::TOO_MANY_REQUESTS => {
                let rate_limit: RateLimit = req.json().await.map_err(DownloadError::from)?;
                log::warn!(
                    "Resource metadata download rate limited, retrying in {} seconds",
                    rate_limit.retry_after
                );

                time::sleep(time::Duration::from_secs_f32(rate_limit.retry_after)).await;
            }
            StatusCode::OK => {
                let messages: Vec<MessageFull> = req.json().await.map_err(DownloadError::from)?;

                return Ok(messages);
            }
            _ => {
                return Err(DownloadError::Unknown((
                    status.as_u16(),
                    status.to_string(),
                )))
            }
        }
    }
}

pub async fn download(url: String) -> Result<Response, DownloadError> {
    let client = Client::builder()
        .read_timeout(READ_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .map_err(DownloadError::from)?;

    loop {
        let req = client.get(&url).send().await.map_err(DownloadError::from)?;

        let status = req.status();
        match status {
            StatusCode::UNAUTHORIZED => return Err(DownloadError::Unauthorized),
            StatusCode::FORBIDDEN => return Err(DownloadError::Forbidden),
            StatusCode::NOT_FOUND => return Err(DownloadError::NotFound),
            StatusCode::TOO_MANY_REQUESTS => {
                let rate_limit: RateLimit = req.json().await.map_err(DownloadError::from)?;
                log::warn!(
                    "Resource download rate limited, retrying in {} seconds",
                    rate_limit.retry_after
                );

                time::sleep(time::Duration::from_secs_f32(rate_limit.retry_after)).await;
            }
            StatusCode::OK => {
                return Ok(req);
            }
            _ => {
                return Err(DownloadError::Unknown((
                    status.as_u16(),
                    status.to_string(),
                )))
            }
        }
    }
}
