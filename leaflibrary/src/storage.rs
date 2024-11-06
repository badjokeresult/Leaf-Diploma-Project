use std::collections::HashMap;
use std::path::PathBuf;

use tokio::io::AsyncWriteExt;
use tokio::fs::OpenOptions;
use tokio::fs;

use uuid::Uuid;

use errors::*;

pub trait UdpStorage {
    fn add(&mut self, hash: &[u8], data: &[u8]) -> impl std::future::Future<Output = Result<(), AddChunkError>> + Send;
    fn retrieve(&self, hash: &[u8]) -> impl std::future::Future<Output = Result<Vec<u8>, RetrieveChunkError>> + Send;
    fn shutdown(&self) -> impl std::future::Future<Output = Result<(), ShutdownError>> + Send;
}

#[derive(Clone)]
pub struct BroadcastUdpServerStorage {
    storage_path: PathBuf,
    database: HashMap<Vec<u8>, String>,
}

impl BroadcastUdpServerStorage {
    pub async fn new(storage_path: &PathBuf) -> Result<BroadcastUdpServerStorage, StorageInitError> {
        Ok(BroadcastUdpServerStorage {
            storage_path: storage_path.clone(),
            database: Self::from_file(storage_path).await.unwrap_or_else(|_| HashMap::new()),
        })
    }

    async fn from_file(storage_path: &PathBuf) -> Result<HashMap<Vec<u8>, String>, FromFileInitError> {
        match fs::read(match storage_path.parent() {
            Some(p) => p.join("db.bin"),
            None => return Err(FromFileInitError(String::from("Error getting working dir"))),
        }).await {
            Ok(data) => Ok(match serde_json::from_slice(&data) {
                Ok(j) => j,
                Err(e) => return Err(FromFileInitError(e.to_string())),
            }),
            Err(e) => Err(FromFileInitError(e.to_string())),
        }
    }
}

impl UdpStorage for BroadcastUdpServerStorage {
    async fn add(&mut self, hash: &[u8], data: &[u8]) -> Result<(), AddChunkError> {
        for (h, s) in &self.database {
            if hash.eq(h) {
                let mut f = match OpenOptions::new()
                    .append(true)
                    .open(s).await {
                    Ok(t) => t,
                    Err(e) => return Err(AddChunkError(e.to_string())),
                };
                match f.write(data).await {
                    Ok(_) => {},
                    Err(e) => return Err(AddChunkError(e.to_string())),
                };
                return Ok(());
            }
        }

        let filename = Uuid::new_v4().to_string() + ".dat";
        let filepath = self.storage_path.join(filename).to_str().unwrap().to_string();
        match fs::write(&filepath, data).await {
            Ok(_) => self.database.insert(hash.to_vec(), filepath),
            Err(e) => return Err(AddChunkError(e.to_string())),
        };
        Ok(())
    }

    async fn retrieve(&self, hash: &[u8]) -> Result<Vec<u8>, RetrieveChunkError> {
        for (h, s) in &self.database {
            if h.eq(hash) {
                return Ok(match fs::read(s).await {
                    Ok(d) => d,
                    Err(e) => return Err(RetrieveChunkError(e.to_string())),
                });
            }
        }
        Err(RetrieveChunkError(String::from("Chunk with such hash not found")))
    }

    async fn shutdown(&self) -> Result<(), ShutdownError> {
        let json = match serde_json::to_vec(&self.database) {
            Ok(j) => j,
            Err(e) => return Err(ShutdownError(e.to_string())),
        };
        match fs::write(match self.storage_path.parent() {
            Some(p) => p.join("db.bin"),
            None => return Err(ShutdownError(String::from("Error getting working dir"))),
        }, json).await {
            Ok(_) => {},
            Err(e) => return Err(ShutdownError(e.to_string())),
        };
        Ok(())
    }
}

mod errors {
    use std::fmt;
    use std::fmt::Formatter;

    #[derive(Debug, Clone)]
    pub struct AddChunkError(pub String);

    impl fmt::Display for AddChunkError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error adding chunk: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct RetrieveChunkError(pub String);

    impl fmt::Display for RetrieveChunkError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error retrieving chunk: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct FromFileInitError(pub String);

    impl fmt::Display for FromFileInitError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error init storage from file: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct StorageInitError(pub String);

    impl fmt::Display for StorageInitError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error init storage: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct ShutdownError(pub String);

    impl fmt::Display for ShutdownError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error shutdown storage: {}", self.0)
        }
    }
}
