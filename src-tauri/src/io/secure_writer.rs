use super::consts::*;
use super::Cipher;
use crate::api;
use crate::errors::DownloadError;

use std::cell::UnsafeCell;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;
use std::{cmp, io};

use aes_gcm::aead::AeadMutInPlace;
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use crc32fast::Hasher;
use futures::{future, StreamExt};
use tokio::sync::{mpsc, Mutex};

type CrcSender = Option<mpsc::Sender<(u64, Hasher)>>;

pub struct SecureWriter {
    file: Arc<Mutex<File>>,
    cipher: Arc<Cipher>,
    write_tx: mpsc::Sender<usize>,
    crc_tx: CrcSender,
}

unsafe impl Send for SecureWriter {}
unsafe impl Sync for SecureWriter {}

impl SecureWriter {
    pub fn new<T: AsRef<Path>>(
        path: T,
        key: &[u8; 32],
        write_sender: mpsc::Sender<usize>,
        crc_sender: CrcSender,
    ) -> io::Result<Self> {
        let key = Key::<Aes256Gcm>::from_slice(key);
        let cipher = Aes256Gcm::new(key);

        Ok(Self {
            file: Arc::new(Mutex::new(File::create(path)?)),
            #[allow(clippy::arc_with_non_send_sync)]
            cipher: Arc::new(Cipher(UnsafeCell::new(cipher))),
            write_tx: write_sender,
            crc_tx: crc_sender,
        })
    }

    pub fn cluster(&self, index: usize, download_urls: Vec<String>) -> SecureClusterW {
        SecureClusterW {
            file: self.file.clone(),
            cipher: self.cipher.clone(),
            index,
            urls: download_urls,
            write_sender: self.write_tx.clone(),
            crc_sender: self.crc_tx.clone(),
        }
    }
}

pub struct SecureClusterW {
    file: Arc<Mutex<File>>,
    cipher: Arc<Cipher>,
    index: usize,
    urls: Vec<String>,
    write_sender: mpsc::Sender<usize>,
    crc_sender: CrcSender,
}

unsafe impl Send for SecureClusterW {}
unsafe impl Sync for SecureClusterW {}

impl SecureClusterW {
    pub async fn download(&mut self) -> Result<(), DownloadError> {
        let futures = self.urls.iter().enumerate().map(|(index, url)| {
            download(
                self.file.clone(),
                self.cipher.clone(),
                url.clone(),
                self.index as u64,
                index as u64,
                self.write_sender.clone(),
                self.crc_sender.clone(),
            )
        });

        match future::try_join_all(futures).await {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

async fn download(
    file: Arc<Mutex<File>>,
    cipher: Arc<Cipher>,
    url: String,
    cluster: u64,
    slice: u64,
    write_tx: mpsc::Sender<usize>,
    crc_tx: CrcSender,
) -> Result<(), DownloadError> {
    let slice = cluster * CLUSTER_CAP + slice;

    let mut position = slice * BYTES_PER_SLICE;
    let mut buffer_index = slice * BUFFERS_PER_SLICE;
    let mut nonce = [0; 12];
    let mut buffer = Vec::with_capacity(BUFFER_SIZE_U);

    let mut stream = api::download(url).await?.bytes_stream();
    let cipher = unsafe { &mut *cipher.0.get() };

    let mut hasher = crc_tx.is_some().then(Hasher::new);

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

            if let Some(hasher) = &mut hasher {
                hasher.update(&buffer);
            }

            if let Err(err) = write_tx.send(buffer.len()).await {
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
        if let Err(err) = write_tx.send(buffer.len()).await {
            log::error!("Failed to send buffer size: {:?}", err);
        }

        drop(file);

        if let Some(hasher) = &mut hasher {
            hasher.update(&buffer);
        }
    }

    if let (Some(hasher), Some(crc_tx)) = (hasher, crc_tx) {
        if let Err(err) = crc_tx.send((slice, hasher)).await {
            log::error!("Failed to send crc: {:?}", err);
        }
    }

    Ok(())
}
