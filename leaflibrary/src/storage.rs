use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};
use tokio::fs;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Eq, PartialEq)]
struct FileChunk {
    pub hash: Vec<u8>,
    data: Vec<u8>,
    pub full_flag: bool,
}

impl FileChunk {
    fn new(hash: Vec<u8>, data: Vec<u8>) -> Self {
        Self { hash, data, full_flag: false, }
    }
}

#[derive(Clone)]
pub struct BroadcastUdpServerStorage {
    storage_path: PathBuf,
    curr_chunks: Arc<Mutex<Vec<FileChunk>>>,
}

impl BroadcastUdpServerStorage {
    pub fn new(storage_path: &PathBuf) -> Self {
        Self {
            storage_path: storage_path.clone(),
            curr_chunks: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn add(&mut self, hash: &[u8], data: &[u8]) -> Result<(), tokio::io::Error> {
        for chunk in self.curr_chunks.lock().await.iter_mut() {
            if chunk.hash.eq(hash) && !chunk.full_flag {
                chunk.data.append(&mut data.to_vec());
                return Ok(());
            }
        }
        self.curr_chunks.lock().await.push(FileChunk::new(hash.to_vec(), data.to_vec()));
        Ok(())
    }

    pub async fn retrieve(&self, hash: &[u8]) -> Result<Vec<u8>, tokio::io::Error> {
        let files = std::fs::read_dir(&self.storage_path)?;
        for file in files {
            let filename = file?.path();
            let content: FileChunk = serde_json::from_slice(&fs::read(&filename).await?)?;
            if content.hash.eq(hash) {
                fs::remove_file(filename).await?;
                return Ok(content.data);
            }
        }
        Err(tokio::io::Error::last_os_error())
    }

    pub async fn finalize(&mut self, hash: &[u8]) -> Result<(), tokio::io::Error> {
        let mut chunks = self.curr_chunks.lock().await;
        for i in 0..chunks.len() {
            if chunks[i].hash.eq(hash) && !chunks[i].full_flag {
                chunks[i].full_flag = true;
                self.save_chunk(&chunks[i]).await?;
                chunks.remove(i);
                return Ok(());
            }
        }
        Err(tokio::io::Error::last_os_error())
    }

    async fn save_chunk(&self, chunk: &FileChunk) -> Result<(), tokio::io::Error> {
        let json = serde_json::to_vec(&chunk)?;
        let filepath = self.storage_path.join(Uuid::new_v4().to_string() + ".dat");
        fs::write(&filepath, &json).await?;
        Ok(())
    }

    pub async fn shutdown(&self) {
        let mut chunks = self.curr_chunks.lock().await;
        for i in 0..chunks.len() {
            if chunks[i].full_flag {
                self.save_chunk(&chunks[i]).await.unwrap();
                chunks.remove(i);
            }
        };
    }
}