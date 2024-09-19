pub const SLICE_SIZE: u64 = 1024 * 1024 * 25;
pub const BUFFER_SIZE_I: u64 = 1024 * 1024;
pub const BUFFER_SIZE_U: usize = BUFFER_SIZE_I as usize;
pub const CLUSTER_CAP: u64 = 10;
pub const AES_OVERHEAD: u64 = 16;
pub const UPLOAD_THREADS: usize = 4;
pub const DOWNLOAD_THREADS: usize = 2;

pub const CLUSTER_SIZE: u64 = SLICE_SIZE * CLUSTER_CAP; // Total size of all attachments per message
pub const RAW_BUFFER_SIZE: u64 = BUFFER_SIZE_I - AES_OVERHEAD; // IO buffer size
pub const BUFFERS_PER_SLICE: u64 = (SLICE_SIZE + BUFFER_SIZE_I - 1) / BUFFER_SIZE_I; // Number of buffers per slice (rounded up)
pub const BYTES_PER_SLICE: u64 = SLICE_SIZE - BUFFERS_PER_SLICE * AES_OVERHEAD; // Number of IO bytes per slice (excluding encryption overhead)
