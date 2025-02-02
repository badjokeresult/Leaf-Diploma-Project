use tokio::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use tokio::fs::OpenOptions;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use common::{Encryptor, Hasher, KuznechikEncryptor, ReedSolomonSecretSharer, SecretSharer, StreebogHasher};

use errors::*;
use crate::socket::{ClientSocket, Socket};

pub trait Parts: Sized {
    async fn from_file(path: impl AsRef<Path>) -> Result<Self, SplittingFileError>;
    async fn encrypt(&mut self, password: &str) -> Result<(), EncryptionError>;
    async fn send_into_domain(&mut self) -> Result<(), SharingChunksError>;
    async fn save_metadata(self) -> Result<(), SavingMetadataError>;
    async fn load_from_metadata(path: impl AsRef<Path>) -> Result<Self, LoadingMetadataError>;
    async fn recv_from_domain(&mut self) -> Result<(), ReceivingChunksError>;
    async fn decrypt(&mut self, password: &str) -> Result<(), DecryptionError>;
    async fn restore_as_file(self) -> Result<(), RestoringFileError>;
}

#[derive(Serialize, Deserialize)]
pub struct FileParts {
    data: Vec<Vec<u8>>,
    recovery: Vec<Vec<u8>>,
    data_hashes: Vec<Vec<u8>>,
    recovery_hashes: Vec<Vec<u8>>,
    path: PathBuf,
}

impl FileParts {
    fn calc_hashes(&mut self) {
        let hasher = StreebogHasher::new();
        let mut data_hashes: Vec<Vec<u8>> = Vec::with_capacity(self.data.len());
        let mut recovery_hashes: Vec<Vec<u8>> = Vec::with_capacity(self.recovery.len());
        for d in &self.data {
            data_hashes.push(hasher.calc_hash_for_chunk(d));
        }
        for r in &self.recovery {
            recovery_hashes.push(hasher.calc_hash_for_chunk(r));
        }

        self.data_hashes = data_hashes;
        self.recovery_hashes = recovery_hashes;
    }
}

impl Parts for FileParts {
    async fn from_file(path: impl AsRef<Path>) -> Result<Self, SplittingFileError> {
        let sharer = ReedSolomonSecretSharer::new().unwrap();
        let mut file = OpenOptions::new()
            .read(true)
            .write(false)
            .truncate(false)
            .open(&path).await.unwrap();
        let mut buf = [0u8; 4 * 1024 * 1024 * 1024];
        let sz = file.read(&mut buf).await.unwrap();
        let chunks = sharer.split_into_chunks(&buf[..sz]).unwrap();
        file.set_len(0).await.unwrap();

        let (data, recovery) = chunks.split_at(chunks.len() / 2);
        Ok(Self {
            data: data.to_vec(),
            recovery: recovery.to_vec(),
            data_hashes: Vec::new(),
            recovery_hashes: Vec::new(),
            path: path.as_ref().to_owned(),
        })
    }

    async fn encrypt(&mut self, password: &str) -> Result<(), EncryptionError> {
        let encryptor = KuznechikEncryptor::new(password).await.unwrap();
        for d in &mut self.data {
            encryptor.encrypt_chunk(d).unwrap();
        }
        for r in &mut self.recovery {
            encryptor.encrypt_chunk(r).unwrap();
        }

        Ok(())
    }

    async fn send_into_domain(&mut self) -> Result<(), SharingChunksError> {
        if self.recovery.len() % self.data.len() != 0 {
            return Err(SharingChunksError(format!("Data = {}, Recovery = {}", self.data.len(), self.recovery.len())));
        }

        self.calc_hashes();
        let socket = ClientSocket::new().await.unwrap();
        for i in 0..self.data.len() {
            socket.send(&self.data_hashes[i], &self.data[i]).await.unwrap();
        }
        for i in 0..self.recovery.len() {
            socket.send(&self.recovery_hashes[i], &self.recovery[i]).await.unwrap();
        }

        self.data.clear();
        self.recovery.clear();

        Ok(())
    }

    async fn save_metadata(self) -> Result<(), SavingMetadataError> {
        let json = serde_json::to_vec(&self).unwrap();
        fs::write(&self.path, &json).await.unwrap();
        Ok(())
    }

    async fn load_from_metadata(path: impl AsRef<Path>) -> Result<Self, LoadingMetadataError> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(false)
            .truncate(false)
            .open(path).await.unwrap();
        let mut buf = [0u8; 4 * 1024 * 1024 * 1024];
        let sz = file.read(&mut buf).await.unwrap();
        let parts = serde_json::from_slice(&buf[..sz]).unwrap();
        Ok(parts)
    }

    async fn recv_from_domain(&mut self) -> Result<(), ReceivingChunksError> {
        let socket = ClientSocket::new().await.unwrap();
        for i in 0..self.data_hashes.len() {
            if let Ok(d) = socket.recv(&self.data_hashes[i]).await {
                self.data.push(d);
                self.recovery.push(Vec::new());
            } else {
                if let Ok(r) = socket.recv(&self.recovery_hashes[i]).await {
                    self.recovery.push(r);
                    self.data.push(Vec::new());
                } else {
                    return Err(ReceivingChunksError(format!("Cannot retrieve data {:?} and recovery {:?}", &self.data_hashes[i], &self.recovery_hashes[i])));
                }
            }
        }
        Ok(())
    }

    async fn decrypt(&mut self, password: &str) -> Result<(), DecryptionError> {
        let decryptor = KuznechikEncryptor::new(password).await.unwrap();
        for d in &mut self.data {
            decryptor.decrypt_chunk(d).unwrap();
        }
        for r in &mut self.recovery {
            decryptor.decrypt_chunk(r).unwrap();
        }

        Ok(())
    }

    async fn restore_as_file(mut self) -> Result<(), RestoringFileError> {
        let sharer = ReedSolomonSecretSharer::new().unwrap();
        let mut file = OpenOptions::new()
        .read(false)
        .write(true)
        .truncate(true)
            .open(self.path)
            .await.unwrap();
        self.data.append(&mut self.recovery);
        let content = sharer.recover_from_chunks(self.data).unwrap();
        file.write_all(&content).await.unwrap();
        Ok(())
    }
}

mod errors {
    use std::fmt;
    use std::fmt::Formatter;

    #[derive(Debug, Clone)]
    pub struct LengthMismatchError(pub String);

    impl fmt::Display for LengthMismatchError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Length of data and recovery mismatched: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct SplittingFileError(pub String);

    impl fmt::Display for SplittingFileError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error splitting file into chunks: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct EncryptionError(pub String);

    impl fmt::Display for EncryptionError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error during an attempt to encrypt chunks: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct SharingChunksError(pub String);

    impl fmt::Display for SharingChunksError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error during a sending chunk into domain: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct SavingMetadataError(pub String);

    impl fmt::Display for SavingMetadataError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error during saving metadata: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct LoadingMetadataError(pub String);

    impl fmt::Display for LoadingMetadataError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error during loading metadata: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct ReceivingChunksError(pub String);

    impl fmt::Display for ReceivingChunksError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error during receiving chunks: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct DecryptionError(pub String);

    impl fmt::Display for DecryptionError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error during an attempt to decrypt chunks: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct RestoringFileError(pub String);

    impl fmt::Display for RestoringFileError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error during restoring a file: {}", self.0)
        }
    }
}
