use crate::api;
use crate::consts::{
    BUFFERS_PER_SLICE, BUFFER_SIZE, BYTES_PER_SLICE, CLUSTER_CAP, RAW_BUFFER_SIZE,
};
use crate::errors::DownloadError;

use std::cell::UnsafeCell;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;
use std::{cmp, io};

use aes_gcm::aead::AeadMutInPlace;
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use futures::{future, StreamExt};
use tokio::sync::{mpsc, Mutex};

struct Cipher(UnsafeCell<Aes256Gcm>);

unsafe impl Send for Cipher {}
unsafe impl Sync for Cipher {}

pub struct Writer {
    file: Arc<Mutex<File>>,
    cipher: Arc<Cipher>,
    tx: mpsc::Sender<usize>,
}

unsafe impl Send for Writer {}
unsafe impl Sync for Writer {}

impl Writer {
    pub fn new<T: AsRef<Path>>(
        path: T,
        key: &[u8; 32],
        sender: mpsc::Sender<usize>,
    ) -> io::Result<Self> {
        let key = Key::<Aes256Gcm>::from_slice(key);
        let cipher = Aes256Gcm::new(key);

        Ok(Self {
            file: Arc::new(Mutex::new(File::create(path)?)),
            #[allow(clippy::arc_with_non_send_sync)]
            cipher: Arc::new(Cipher(UnsafeCell::new(cipher))),
            tx: sender,
        })
    }

    pub fn cluster(&self, index: usize, download_urls: Vec<String>) -> Cluster {
        Cluster {
            file: self.file.clone(),
            cipher: self.cipher.clone(),
            index,
            urls: download_urls,
            sender: self.tx.clone(),
        }
    }
}

pub struct Cluster {
    file: Arc<Mutex<File>>,
    cipher: Arc<Cipher>,
    index: usize,
    urls: Vec<String>,
    sender: mpsc::Sender<usize>,
}

unsafe impl Send for Cluster {}
unsafe impl Sync for Cluster {}

impl Cluster {
    pub async fn download(&mut self) -> Result<(), DownloadError> {
        let futures = self.urls.iter().enumerate().map(|(index, url)| {
            download(
                self.file.clone(),
                self.cipher.clone(),
                url.clone(),
                self.index as u64,
                index as u64,
                self.sender.clone(),
            )
        });

        match future::try_join_all(futures).await {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

const BUFFER_SIZE_U: usize = BUFFER_SIZE as usize;
async fn download(
    file: Arc<Mutex<File>>,
    cipher: Arc<Cipher>,
    url: String,
    cluster: u64,
    slice: u64,
    tx: mpsc::Sender<usize>,
) -> Result<(), DownloadError> {
    let slice = cluster * CLUSTER_CAP + slice;

    let mut position = slice * BYTES_PER_SLICE;
    let mut buffer_index = slice * BUFFERS_PER_SLICE;
    let mut nonce = [0; 12];
    let mut buffer = Vec::with_capacity(BUFFER_SIZE_U);

    let mut stream = api::download(url).await?.bytes_stream();
    let cipher = unsafe { &mut *cipher.0.get() };

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(DownloadError::from)?;
        let mut cursor = 0;

        loop {
            let available = cmp::min(BUFFER_SIZE_U - buffer.len(), chunk.len() - cursor);
            buffer.extend_from_slice(&chunk[cursor..cursor + available]);

            if buffer.len() != BUFFER_SIZE_U {
                break;
            }

            nonce[4..].copy_from_slice(&buffer_index.to_be_bytes());
            let nonce = Nonce::from(nonce);

            cipher
                .decrypt_in_place(&nonce, b"", &mut buffer)
                .map_err(DownloadError::from)?;

            let mut file = file.lock().await;
            file.seek(SeekFrom::Start(position))
                .map_err(DownloadError::from)?;

            file.write_all(&buffer).map_err(DownloadError::from)?;
            drop(file);

            if let Err(err) = tx.send(buffer.len()).await {
                log::error!("Failed to send buffer size: {:?}", err);
            }

            buffer.clear();
            buffer_index += 1;
            cursor += available;
            position += RAW_BUFFER_SIZE;
        }
    }

    if !buffer.is_empty() {
        nonce[4..].copy_from_slice(&buffer_index.to_be_bytes());
        let nonce = Nonce::from(nonce);

        cipher
            .decrypt_in_place(&nonce, b"", &mut buffer)
            .map_err(DownloadError::from)?;

        let mut file = file.lock().await;
        file.seek(SeekFrom::Start(position))
            .map_err(DownloadError::from)?;

        file.write_all(&buffer).map_err(DownloadError::from)?;
        if let Err(err) = tx.send(buffer.len()).await {
            log::error!("Failed to send buffer size: {:?}", err);
        }

        drop(file);
    }

    Ok(())
}
