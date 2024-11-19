use std::collections::HashMap;
use std::path::PathBuf;

use atomic_refcell::AtomicRefCell;

use tokio::io::AsyncWriteExt;
use tokio::fs::OpenOptions;
use tokio::fs;

use uuid::Uuid;

use errors::*;

pub trait UdpStorage {
    fn add(&self, hash: &[u8], data: &[u8]) -> impl std::future::Future<Output = Result<(), AddChunkError>> + Send;
    fn retrieve(&self, hash: &[u8]) -> impl std::future::Future<Output = Result<Vec<u8>, RetrieveChunkError>> + Send;
    fn shutdown(&self) -> impl std::future::Future<Output = Result<(), ShutdownError>> + Send;
}

#[derive(Clone)]
pub struct BroadcastUdpServerStorage {
    storage_path: PathBuf,
    database: AtomicRefCell<HashMap<Vec<u8>, String>>,
}

impl BroadcastUdpServerStorage {
    pub async fn new(storage_path: &PathBuf) -> Result<BroadcastUdpServerStorage, StorageInitError> {
        Ok(BroadcastUdpServerStorage {
            storage_path: storage_path.clone(),
            database: Self::from_file(storage_path).await.unwrap_or_else(|_| AtomicRefCell::new(HashMap::new())),
        })
    }

    async fn from_file(storage_path: &PathBuf) -> Result<AtomicRefCell<HashMap<Vec<u8>, String>>, FromFileInitError> {
        let data = match fs::read(match storage_path.parent() {
            Some(p) => p.join("db.bin"),
            None => return Err(FromFileInitError(String::from("Error getting working dir"))),
        }).await {
            Ok(data) => data,
            Err(e) => return Err(FromFileInitError(e.to_string())),
        };

        let kv: HashMap<Vec<u8>, String> = serde_json::from_slice(&data).unwrap();
        Ok(AtomicRefCell::new(kv))
    }
}

impl UdpStorage for BroadcastUdpServerStorage {
    async fn add(&self, hash: &[u8], data: &[u8]) -> Result<(), AddChunkError> {
        if let None = self.database.borrow().get(hash) {
            let filename = Uuid::new_v4().to_string() + ".dat";
            let filepath = Some(self.storage_path.join(filename));
            self.database.borrow_mut().insert(hash.to_vec(), filepath.unwrap().to_str().unwrap().to_string());
        }
        let mut f = match OpenOptions::new()
            .append(true)
            .open(self.database.borrow().get(hash).unwrap()).await {
                Ok(t) => t,
                Err(e) => return Err(AddChunkError(e.to_string()))
            };
        match f.write(data).await {
            Ok(_) => Ok(()),
            Err(e) => Err(AddChunkError(e.to_string())),
        }
    }

    async fn retrieve(&self, hash: &[u8]) -> Result<Vec<u8>, RetrieveChunkError> {
        if let Some(f) = self.database.borrow().get(hash) {
            return match fs::read(f).await {
                Ok(data) => Ok(data),
                Err(e) => Err(RetrieveChunkError(e.to_string())),
            }
        }
        Err(RetrieveChunkError(String::from("No chunk with such hash")))
    }

    async fn shutdown(&self) -> Result<(), ShutdownError> {
        let json = match serde_json::to_vec(&self.database.borrow().clone()) {
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
