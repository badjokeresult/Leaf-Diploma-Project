use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

use tokio::fs;
use uuid::Uuid;
use walkdir::WalkDir;

use errors::*;

pub trait ServerStorage {
    async fn save(&self, hash: &[u8], data: &[u8]) -> Result<(), SavingDataError>;
    async fn get(&self, hash: &[u8]) -> Result<Vec<u8>, RetrievingDataError>;
    async fn get_occupied_space(&self) -> Result<usize, RetrievingDataError>;
}

pub struct UdpServerStorage {
    database: RefCell<HashMap<Vec<u8>, PathBuf>>,
    path: PathBuf,
}

impl UdpServerStorage {
    pub fn new(path: PathBuf) -> UdpServerStorage {
        UdpServerStorage {
            database: RefCell::new(HashMap::new()),
            path,
        }
    }
}

impl ServerStorage for UdpServerStorage {
    async fn save(&self, hash: &[u8], data: &[u8]) -> Result<(), SavingDataError> {
        let filename = self.path.join(PathBuf::from(Uuid::new_v4().to_string()));
        fs::write(&filename, data).await.unwrap();
        if let Some(_) = self.database.borrow_mut().insert(hash.to_vec(), filename) {
            return Ok(());
        }
        Err(SavingDataError(format!("{:?}", hash)))
    }

    async fn get(&self, hash: &[u8]) -> Result<Vec<u8>, RetrievingDataError> {
        if let Some(x) = self.database.borrow_mut().remove(hash) {
            let data = fs::read(x).await.unwrap();
            return Ok(data);
        }
        Err(RetrievingDataError(String::from("No data for such hash sum")))
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
