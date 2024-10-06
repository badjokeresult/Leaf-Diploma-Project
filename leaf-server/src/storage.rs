use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::fs;
use dirs;

use consts::*;
use errors::*;

pub mod consts {
    pub const DEFAULT_WORKING_DIR: &str = ".leaf";
    pub const DEFAULT_STORAGE_FILE_NAME: &str = "stor";
    pub const DEFAULT_STORAGE_PATH: &str = "server";
}


type Result<T> = std::result::Result<T, Box<dyn ServerStorageError>>;

pub trait ServerStorage {
    async fn add(&mut self, hash: &[u8], chunk: &[u8]) -> Result<()>;
    async fn get(&self, hash: &[u8]) -> Result<Vec<u8>>;
    async fn pop(&mut self, hash: &[u8]) -> Result<Vec<u8>>;
}

#[derive(Serialize, Deserialize)]
pub struct BroadcastServerStorage {
    database: HashMap<Vec<u8>, Vec<u8>>,
}

impl BroadcastServerStorage {
    pub async fn new() -> Result<BroadcastServerStorage> {
        let default_working_file_path: PathBuf = dirs::home_dir().unwrap()
            .join(DEFAULT_WORKING_DIR)
            .join(DEFAULT_STORAGE_PATH)
            .join(DEFAULT_STORAGE_FILE_NAME);
        Self::from_file(&default_working_file_path).await
    }

    async fn from_file(path: &PathBuf) -> Result<BroadcastServerStorage> {
        let content = match fs::read_to_string(path).await {
            Ok(c) => c,
            Err(_) => return Ok(BroadcastServerStorage {database: HashMap::new()}),
        };

        let storage: Self = match serde_json::from_str(&content) {
            Ok(s) => s,
            Err(e) => return Err(Box::new(FromJsonDeserializationError(e.to_string()))),
        };

        Ok(storage)
    }
}

impl ServerStorage for BroadcastServerStorage {
    async fn add(& mut self, hash: &[u8], chunk: &[u8]) -> Result<()> {
        self.database.insert(hash.to_vec(), chunk.to_vec()).unwrap();
        Ok(())
    }

    async fn get(& self, hash: &[u8]) -> Result<Vec<u8>> {
        Ok(self.database.get(hash).unwrap().clone())
    }

    async fn pop(& mut self, hash: &[u8]) -> Result<Vec<u8>> {
        Ok(self.database.remove(hash).unwrap())
    }
}

mod errors {
    use std::fmt;
    use std::fmt::Formatter;

    pub trait ServerStorageError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result;
    }

    #[derive(Debug, Clone)]
    pub struct ServerDbFromFileInitError(pub String);

    impl ServerStorageError for ServerDbFromFileInitError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error initialization server DB from file: {}", self.0)
        }
    }

    impl fmt::Display for ServerDbFromFileInitError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            ServerStorageError::fmt(self, f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct FromJsonDeserializationError(pub String);

    impl ServerStorageError for FromJsonDeserializationError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error deserialize from JSON into DB: {}", self.0)
        }
    }

    impl fmt::Display for FromJsonDeserializationError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            ServerStorageError::fmt(self, f)
        }
    }
}

#[cfg(test)]
mod tests {

}