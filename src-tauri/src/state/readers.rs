use super::errors::UploadError;
use super::model::{File, Job, State};
use crate::api;
use crate::io::consts::UPLOAD_THREADS;
use crate::io::reader::{InsecureClusterR, InsecureReader};
use crate::io::secure_reader::{SecureClusterR, SecureReader};
use crate::utils::Flatten;

use std::fs;
use std::sync::Arc;
use std::time::Instant;

use crc32fast::Hasher;
use futures::future;
use futures::stream::{self, StreamExt, TryStreamExt};
use tauri::Manager;
use tokio::select;
use tokio::sync::{mpsc, oneshot};

impl State {
    pub fn extend_upload_queue(&mut self, files: Vec<String>) {
        if !self.rt.job.is_upload_extendable() {
            log::warn!("Not uploading, ignoring files");
            return;
        }

        let mut queue = Vec::with_capacity(files.len());
        let handle = unsafe { self.rt.app_handle.as_ref().unwrap() };

        for file in files {
            let meta = match fs::metadata(&file) {
                Ok(file) => file,
                Err(err) => {
                    log::error!("failed to get file metadata: {}", err);
                    handle
                        .emit_all("upload_error", &UploadError::Io(err))
                        .expect("failed to emit upload_error");

                    return;
                }
            };

            let len = meta.len();
            if len == 0 {
                log::warn!("Skipping empty file: {}", file);
                continue;
            }

            if meta.is_file() {
                queue.push((file.clone(), len));
            }
        }

        if queue.is_empty() {
            log::warn!("No files to upload");
            return;
        }

        handle
            .emit_all("extend_upload_queue", &queue)
            .expect("failed to emit extend_upload_queue");

        log::info!("Extending the queue with {} files", queue.len());
        self.rt
            .upload_queue
            .extend(queue.into_iter().map(|(file, _)| file));

        if self.rt.job == Job::Idle {
            log::info!("Starting uploading {} files", self.rt.upload_queue.len());
            self.upload();
        }
    }

    fn upload(&mut self) {
        let file = match self.rt.upload_queue.pop_front() {
            Some(file) => file,
            None => {
                log::info!("No more files to upload, stopping");

                self.rt.job = Job::Idle;
                return;
            }
        };

        log::info!("Uploading file: {}", file);
        let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
        self.rt.job = Job::Upload { cancel_tx };

        if self.do_encrypt {
            self.upload_secure(file, cancel_rx);
        } else {
            self.upload_insecure(file, cancel_rx);
        }
    }

    fn upload_secure(&mut self, file: String, cancel_rx: oneshot::Receiver<()>) {
        let (tx, mut rx) = mpsc::channel::<usize>(10);
        let handle = unsafe { self.rt.app_handle.as_ref().unwrap() };

        tokio::spawn(async move {
            let mut bytes = 0;
            while let Some(read) = rx.recv().await {
                bytes += read;
                handle
                    .emit_all("upload_progress", bytes)
                    .expect("failed to emit upload_progress");
            }
        });

        let (crc_tx, mut crc_rx) = mpsc::channel::<(u64, Hasher)>(4);

        let crc_handle = tokio::spawn(async move {
            let mut hashers = Vec::new();
            while let Some((idx, hasher)) = crc_rx.recv().await {
                let idx = idx as usize;
                if hashers.len() <= idx {
                    hashers.resize(idx + 1, unsafe { std::mem::zeroed() });
                }

                hashers.insert(idx, hasher);
            }

            let mut hasher = Hasher::new();
            for other in hashers {
                hasher.combine(&other);
            }

            Ok(hasher.finalize())
        });

        let key = self.aes_key();
        let mut reader = match SecureReader::new(&file, &key, tx, crc_tx) {
            Ok(reader) => reader,
            Err(err) => {
                log::error!("failed to open file: {}", file);
                handle
                    .emit_all("upload_error", &UploadError::Io(err))
                    .expect("failed to emit upload_error");

                self.rt.job = Job::Idle;
                self.rt.upload_queue.clear();
                return;
            }
        };

        let clusters = reader.clusters as usize;
        let file_size = reader.file_size;

        // Channel ID, cluster index
        type Sender = (u64, usize);
        // Upload details, current cluster, finish sender
        type OneShot = (
            Vec<api::UploadDetailsInner>,
            SecureClusterR,
            mpsc::Sender<Sender>,
        );

        let (tx, mut rx) = mpsc::channel::<Sender>(UPLOAD_THREADS);

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
        let uploaders = stream
            .map(Ok)
            .try_for_each_concurrent(UPLOAD_THREADS, move |rx| {
                let token2 = Arc::clone(&token2);
                let channel2 = Arc::clone(&channel2);

                async move {
                    let (details, cluster, sender) = match rx.await {
                        Ok(result) => result,
                        Err(_) => return Ok(()), // TODO: comment why returning Ok(()) is actually ok
                    };

                    let index = cluster.index as usize;
                    api::upload(&details, cluster).await?;

                    let id = api::finalize(&token2, &channel2, &details).await?;
                    sender
                        .send((id, index))
                        .await
                        .expect("failed to send finish signal");
                    Ok::<(), UploadError>(())
                }
            });

        let futures = tokio::spawn(async move {
            let mut ids = vec![0; clusters];
            while let Some((id, index)) = rx.recv().await {
                ids[index] = id;
            }

            Ok::<_, UploadError>(ids)
        });

        let token = Arc::clone(&token);
        let channel = Arc::clone(&channel);
        let preuploads = tokio::spawn(async move {
            while let Some(cluster) = reader.next_cluster() {
                let details = api::preupload(&token, &channel, cluster.get_size()).await;
                let details = match details {
                    Ok(details) => details,
                    Err(err) => return Err(err),
                };

                let sender = senders.pop().unwrap();

                // When the receiver is dropped, uploading was canceled
                if sender.send((details, cluster, tx.clone())).is_err() {
                    break;
                }
            }

            Ok(())
        });

        let state = unsafe { &*self.rt.this };
        tokio::spawn(async move {
            let now = Instant::now();

            let futures = future::try_join4(
                Flatten::flatten(futures),
                uploaders,
                Flatten::flatten(preuploads),
                Flatten::flatten(crc_handle),
            );
            let futures = select! {
                futures = futures => futures,
                _ = cancel_rx => {
                    log::debug!("Upload canceled");
                    return;
                }
            };

            let mut state = state.write().await;
            let handle = unsafe { state.rt.app_handle.as_ref().unwrap() };
            let (ids, crc) = match futures {
                Ok((ids, _, _, crc)) => (ids, crc),
                Err(err) => {
                    log::error!("Failed to upload a file, reason: {}", err);

                    handle
                        .emit_all("upload_error", &err)
                        .expect("failed to emit upload_error");

                    state.rt.upload_queue.clear();
                    state.rt.job = Job::Idle;

                    return;
                }
            };

            let took = now.elapsed().as_secs_f64();
            log::info!(
                "Uploaded {} cluster(s) in {:.2}s; crc32: {:x}",
                clusters,
                took,
                crc,
            );

            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("failed to get timestamp")
                .as_secs();

            let file = File {
                id: state.next_id(),
                path: file,
                size: file_size,
                download_ids: ids,
                created_at: timestamp,
                updated_at: timestamp,
                crc32: crc,
                encryption_key: Some(key),
            };

            handle
                .emit_all("file_uploaded", &file)
                .expect("failed to emit file_uploaded");

            state.files.push(file);

            state.write();
            state.upload();
        });
    }

    fn upload_insecure(&mut self, file: String, cancel_rx: oneshot::Receiver<()>) {
        let (tx, mut rx) = mpsc::channel::<usize>(10);
        let handle = unsafe { self.rt.app_handle.as_ref().unwrap() };

        tokio::spawn(async move {
            let mut bytes = 0;
            while let Some(read) = rx.recv().await {
                bytes += read;
                handle
                    .emit_all("upload_progress", bytes)
                    .expect("failed to emit upload_progress");
            }
        });

        let (crc_tx, mut crc_rx) = mpsc::channel::<(u64, Hasher)>(4);

        let crc_handle = tokio::spawn(async move {
            let mut hashers = Vec::new();
            while let Some((idx, hasher)) = crc_rx.recv().await {
                let idx = idx as usize;
                if hashers.len() <= idx {
                    hashers.resize(idx + 1, unsafe { std::mem::zeroed() });
                }

                hashers.insert(idx, hasher);
            }

            let mut hasher = Hasher::new();
            for other in hashers {
                hasher.combine(&other);
            }

            Ok(hasher.finalize())
        });

        let mut reader = match InsecureReader::new(&file, tx, crc_tx) {
            Ok(reader) => reader,
            Err(err) => {
                log::error!("failed to open file: {}", file);
                handle
                    .emit_all("upload_error", &UploadError::Io(err))
                    .expect("failed to emit upload_error");

                self.rt.job = Job::Idle;
                self.rt.upload_queue.clear();
                return;
            }
        };

        let clusters = reader.clusters as usize;
        let file_size = reader.file_size;

        // Channel ID, cluster index
        type Sender = (u64, usize);
        // Upload details, current cluster, finish sender
        type OneShot = (
            Vec<api::UploadDetailsInner>,
            InsecureClusterR,
            mpsc::Sender<Sender>,
        );

        let (tx, mut rx) = mpsc::channel::<Sender>(UPLOAD_THREADS);

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
        let uploaders = stream
            .map(Ok)
            .try_for_each_concurrent(UPLOAD_THREADS, move |rx| {
                let token2 = Arc::clone(&token2);
                let channel2 = Arc::clone(&channel2);

                async move {
                    let (details, cluster, sender) = match rx.await {
                        Ok(result) => result,
                        Err(_) => return Ok(()), // TODO: comment why returning Ok(()) is actually ok
                    };

                    let index = cluster.index as usize;
                    api::upload(&details, cluster).await?;

                    let id = api::finalize(&token2, &channel2, &details).await?;
                    sender
                        .send((id, index))
                        .await
                        .expect("failed to send finish signal");
                    Ok::<(), UploadError>(())
                }
            });

        let futures = tokio::spawn(async move {
            let mut ids = vec![0; clusters];
            while let Some((id, index)) = rx.recv().await {
                ids[index] = id;
            }

            Ok::<_, UploadError>(ids)
        });

        let token = Arc::clone(&token);
        let channel = Arc::clone(&channel);
        let preuploads = tokio::spawn(async move {
            while let Some(cluster) = reader.next_cluster() {
                let details = api::preupload(&token, &channel, cluster.get_size()).await;
                let details = match details {
                    Ok(details) => details,
                    Err(err) => return Err(err),
                };

                let sender = senders.pop().unwrap();

                // When the receiver is dropped, uploading was canceled
                if sender.send((details, cluster, tx.clone())).is_err() {
                    break;
                }
            }

            Ok(())
        });

        let state = unsafe { &*self.rt.this };
        tokio::spawn(async move {
            let now = Instant::now();

            let futures = future::try_join4(
                Flatten::flatten(futures),
                uploaders,
                Flatten::flatten(preuploads),
                Flatten::flatten(crc_handle),
            );
            let futures = select! {
                futures = futures => futures,
                _ = cancel_rx => {
                    log::debug!("Upload canceled");
                    return;
                }
            };

            let mut state = state.write().await;
            let handle = unsafe { state.rt.app_handle.as_ref().unwrap() };
            let (ids, crc) = match futures {
                Ok((ids, _, _, crc)) => (ids, crc),
                Err(err) => {
                    log::error!("Failed to upload a file, reason: {}", err);

                    handle
                        .emit_all("upload_error", &err)
                        .expect("failed to emit upload_error");

                    state.rt.upload_queue.clear();
                    state.rt.job = Job::Idle;

                    return;
                }
            };

            let took = now.elapsed().as_secs_f64();
            log::info!(
                "Uploaded {} cluster(s) in {:.2}s; crc32: {:x}",
                clusters,
                took,
                crc,
            );

            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("failed to get timestamp")
                .as_secs();

            let file = File {
                id: state.next_id(),
                path: file,
                size: file_size,
                download_ids: ids,
                created_at: timestamp,
                updated_at: timestamp,
                crc32: crc,
                encryption_key: None,
            };

            handle
                .emit_all("file_uploaded", &file)
                .expect("failed to emit file_uploaded");

            state.files.push(file);

            state.write();
            state.upload();
        });
    }
}
