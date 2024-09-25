use std::path::PathBuf;
use crate::fs::{FilesystemWorker, LocalFilesystemWorker};
use crate::shared_secret::{ReedSolomonSecretSharer, SecretSharer};

mod hash;
mod shared_secret;
mod message;
mod consts;
mod crypto;
mod fs;

pub async fn store_file(path: PathBuf) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error>> {
    // 1. Read file
    let fs_worker: Box<dyn FilesystemWorker> = Box::new(LocalFilesystemWorker::new());
    let content = fs_worker.get_content(&path).await?;
    // 2. Split file content into chunks
    let sharer: Box<dyn SecretSharer> = Box::new(ReedSolomonSecretSharer::new());

}