use std::path::PathBuf;
use serde::{Deserialize, Serialize};

use tokio::fs;

#[derive(Serialize, Deserialize)]
struct FileChunk {
    pub hash: Vec<u8>,
    data: Vec<u8>,
}

impl FileChunk {
    fn new(hash: Vec<u8>, data: Vec<u8>) -> Self {
        Self { hash, data }
    }
}

#[derive(Clone)]
pub struct BroadcastUdpServerStorage {
    storage_path: PathBuf,
}

impl BroadcastUdpServerStorage {
    pub fn new(storage_path: PathBuf) -> Self {
        Self { storage_path }
    }

    pub async fn add(&self, hash: &[u8], data: &[u8]) -> Result<(), tokio::io::Error> {
        let chunk = serde_json::to_vec(&FileChunk::new(hash.into(), data.into()))?;
        let filename = uuid::Uuid::new_v4().to_string() + ".bin";
        let fullpath = self.storage_path.join(filename);
        fs::write(fullpath, chunk).await?;
        Ok(())
    }

    pub async fn retrieve(&self, hash: &[u8]) -> Result<Vec<u8>, tokio::io::Error> {
        let files = std::fs::read_dir(&self.storage_path)?;
        for file in files {
            let content: FileChunk = serde_json::from_slice(&fs::read(file.unwrap().path()).await?)?;
            if content.hash.eq(hash) {
                return Ok(content.data);
            }
        }
        Err(tokio::io::Error::last_os_error())
    }
}