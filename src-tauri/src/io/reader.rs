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
            local_total_slices: slices,
            slice_counter: 0,
            cluster_index: self.cluster as u64 - 1,
            read_sender: self.read_sender.clone(),
            crc_sender: self.crc_sender.clone(),
        })
    }
}

pub struct InsecureClusterR {
    file: Arc<Mutex<File>>,
    file_size: u64,
    local_total_slices: usize,
    slice_counter: usize,
    pub cluster_index: u64,
    read_sender: mpsc::Sender<usize>,
    crc_sender: CrcSender,
}

unsafe impl Send for InsecureClusterR {}
unsafe impl Sync for InsecureClusterR {}

impl InsecureClusterR {
    pub fn get_size(&self) -> u64 {
        cmp::min(
            CLUSTER_SIZE,
            self.file_size - self.cluster_index * CLUSTER_SIZE,
        )
    }
}

impl Cluster for InsecureClusterR {
    type Iter = InsecureSlice;

    fn next_slice(&mut self) -> Option<Self::Iter> {
        log::debug!(
            "Next slice: {} / {}",
            self.slice_counter,
            self.local_total_slices
        );
        if self.slice_counter == self.local_total_slices {
            return None;
        }

        let slice_index = self.cluster_index * CLUSTER_CAP + self.slice_counter as u64;
        self.slice_counter += 1;

        Some(InsecureSlice {
            file: self.file.clone(),
            position_in_slice: 0,
            file_size: self.file_size,
            slice_index,
            read_sender: self.read_sender.clone(),
            crc_sender: self.crc_sender.clone(),
            crc32: Hasher::new(),
        })
    }
}

pub struct InsecureSlice {
    file: Arc<Mutex<File>>,
    position_in_slice: u64,
    file_size: u64,
    slice_index: u64,
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

        let slice = self.slice_index;
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
        if self.position_in_slice == SLICE_SIZE {
            self.send_crc();
            return None;
        }

        let position = self.position_in_slice + self.slice_index * SLICE_SIZE;

        let buffer_size = cmp::min(self.file_size.saturating_sub(position), BUFFER_SIZE_I);
        let buffer_size = cmp::min(SLICE_SIZE, buffer_size);

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

        let size = buffer.len();
        let sender = self.read_sender.clone();
        tokio::spawn(async move {
            if let Err(err) = sender.send(size).await {
                log::error!("Failed to send read size: {:?}", err);
            }
        });

        self.position_in_slice += buffer_size;
        Some(Ok(buffer))
    }
}
