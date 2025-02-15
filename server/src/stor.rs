use common::{Hasher, StreebogHasher};
use consts::*;
use errors::*;
use std::path::PathBuf;
use tokio::fs;
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

pub struct UdpServerStorage {
    //database: Arc<RwLock<HashMap<String, PathBuf>>>,
    hasher: StreebogHasher,
    path: PathBuf,
}

impl UdpServerStorage {
    pub fn new(path: PathBuf) -> UdpServerStorage {
        UdpServerStorage {
            //database: Arc::new(RwLock::new(HashMap::new())),
            hasher: StreebogHasher::new(),
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

    async fn search_for_hash(&self, hash: &str) -> Result<(PathBuf, Vec<u8>), RetrievingDataError> {
        for entry in WalkDir::new(&self.path) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => return Err(RetrievingDataError(format!("{:?}", e))),
            };
            if entry.path().is_file() {
                let content = fs::read(entry.path()).await.unwrap();
                let h = self.hasher.calc_hash_for_chunk(&content);
                if hash.eq(&h) {
                    return Ok((PathBuf::from(entry.path()), content));
                }
            }
        }
        Err(RetrievingDataError(format!("hash not found: {}", hash)))
    }
}

impl ServerStorage for UdpServerStorage {
    async fn save(&self, hash: &str, data: &[u8]) -> Result<(), SavingDataError> {
        let hash = String::from(hash);

        if let Ok(_) = self.search_for_hash(&hash).await {
            println!("Hash already present: {}", hash);
            return Err(SavingDataError(String::from("Hash already presents file")));
        }

        let filename = self.path.join(format!("{}.bin", Uuid::new_v4()));
        println!("Saving file with hash: {}", hash);
        fs::write(&filename, data)
            .await
            .map_err(|e| SavingDataError(e.to_string()))?;

        //db.insert(hash, filename);
        Ok(())
    }

    async fn get(&self, hash: &str) -> Result<Vec<u8>, RetrievingDataError> {
        // for key in self.database.read().await.keys() {
        //     println!("{:#?}", key);
        // }
        //let mut db: RwLockWriteGuard<'_, HashMap<String, PathBuf>> = self.database.write().await;
        if let Ok((p, c)) = self.search_for_hash(hash).await {
            fs::remove_file(&p).await.unwrap();
            return Ok(c);
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
