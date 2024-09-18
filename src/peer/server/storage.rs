use std::env;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::io;
use tokio::io::{Error, ErrorKind};
use tokio::fs;
use uuid::Uuid;

pub trait ChunksStorage {
    async fn save(&mut self, chunk: &[u8], hash: &[u8]);
    async fn retrieve(&mut self, hash: &[u8]) -> io::Result<Vec<u8>>;
    async fn save_meta_before_shutdown(&self);
}

const LAST_CHECKPOINT_FILENAME: &str = "last_checkpoint.json";

#[derive(Serialize, Deserialize)]
pub struct LocalChunksStorage {
    keeping_dir: PathBuf,
    matching_db: HashMap<Vec<u8>, String>,
}

impl LocalChunksStorage {
    pub async fn new() -> LocalChunksStorage {
        let binding = env::var("HOME").unwrap();
        let home_dir = binding.as_str();
        let keeping_dir = PathBuf::from(home_dir).join(".leaf").join("chunks");

        Self::from_file(keeping_dir.parent().unwrap()).await.unwrap_or_else(|| {
            let matching_db = HashMap::new();

            LocalChunksStorage {
                keeping_dir,
                matching_db,
            }
        })
    }

    async fn from_file(path: &Path) -> Option<LocalChunksStorage> {
        let mut storage = None;
        for file in path.read_dir().unwrap() {
            let filename = file.unwrap().file_name();
            if filename.eq(LAST_CHECKPOINT_FILENAME) {
                let json = fs::read_to_string(filename).await.unwrap();
                storage = Some(serde_json::from_str(&json).unwrap());
                break;
            };
        }
        storage
    }
}

impl ChunksStorage for LocalChunksStorage {
    async fn save(&mut self, chunk: &[u8], hash: &[u8]) {
        let filename = Uuid::from_slice(chunk).unwrap().to_string();
        let filepath = self.keeping_dir.join(&filename);
        fs::write(filepath, chunk).await.unwrap();
        let hash_vec = hash.to_vec();
        self.matching_db.insert(hash_vec, filename);
    }

    async fn retrieve(&mut self, hash: &[u8]) -> io::Result<Vec<u8>> {
        let filename = match self.matching_db.get(hash) {
            Some(f) => f.to_string(),
            None => return Err(Error::new(ErrorKind::NotFound, "No chunk with such hash sum is found")),
        };

        let filepath = self.keeping_dir.join(&filename);
        let content = fs::read_to_string(filepath).await.unwrap();

        let content_bytes = content.as_bytes();
        let content_bytes_vec = content_bytes.to_vec();

        Ok(content_bytes_vec)
    }

    async fn save_meta_before_shutdown(&self) {
        let json = serde_json::to_string(&self).unwrap();
        let folder = self.keeping_dir.parent().unwrap();
        fs::write(folder.join(LAST_CHECKPOINT_FILENAME), json).await.unwrap();
    }
}
