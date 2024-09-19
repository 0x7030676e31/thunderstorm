use super::consts::*;
use super::Cluster;

use std::fs::File;
use std::io::{self, Error, Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex};
use std::{cmp, mem};

use crc32fast::Hasher;
use tokio::sync::mpsc;

type CrcSender = mpsc::Sender<(u64, Hasher)>;

pub struct InsecureReader {
    file: Arc<Mutex<File>>,
    slices: usize,
    pub clusters: usize,
    cluster: usize,
    pub file_size: u64,
    read_sender: mpsc::Sender<usize>,
    crc_sender: CrcSender,
}

impl InsecureReader {
    pub fn new<T: AsRef<str>>(
        path: T,
        read_sender: mpsc::Sender<usize>,
        crc_sender: CrcSender,
    ) -> io::Result<Self> {
        let file = File::open(path.as_ref())?;
        let size = file.metadata()?.len();

        let slices = (size + SLICE_SIZE - 1) / SLICE_SIZE;
        let clusters = (slices + CLUSTER_CAP - 1) / CLUSTER_CAP;

        Ok(Self {
            file: Arc::new(Mutex::new(file)),
            slices: slices as usize,
            clusters: clusters as usize,
            cluster: 0,
            file_size: size,
            read_sender,
            crc_sender,
        })
    }

    pub fn next_cluster(&mut self) -> Option<InsecureClusterR> {
        log::debug!("Next cluster: {} / {}", self.cluster, self.clusters);
        if self.cluster == self.clusters {
            return None;
        }

        let slices = cmp::min(
            CLUSTER_CAP as usize,
            self.slices - self.cluster * CLUSTER_CAP as usize,
        );

        self.cluster += 1;
        Some(InsecureClusterR {
            file: self.file.clone(),
            file_size: self.file_size,
            slices,
            slice: 0,
            index: self.cluster as u64 - 1,
            read_sender: self.read_sender.clone(),
            crc_sender: self.crc_sender.clone(),
        })
    }
}

pub struct InsecureClusterR {
    file: Arc<Mutex<File>>,
    file_size: u64,
    slices: usize,
    slice: usize,
    pub index: u64,
    read_sender: mpsc::Sender<usize>,
    crc_sender: CrcSender,
}

unsafe impl Send for InsecureClusterR {}
unsafe impl Sync for InsecureClusterR {}

impl InsecureClusterR {
    pub fn get_size(&self) -> u64 {
        cmp::min(CLUSTER_SIZE, self.file_size - self.index * CLUSTER_SIZE)
    }
}

impl Cluster for InsecureClusterR {
    type Iter = InsecureSlice;

    fn next_slice(&mut self) -> Option<Self::Iter> {
        log::debug!("Next slice: {} / {}", self.slice, self.slices);
        if self.slice == self.slices {
            return None;
        }

        let this_slice = self.index * CLUSTER_CAP + self.slice as u64;
        self.slice += 1;

        Some(InsecureSlice {
            file: self.file.clone(),
            position: this_slice * SLICE_SIZE,
            file_size: self.file_size,
            slice: this_slice,
            index: self.index,
            read_sender: self.read_sender.clone(),
            crc_sender: self.crc_sender.clone(),
            crc32: Hasher::new(),
        })
    }
}

pub struct InsecureSlice {
    file: Arc<Mutex<File>>,
    position: u64,
    file_size: u64,
    slice: u64,
    index: u64,
    read_sender: mpsc::Sender<usize>,
    crc_sender: CrcSender,
    crc32: Hasher,
}

unsafe impl Send for InsecureSlice {}
unsafe impl Sync for InsecureSlice {}

impl InsecureSlice {
    fn send_crc(&mut self) {
        let sender = mem::replace(&mut self.crc_sender, mpsc::channel(1).0);
        let hasher = mem::replace(&mut self.crc32, Hasher::new());

        let slice = self.slice;
        tokio::spawn(async move {
            if let Err(err) = sender.send((slice, hasher)).await {
                log::error!("Failed to send CRC32: {:?}", err);
            }
        });
    }
}

impl Iterator for InsecureSlice {
    type Item = Result<Vec<u8>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == BUFFERS_PER_SLICE {
            self.send_crc();
            return None;
        }

        let already_read = self.index * BUFFER_SIZE_I;
        let position = self.position + already_read;

        let buffer_size = cmp::min(BUFFER_SIZE_I, SLICE_SIZE - already_read);
        let buffer_size = cmp::min(buffer_size, self.file_size.saturating_sub(position));

        if buffer_size == 0 {
            self.send_crc();
            return None;
        }

        let mut buffer = vec![0; buffer_size as usize];
        let mut file = self.file.lock().unwrap();

        file.seek(SeekFrom::Start(position))
            .expect("Failed to seek file");

        file.read_exact(&mut buffer).expect("Failed to read file");
        drop(file);

        self.crc32.update(&buffer);
        self.index += 1;

        let size = buffer.len();
        let sender = self.read_sender.clone();
        tokio::spawn(async move {
            if let Err(err) = sender.send(size).await {
                log::error!("Failed to send read size: {:?}", err);
            }
        });

        Some(Ok(buffer))
    }
}
