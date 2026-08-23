//! Domain models shared by every layer.

pub mod download;

pub use download::{
    ChunkRecord, ChunkStatus, DownloadId, DownloadRecord, DownloadState, PublicId,
};
