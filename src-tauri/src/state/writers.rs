use super::errors::DownloadError;
use super::model::{Job, State};
use crate::api::{self, Take};
use crate::io::consts::{BYTES_PER_SLICE, DOWNLOAD_THREADS, SLICE_SIZE};
use crate::io::secure_writer::{SecureClusterW, SecureWriter};
use crate::io::writer::{InsecureClusterW, InsecureWriter};
use crate::utils::{download_target, Flatten};

use std::sync::Arc;
use std::time::Instant;
use std::{cmp, fs};

use crc32fast::Hasher;
use futures::future;
use futures::stream::{self, StreamExt, TryStreamExt};
use tauri::Manager;
use tokio::select;
use tokio::sync::{mpsc, oneshot};

impl State {
    pub fn extend_download_queue(&mut self, files: Vec<u32>) {
        if !self.rt.job.is_download_extendable() {
            log::warn!("Not downloading, ignoring files");
            return;
        }

        let mut queue = Vec::with_capacity(files.len());
        let mut pairs = Vec::with_capacity(files.len());
        let handle = unsafe { self.rt.app_handle.as_ref().unwrap() };

        for id in files {
            if let Some(file) = self.files.iter().find(|file| file.id == id) {
                pairs.push((&file.path, file.size));
                queue.push(file.id);
            }
        }

        if queue.is_empty() {
            log::warn!("No files to download");
            return;
        }

        handle
            .emit_all("extend_download_queue", &pairs)
            .expect("failed to emit extend_download_queue");

        log::info!("Extending the queue with {} files", queue.len());
        self.rt.download_queue.extend(queue);

        if self.rt.job == Job::Idle {
            log::info!(
                "Starting downloading {} files",
                self.rt.download_queue.len()
            );
            self.download();
        }
    }

    fn download(&mut self) {
        let id = match self.rt.download_queue.pop_front() {
            Some(id) => id,
            None => {
                log::info!("No more files to download, stopping");

                self.rt.job = Job::Idle;
                return;
            }
        };

        log::info!("Attempting to download file: {}", id);
        let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
        self.rt.job = Job::Download { cancel_tx };

        let encryption_key = self
            .files
            .iter()
            .find(|file| file.id == id)
            .map(|file| file.encryption_key)
            .flatten();

        if let Some(key) = encryption_key {
            self.download_secure(id, cancel_rx, key);
        } else {
            self.download_insecure(id, cancel_rx);
        }
    }

    fn download_secure(&mut self, id: u32, cancel_rx: oneshot::Receiver<()>, key: [u8; 32]) {
        let handle = unsafe { self.rt.app_handle.as_ref().unwrap() };
        let file = match self.files.iter().find(|file| file.id == id) {
            Some(file) => file,
            None => {
                log::error!("File not found: {}", id);
                handle
                    .emit_all("download_error", &DownloadError::NotFoundLocal)
                    .expect("failed to emit download_error");

                self.rt.job = Job::Idle;
                self.rt.download_queue.clear();
                return;
            }
        };

        let (tx, mut rx) = mpsc::channel::<usize>(10);
        tokio::spawn(async move {
            let mut bytes = 0;
            while let Some(read) = rx.recv().await {
                bytes += read;
                handle
                    .emit_all("download_progress", bytes)
                    .expect("failed to emit download_progress");
            }
        });

        let target = download_target(&file.path);
        let token = Arc::new(self.token.clone());
        let channel = Arc::new(self.channel_id.clone());
        let cluster_count = file.download_ids.len();
        let mut ids = file.download_ids.clone();

        let (crc_tx, crc_rx) = self
            .do_checksum
            .then(|| mpsc::channel::<(u64, Hasher)>(4))
            .map_or_else(|| (None, None), |(tx, rx)| (Some(tx), Some(rx)));

        let slices = file.size / BYTES_PER_SLICE;
        let crc_handle = tokio::spawn(async move {
            let mut rx = match crc_rx {
                Some(rx) => rx,
                None => return Ok(None),
            };

            let mut hashers = vec![unsafe { std::mem::zeroed() }; slices as usize];
            while let Some((idx, hasher)) = rx.recv().await {
                hashers.insert(idx as usize, hasher);
            }

            let mut hasher = Hasher::new();
            for other in hashers {
                hasher.combine(&other);
            }

            Ok(Some(hasher.finalize()))
        });

        let writer = match SecureWriter::new(&target, &key, tx, crc_tx) {
            Ok(writer) => writer,
            Err(err) => {
                log::error!("failed to open file: {}", target);
                handle
                    .emit_all("download_error", &DownloadError::Io(err))
                    .expect("failed to emit download_error");

                self.rt.job = Job::Idle;
                self.rt.download_queue.clear();
                return;
            }
        };

        log::info!("Downloading file: {}", target);
        let mut senders = Vec::with_capacity(ids.len());
        let mut receivers = Vec::with_capacity(ids.len());

        for _ in 0..ids.len() {
            let (sender, receiver) = oneshot::channel::<SecureClusterW>();
            senders.push(sender);
            receivers.push(receiver);
        }

        let stream = stream::iter(receivers);
        let downloaders =
            stream
                .map(Ok)
                .try_for_each_concurrent(DOWNLOAD_THREADS, move |rx| async move {
                    let mut cluster = match rx.await {
                        Ok(cluster) => cluster,
                        Err(_) => return Ok(()), // TODO: comment why returning Ok(()) is actually ok
                    };

                    cluster.download().await
                });

        let future = tokio::spawn(async move {
            let message_count = cmp::min(ids.len() * 2, 100);
            let mut set = ids.clone();

            'outer: while let Some(id) = ids.first() {
                let mut messages =
                    match api::fetch_messages(&token, &channel, *id, message_count).await {
                        Ok(messages) => messages,
                        Err(err) => {
                            log::error!("failed to fetch messages: {}", err);
                            return Err(err);
                        }
                    };

                let id = *id;
                let mut has_found = false;
                for message in messages.iter_mut() {
                    let message_id = message
                        .id
                        .parse::<u64>()
                        .expect("failed to parse message ID");

                    if message_id == id {
                        has_found = true;
                    }

                    if let Some((idx, id)) = set
                        .iter_mut()
                        .enumerate()
                        .find(|(_, id)| **id == message_id)
                    {
                        ids.retain(|id| *id != message_id);
                        let attachments = message.attachments.take();
                        let cluster = writer.cluster(idx, attachments);
                        *id = 0;

                        let sender = senders.pop().unwrap();
                        if sender.send(cluster).is_err() {
                            break 'outer;
                        }
                    }
                }

                if !has_found {
                    log::warn!("Message not found: {}", id);
                    return Err(DownloadError::NotFoundRemote);
                }
            }

            Ok(())
        });

        let state = unsafe { &*self.rt.this };
        tokio::spawn(async move {
            let now = Instant::now();

            let futures = future::try_join3(
                downloaders,
                Flatten::flatten(future),
                Flatten::flatten(crc_handle),
            );
            let futures = select! {
                futures = futures => futures,
                _ = cancel_rx => {
                    log::debug!("Download canceled");
                    if let Err(err) = fs::remove_file(&target) {
                        log::error!("failed to remove file: {}", err);
                    }

                    return;
                }
            };

            let took = now.elapsed().as_secs_f64();
            log::info!("Downloaded {} cluster(s) in {:.2}s", cluster_count, took);

            let mut state = state.write().await;
            let handle = unsafe { state.rt.app_handle.as_ref().unwrap() };
            let crc = match futures {
                Ok((_, _, crc)) => crc,
                Err(err) => {
                    log::error!("Failed to download file, reason: {}", err);

                    handle
                        .emit_all("download_error", &err)
                        .expect("failed to emit download_error");

                    state.rt.download_queue.clear();
                    state.rt.job = Job::Idle;

                    return;
                }
            };

            if let Some(file) = state.files.iter().find(|file| file.id == id)
                && crc.is_some_and(|crc| crc != file.crc32)
            {
                log::warn!("CRC32 mismatch: {:x} != {:x}", crc.unwrap(), file.crc32);
                handle
                    .emit_all(
                        "download_error",
                        &DownloadError::ChecksumMismatch(crc.unwrap(), file.crc32),
                    )
                    .expect("failed to emit download_error");

                state.rt.download_queue.clear();
                state.rt.job = Job::Idle;

                return;
            }

            handle
                .emit_all("file_downloaded", &target)
                .expect("failed to emit file_downloaded");

            state.download();
        });
    }

    fn download_insecure(&mut self, id: u32, cancel_rx: oneshot::Receiver<()>) {
        let handle = unsafe { self.rt.app_handle.as_ref().unwrap() };
        let file = match self.files.iter().find(|file| file.id == id) {
            Some(file) => file,
            None => {
                log::error!("File not found: {}", id);
                handle
                    .emit_all("download_error", &DownloadError::NotFoundLocal)
                    .expect("failed to emit download_error");

                self.rt.job = Job::Idle;
                self.rt.download_queue.clear();
                return;
            }
        };

        let (tx, mut rx) = mpsc::channel::<usize>(10);
        tokio::spawn(async move {
            let mut bytes = 0;
            while let Some(read) = rx.recv().await {
                bytes += read;
                handle
                    .emit_all("download_progress", bytes)
                    .expect("failed to emit download_progress");
            }
        });

        let target = download_target(&file.path);
        let token = Arc::new(self.token.clone());
        let channel = Arc::new(self.channel_id.clone());
        let cluster_count = file.download_ids.len();
        let mut ids = file.download_ids.clone();

        let (crc_tx, crc_rx) = self
            .do_checksum
            .then(|| mpsc::channel::<(u64, Hasher)>(4))
            .map_or_else(|| (None, None), |(tx, rx)| (Some(tx), Some(rx)));

        let slices = (file.size + SLICE_SIZE - 1) / SLICE_SIZE;
        let crc_handle = tokio::spawn(async move {
            let mut rx = match crc_rx {
                Some(rx) => rx,
                None => return Ok(None),
            };

            let mut hashers = vec![unsafe { std::mem::zeroed() }; slices as usize];
            while let Some((idx, hasher)) = rx.recv().await {
                hashers[idx as usize] = hasher;
            }

            let mut hasher = Hasher::new();
            for other in hashers {
                hasher.combine(&other);
            }

            Ok(Some(hasher.finalize()))
        });

        let writer = match InsecureWriter::new(&target, tx, crc_tx) {
            Ok(writer) => writer,
            Err(err) => {
                log::error!("failed to open file: {}", target);
                handle
                    .emit_all("download_error", &DownloadError::Io(err))
                    .expect("failed to emit download_error");

                self.rt.job = Job::Idle;
                self.rt.download_queue.clear();
                return;
            }
        };

        log::info!("Downloading file: {}", target);
        let mut senders = Vec::with_capacity(ids.len());
        let mut receivers = Vec::with_capacity(ids.len());

        for _ in 0..ids.len() {
            let (sender, receiver) = oneshot::channel::<InsecureClusterW>();
            senders.push(sender);
            receivers.push(receiver);
        }

        let stream = stream::iter(receivers);
        let downloaders =
            stream
                .map(Ok)
                .try_for_each_concurrent(DOWNLOAD_THREADS, move |rx| async move {
                    let mut cluster = match rx.await {
                        Ok(cluster) => cluster,
                        Err(_) => return Ok(()), // TODO: comment why returning Ok(()) is actually ok
                    };

                    cluster.download().await
                });

        let future = tokio::spawn(async move {
            let message_count = cmp::min(ids.len() * 2, 100);
            let mut set = ids.clone();

            'outer: while let Some(id) = ids.first() {
                let mut messages =
                    match api::fetch_messages(&token, &channel, *id, message_count).await {
                        Ok(messages) => messages,
                        Err(err) => {
                            log::error!("failed to fetch messages: {}", err);
                            return Err(err);
                        }
                    };

                let id = *id;
                let mut has_found = false;
                for message in messages.iter_mut() {
                    let message_id = message
                        .id
                        .parse::<u64>()
                        .expect("failed to parse message ID");

                    if message_id == id {
                        has_found = true;
                    }

                    if let Some((idx, id)) = set
                        .iter_mut()
                        .enumerate()
                        .find(|(_, id)| **id == message_id)
                    {
                        ids.retain(|id| *id != message_id);
                        let attachments = message.attachments.take();
                        let cluster = writer.cluster(idx, attachments);
                        *id = 0;

                        let sender = senders.pop().unwrap();
                        if sender.send(cluster).is_err() {
                            break 'outer;
                        }
                    }
                }

                if !has_found {
                    log::warn!("Message not found: {}", id);
                    return Err(DownloadError::NotFoundRemote);
                }
            }

            Ok(())
        });

        let state = unsafe { &*self.rt.this };
        tokio::spawn(async move {
            let now = Instant::now();

            let futures = future::try_join3(
                downloaders,
                Flatten::flatten(future),
                Flatten::flatten(crc_handle),
            );
            let futures = select! {
                futures = futures => futures,
                _ = cancel_rx => {
                    log::debug!("Download canceled");
                    if let Err(err) = fs::remove_file(&target) {
                        log::error!("failed to remove file: {}", err);
                    }

                    return;
                }
            };

            let took = now.elapsed().as_secs_f64();
            log::info!("Downloaded {} cluster(s) in {:.2}s", cluster_count, took);

            let mut state = state.write().await;
            let handle = unsafe { state.rt.app_handle.as_ref().unwrap() };
            let crc = match futures {
                Ok((_, _, crc)) => crc,
                Err(err) => {
                    log::error!("Failed to download file, reason: {}", err);

                    handle
                        .emit_all("download_error", &err)
                        .expect("failed to emit download_error");

                    state.rt.download_queue.clear();
                    state.rt.job = Job::Idle;

                    return;
                }
            };

            if let Some(file) = state.files.iter().find(|file| file.id == id)
                && crc.is_some_and(|crc| crc != file.crc32)
            {
                log::warn!("CRC32 mismatch: {:x} != {:x}", crc.unwrap(), file.crc32);
                handle
                    .emit_all(
                        "download_error",
                        &DownloadError::ChecksumMismatch(crc.unwrap(), file.crc32),
                    )
                    .expect("failed to emit download_error");

                state.rt.download_queue.clear();
                state.rt.job = Job::Idle;

                return;
            }

            handle
                .emit_all("file_downloaded", &target)
                .expect("failed to emit file_downloaded");

            state.download();
        });
    }
}
