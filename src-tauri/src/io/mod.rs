use std::cell::UnsafeCell;
use std::io;

use aes_gcm::Aes256Gcm;

pub mod reader;
pub mod writer;

pub mod secure_reader;
pub mod secure_writer;

pub mod consts;

pub struct Cipher(UnsafeCell<Aes256Gcm>);

unsafe impl Send for Cipher {}
unsafe impl Sync for Cipher {}

pub trait Cluster {
    type Iter: Iterator<Item = Result<Vec<u8>, io::Error>>;

    fn next_slice(&mut self) -> Option<Self::Iter>;
}
