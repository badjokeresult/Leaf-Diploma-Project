use std::collections::HashMap;
use std::fs::OpenOptions;
use std::path::PathBuf;

use tokio::fs;
use uuid::Uuid;

#[derive(Clone)]
pub struct BroadcastUdpServerStorage {
    storage_path: PathBuf,
    database: HashMap<Vec<u8>, String>,
}

impl BroadcastUdpServerStorage {
    pub async fn new(storage_path: &PathBuf) -> Self {
        Self {
            storage_path: storage_path.clone(),
            database: Self::from_file(storage_path).await.unwrap_or_else(|e| HashMap::new()),
        }
    }

    async fn from_file(storage_path: &PathBuf) -> Result<HashMap<Vec<u8>, String>, tokio::io::Error> {
        match fs::read(match storage_path.parent() {
            Some(p) => p.join("db.bin"),
            None => return Err(tokio::io::Error::last_os_error()),
        }).await {
            Ok(data) => Ok(serde_json::from_slice(&data)?),
            Err(_) => Err(tokio::io::Error::last_os_error()),
        }
    }

    pub async fn add(&mut self, hash: &[u8], data: &[u8]) -> Result<(), tokio::io::Error> {
        for (h, s) in &self.database {
            if hash.eq(h) {
                let mut f = OpenOptions::new()
                    .append(true)
                    .open(s).await?;
                f.write(data).await?;
                return Ok(());
            }
        }

        let filename = Uuid::new_v4().to_string() + ".dat";
        let filepath = self.storage_path.join(filename).to_str().unwrap().to_string();
        match fs::write(&filepath, data).await {
            Ok(_) => self.database.insert(hash.to_vec(), filepath).unwrap(),
            Err(e) => return Err(tokio::io::Error::new(tokio::io::ErrorKind::Other, format!("{:?}", e))),
        };
        Ok(())
    }

    pub async fn retrieve(&self, hash: &[u8]) -> Result<Vec<u8>, tokio::io::Error> {
        for (h, s) in &self.database {
            if hash.eq(h) {
                return Ok(fs::read(s).await?);
            }
        }
        Err(tokio::io::Error::last_os_error())
    }

    pub async fn shutdown(&self) {
        let json = serde_json::to_vec(&self.database).unwrap();
        fs::write(&self.storage_path.parent().unwrap().join("db.bin"), json).await.unwrap();
    }
}