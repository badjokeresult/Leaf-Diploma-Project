use consts::*;
use errors::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::{RwLock, RwLockWriteGuard};
use uuid::Uuid;
use walkdir::WalkDir;

mod consts {
    pub const MAX_OCCUPIED_SPACE: usize = 10 * 1024 * 1024 * 1024;
}

pub trait ServerStorage {
    async fn save(&self, hash: &str, data: &[u8]) -> Result<(), SavingDataError>;
    async fn get(&self, hash: &str) -> Result<Vec<u8>, RetrievingDataError>;
    async fn can_save(&self) -> bool;
}

#[derive(Clone)]
pub struct UdpServerStorage {
    database: Arc<RwLock<HashMap<String, PathBuf>>>,
    path: PathBuf,
}

impl UdpServerStorage {
    pub fn new(path: PathBuf) -> UdpServerStorage {
        UdpServerStorage {
            database: Arc::new(RwLock::new(HashMap::new())),
            path,
        }
    }

    async fn get_occupied_space(&self) -> Result<usize, RetrievingDataError> {
        let mut size = 0;
        for entry in WalkDir::new(&self.path) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => return Err(RetrievingDataError(format!("{:?}", e))),
            };
            if entry.path().is_file() {
                if let Ok(meta) = entry.path().metadata() {
                    size += meta.len() as usize;
                }
            }
        }
        Ok(size)
    }
}

impl ServerStorage for UdpServerStorage {
    async fn save(&self, hash: &str, data: &[u8]) -> Result<(), SavingDataError> {
        let hash = String::from(hash);

        let mut db = self.database.write().await;

        if db.contains_key(&hash) {
            println!("Hash already present: {}", hash); // Логирование хэш-суммы
            return Err(SavingDataError(format!(
                "Hash already presents file {:#?}",
                db.get(&hash).unwrap()
            )));
        }

        let filename = self.path.join(format!("{}.bin", Uuid::new_v4()));
        println!("Saving file with hash: {}", hash); // Логирование хэш-суммы
        tokio::fs::write(&filename, data)
            .await
            .map_err(|e| SavingDataError(e.to_string()))?;

        db.insert(hash, filename);
        Ok(())
    }

    async fn get(&self, hash: &str) -> Result<Vec<u8>, RetrievingDataError> {
        for key in self.database.read().await.keys() {
            println!("{:#?}", key);
        }
        let mut db: RwLockWriteGuard<'_, HashMap<String, PathBuf>> = self.database.write().await;
        if let Some(x) = db.remove(hash) {
            let data = fs::read(x).await.unwrap();
            return Ok(data);
        }
        Err(RetrievingDataError(String::from(
            "No data for such hash sum",
        )))
    }

    async fn can_save(&self) -> bool {
        self.get_occupied_space().await.unwrap() < MAX_OCCUPIED_SPACE
    }
}

mod errors {
    use std::fmt;

    #[derive(Debug, Clone)]
    pub struct SavingDataError(pub String);

    impl fmt::Display for SavingDataError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "Error saving data: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct RetrievingDataError(pub String);

    impl fmt::Display for RetrievingDataError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Error retrieving data: {}", self.0)
        }
    }
}
