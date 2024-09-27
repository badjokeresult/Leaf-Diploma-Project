use std::path::PathBuf;
use crate::crypto::{Encryptor, KuznechikEncryptor};
use crate::fs::{FilesystemWorker, LocalFilesystemWorker};
use crate::shared_secret::{ReedSolomonSecretSharer, SecretSharer};

mod hash;
mod shared_secret;
mod message;
mod consts;
mod crypto;
mod fs;
mod codec;
mod peer;

pub async fn store_file(path: PathBuf) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error>> {
    // 1. Read file
    let fs_worker: Box<dyn FilesystemWorker> = Box::new(LocalFilesystemWorker::new());
    let content = fs_worker.get_content(&path).await?;
    // 2. Split file content into chunks
    let sharer: Box<dyn SecretSharer> = Box::new(ReedSolomonSecretSharer::new());
    let chunks = sharer.split_into_chunks(&content)?;
    // 3. Encrypt chunks
    let encryptor: Box<dyn Encryptor> = Box::new(KuznechikEncryptor::new()?);
    let mut encrypted_chunks = vec![];
    for chunk in chunks {
        encrypted_chunks.push(encryptor.encrypt_chunk(&chunk).await?);
    }
    // 4. Send it into client side peer
    let client =
}