use std::path::PathBuf;

use tokio::fs;

use serde::{Deserialize, Serialize};
use common::{Encryptor, KuznechikEncryptor, ReedSolomonSecretSharer, SecretSharer};

use crate::hashes::{FileHashes, Hashes};

pub trait Parts {
    async fn from_file(path: PathBuf) -> Self;
    async fn encrypt(&mut self, password: &str);
    async fn calc_hashes(&mut self);
    async fn decrypt(&mut self, password: &str);
    async fn to_file(self);
    fn len(&self) -> usize;
}

#[derive(Serialize, Deserialize)]
pub struct FileParts {
    data: Vec<Vec<u8>>,
    recovery: Vec<Vec<u8>>,
    filepath: PathBuf,
    hashes: Option<FileHashes>,
}

impl Parts for FileParts {
    async fn from_file(path: PathBuf) -> Self {
        let content = fs::read(&path).await.unwrap();
        let sharer: Box<dyn SecretSharer> = Box::new(ReedSolomonSecretSharer::new().unwrap());
        let chunks = sharer.split_into_chunks(&content).unwrap();
        let (data, recovery) = chunks.split_at(content.len() / 2);
        let data = data.to_vec();
        let recovery = recovery.to_vec();

        FileParts { data, recovery, filepath: path, hashes: None }
    }

    async fn encrypt(&mut self, password: &str) {
        let encryptor: Box<dyn Encryptor> = Box::new(KuznechikEncryptor::new(password).await.unwrap());

        for d in self.data.iter_mut() {
            encryptor.encrypt_chunk(d).unwrap();
        }
        for r in self.recovery.iter_mut() {
            encryptor.encrypt_chunk(r).unwrap();
        }
    }

    async fn calc_hashes(&mut self) {
        let hashes = FileHashes::new(&self.data, &self.recovery);
        self.hashes = Some(hashes);
    }

    async fn decrypt(&mut self, password: &str) {
        let decryptor: Box<dyn Encryptor> = Box::new(KuznechikEncryptor::new(password).await.unwrap());

        for d in self.data.iter_mut() {
            decryptor.decrypt_chunk(d).unwrap();
        }
        for r in self.recovery.iter_mut() {
            decryptor.decrypt_chunk(r).unwrap();
        }
    }

    async fn to_file(self) {
        let sharer: Box<dyn SecretSharer> = Box::new(ReedSolomonSecretSharer::new().unwrap());
        let mut data = self.data;
        let mut recovery = self.recovery;
        data.append(&mut recovery);

        let content = sharer.recover_from_chunks(data).unwrap();
        fs::write(&self.filepath, &content).await.unwrap();
    }

    fn len(&self) -> usize {
        self.data.len()
    }
}

impl From<String> for FileParts {
    fn from(s: String) -> Self {
        let obj = serde_json::from_str(&s).unwrap();
        obj
    }
}

impl Into<String> for FileParts {
    fn into(self) -> String {
        serde_json::to_string_pretty(&self).unwrap()
    }
}
