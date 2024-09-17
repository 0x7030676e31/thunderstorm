use crate::consts::*;

use std::cell::UnsafeCell;
use std::cmp;
use std::fs::File;
use std::io::{self, Error, ErrorKind, Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex};

use aes_gcm::aead::AeadMutInPlace;
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use crc32fast::Hasher;
use tokio::sync::mpsc;

type CrcSender = mpsc::Sender<(u64, Hasher)>;

#[derive(Debug)]
pub struct Reader {
    file: Arc<Mutex<File>>,
    cipher: Arc<UnsafeCell<Aes256Gcm>>,
    slices: usize,
    pub clusters: usize,
    cluster: usize,
    pub file_size: u64,
    final_size: u64,
    read_sender: mpsc::Sender<usize>,
    crc_sender: Option<CrcSender>,
}

unsafe impl Send for Reader {}
unsafe impl Sync for Reader {}

impl Reader {
    pub fn new<T: AsRef<str>>(
        path: T,
        key: &[u8; 32],
        read_sender: mpsc::Sender<usize>,
        crc_sender: Option<CrcSender>,
    ) -> io::Result<Self> {
        let file = File::open(path.as_ref())?;
        let size = file.metadata()?.len();

        let slices = (size + BYTES_PER_SLICE - 1) / BYTES_PER_SLICE;
        let full_slices = size / BYTES_PER_SLICE;
        let clusters = (slices + CLUSTER_CAP - 1) / CLUSTER_CAP;

        let trailing_bytes = size - full_slices * BYTES_PER_SLICE;
        let trailing_buffers = (trailing_bytes + RAW_BUFFER_SIZE - 1) / RAW_BUFFER_SIZE;
        let encrypted_size =
            size + full_slices * BUFFERS_PER_SLICE * AES_OVERHEAD + trailing_buffers * AES_OVERHEAD;

        let key = Key::<Aes256Gcm>::from_slice(key);
        let cipher = Aes256Gcm::new(key);

        Ok(Self {
            file: Arc::new(Mutex::new(file)),
            #[allow(clippy::arc_with_non_send_sync)]
            cipher: Arc::new(UnsafeCell::new(cipher)),
            slices: slices as usize,
            clusters: clusters as usize,
            cluster: 0,
            file_size: size,
            final_size: encrypted_size,
            read_sender,
            crc_sender,
        })
    }

    pub fn next_cluster(&mut self) -> Option<Cluster> {
        if self.cluster == self.clusters {
            return None;
        }

        let slices = cmp::min(
            CLUSTER_CAP as usize,
            self.slices - self.cluster * CLUSTER_CAP as usize,
        );

        self.cluster += 1;
        Some(Cluster {
            file: self.file.clone(),
            cipher: self.cipher.clone(),
            file_size: self.file_size,
            slices,
            slice: 0,
            index: self.cluster as u64 - 1,
            final_size: self.final_size,
            read_sender: self.read_sender.clone(),
            crc_sender: self.crc_sender.clone(),
        })
    }
}

#[derive(Debug)]
pub struct Cluster {
    file: Arc<Mutex<File>>,
    cipher: Arc<UnsafeCell<Aes256Gcm>>,
    file_size: u64,
    slices: usize,
    slice: usize,
    pub index: u64, // index of the current cluster
    final_size: u64,
    read_sender: mpsc::Sender<usize>,
    crc_sender: Option<CrcSender>,
}

unsafe impl Send for Cluster {}
unsafe impl Sync for Cluster {}

impl Cluster {
    pub fn get_size(&self) -> u64 {
        cmp::min(CLUSTER_SIZE, self.final_size - self.index * CLUSTER_SIZE)
    }

    pub fn next_slice(&mut self) -> Option<Slice> {
        if self.slice == self.slices {
            return None;
        }

        let this_slice = self.index * CLUSTER_CAP + self.slice as u64;
        let hasher = if self.crc_sender.is_some() {
            Some(Hasher::new())
        } else {
            None
        };

        self.slice += 1;
        Some(Slice {
            file: self.file.clone(),
            cipher: self.cipher.clone(),
            position: this_slice * BYTES_PER_SLICE,
            file_size: self.file_size,
            slice: this_slice,
            index: 0,
            nonce: [0; 12],
            read_sender: self.read_sender.clone(),
            crc_sender: self.crc_sender.clone(),
            crc32: hasher,
        })
    }
}

pub struct Slice {
    file: Arc<Mutex<File>>,
    cipher: Arc<UnsafeCell<Aes256Gcm>>,
    position: u64,
    file_size: u64,
    slice: u64, // index of this slice
    index: u64, // index of the current buffer
    nonce: [u8; 12],
    read_sender: mpsc::Sender<usize>,
    crc_sender: Option<CrcSender>,
    crc32: Option<Hasher>,
}

unsafe impl Send for Slice {}
unsafe impl Sync for Slice {}

impl Slice {
    fn send_crc(&mut self) {
        let hasher = self.crc32.take();
        let sender = self.crc_sender.take();

        if let (Some(hasher), Some(sender)) = (hasher, sender) {
            let slice = self.slice;
            tokio::spawn(async move {
                if let Err(err) = sender.send((slice, hasher)).await {
                    log::error!("Failed to send CRC32: {:?}", err);
                }
            });
        }
    }
}

impl Iterator for Slice {
    type Item = Result<Vec<u8>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == BUFFERS_PER_SLICE {
            self.send_crc();
            return None;
        }

        let slice_position = self.index * RAW_BUFFER_SIZE;

        let buffer_size = cmp::min(
            RAW_BUFFER_SIZE,
            BYTES_PER_SLICE.saturating_sub(slice_position),
        );

        let buffer_size = cmp::min(
            buffer_size,
            (self.file_size - self.position).saturating_sub(slice_position),
        );

        if buffer_size == 0 {
            self.send_crc();
            return None;
        }

        let mut buffer = vec![0; buffer_size as usize];
        let mut file = self.file.lock().expect("Failed to lock file");

        file.seek(SeekFrom::Start(self.position + slice_position))
            .expect("Failed to seek file");

        file.read_exact(&mut buffer).expect("Failed to read file");

        drop(file);
        if let Some(hasher) = self.crc32.as_mut() {
            hasher.update(&buffer)
        }

        let size = buffer.len();
        let sender = self.read_sender.clone();
        tokio::spawn(async move {
            if let Err(err) = sender.send(size).await {
                log::error!("Failed to send buffer size: {:?}", err);
            }
        });

        self.nonce[4..]
            .copy_from_slice(&(self.slice * BUFFERS_PER_SLICE + self.index).to_be_bytes());
        let nonce = Nonce::from(self.nonce);

        if let Err(err) =
            unsafe { &mut *self.cipher.get() }.encrypt_in_place(&nonce, b"", &mut buffer)
        {
            log::error!("Failed to encrypt buffer: {:?}", err);
            return Some(Err(Error::new(
                ErrorKind::Other,
                format!("Failed to encrypt buffer: {:?}", err),
            )));
        }

        self.index += 1;
        Some(Ok(buffer))
    }
}
