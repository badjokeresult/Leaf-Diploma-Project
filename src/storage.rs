use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize)]
pub struct BroadcastServerStorage {
    pub database: HashMap<Vec<u8>, Vec<u8>>,
}

impl BroadcastServerStorage {
    pub fn new(filepath: PathBuf) -> BroadcastServerStorage {
        match fs::read(filepath) {
            Ok(c) => serde_json::from_slice(&c).unwrap(),
            Err(_) => BroadcastServerStorage { database: HashMap::new() }
        }
    }

    pub async fn save_metadata(self, filepath: PathBuf) {

    }
}
