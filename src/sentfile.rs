use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SentFile {
    pub hashes: Vec<Option<Vec<u8>>>,
    pub size: usize,
}

impl SentFile {
    pub fn new(hashes: Vec<Option<Vec<u8>>>, size: usize) -> SentFile {
        SentFile {
            hashes,
            size,
        }
    }

    pub async fn save_metadata(self, filename: &PathBuf) {
        let json = serde_json::to_vec(&self).unwrap();
        tokio::fs::write(filename, &json).await.unwrap();
    }

    pub fn from_metadata(content: &[u8]) -> SentFile {
        let obj: SentFile = serde_json::from_slice(content).unwrap();
        obj
    }
}
