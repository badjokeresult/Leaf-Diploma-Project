use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use dirs;

use consts::*;
use errors::*;

pub mod consts {
    pub const DEFAULT_WORKING_DIR: &str = ".leaf";
    pub const DEFAULT_STORAGE_FILE_NAME: &str = "stor";
    pub const DEFAULT_STORAGE_PATH: &str = "server";
}

pub trait ServerStorage {
    fn add(&mut self, hash: &[u8], chunk: &[u8]);
    fn get(&self, hash: &[u8]) -> Result<Vec<u8>, RetrieveFromStorageError>;
    fn pop(&mut self, hash: &[u8]) -> Result<Vec<u8>, PoppingFromStorageError>;
}

#[derive(Serialize, Deserialize)]
pub struct BroadcastServerStorage {
    database: HashMap<Vec<u8>, Vec<u8>>,
}

impl BroadcastServerStorage {
    pub fn new() -> Result<BroadcastServerStorage, ServerPeerInitializingError> {
        let default_working_file_path: PathBuf = dirs::home_dir().unwrap()
            .join(DEFAULT_WORKING_DIR)
            .join(DEFAULT_STORAGE_PATH)
            .join(DEFAULT_STORAGE_FILE_NAME);
        match Self::from_file(&default_working_file_path) {
            Ok(s) => Ok(s),
            Err(e) => Err(ServerPeerInitializingError(e.to_string())),
        }
    }

    fn from_file(path: &PathBuf) -> Result<BroadcastServerStorage, FromFileInitializingError> {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Ok(BroadcastServerStorage {database: HashMap::new()}),
        };

        let storage: Self = match serde_json::from_str(&content) {
            Ok(s) => s,
            Err(e) => return Err(FromFileInitializingError(e.to_string())),
        };

        Ok(storage)
    }
}

impl ServerStorage for BroadcastServerStorage {
    fn add(&mut self, hash: &[u8], chunk: &[u8]) {
        self.database.insert(hash.to_vec(), chunk.to_vec());
    }

    fn get(&self, hash: &[u8]) -> Result<Vec<u8>, RetrieveFromStorageError> {
        match self.database.get(hash) {
            Some(d) => Ok(d.clone()),
            None => Err(RetrieveFromStorageError(format!("{:?}", hash))),
        }
    }

    fn pop(&mut self, hash: &[u8]) -> Result<Vec<u8>, PoppingFromStorageError> {
        match self.database.remove(hash) {
            Some(d) => Ok(d),
            None => Err(PoppingFromStorageError(format!("{:?}", hash)))
        }
    }
}

mod errors {
    use std::fmt;
    use std::fmt::Formatter;

    #[derive(Debug, Clone)]
    pub struct ServerPeerInitializingError(pub String);

    impl fmt::Display for ServerPeerInitializingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error initializing server peer: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct FromFileInitializingError(pub String);

    impl fmt::Display for FromFileInitializingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error initializing server peer from file: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct RetrieveFromStorageError(pub String);

    impl fmt::Display for RetrieveFromStorageError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error retrieving from storage: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct PoppingFromStorageError(pub String);

    impl fmt::Display for PoppingFromStorageError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error popping data from storage: {}", self.0)
        }
    }
}

#[cfg(test)]
mod tests {

}