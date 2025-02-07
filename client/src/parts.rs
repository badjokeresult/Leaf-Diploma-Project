use std::hash::Hash;
use tokio::fs;
use std::path::Path;
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
    async fn save_metadata(self, path: impl AsRef<Path>, other: Self) -> Result<(), SavingMetadataError>;
    async fn load_from_metadata(path: impl AsRef<Path>) -> Result<(Self, Self), LoadingMetadataError>;
    async fn recv_from_domain(&mut self) -> Result<(), ReceivingChunksError>;
    async fn decrypt(&mut self, password: &str) -> Result<(), DecryptionError>;
    async fn restore_as_file(self, path: impl AsRef<Path>, other: Self) -> Result<(), RestoringFileError>;
    fn len(&self) -> usize;
    fn get_data_cloned(&self) -> Vec<Vec<u8>>;
    fn get_hashes_cloned(&self) -> Vec<Vec<u8>>;
}

#[derive(Serialize, Deserialize)]
pub struct FileParts {
    data: Vec<Vec<u8>>,
    hashes: Vec<Vec<u8>>,
}

impl FileParts {
    fn new(data: Vec<Vec<u8>>, hashes: Vec<Vec<u8>>) -> FileParts {
        FileParts { data, hashes }
    }
    fn calc_hashes(&mut self) {
        let hasher = StreebogHasher::new();
        let mut data_hashes: Vec<Vec<u8>> = Vec::with_capacity(self.data.len());
        for d in &self.data {
            data_hashes.push(hasher.calc_hash_for_chunk(d));
        }
        self.hashes = data_hashes;
    }
}

impl Parts for FileParts {
    async fn from_file(path: impl AsRef<Path>) -> Result<(Self, Self), SplittingFileError> {
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
        Ok((Self {
            data: data.to_vec(),
            hashes: Vec::with_capacity(data.len()),
        }, Self {
            data: recovery.to_vec(),
            hashes: Vec::with_capacity(recovery.len()),
        }))
    }

    async fn encrypt(&mut self, password: &str) -> Result<(), EncryptionError> {
        let encryptor = KuznechikEncryptor::new(password).await.unwrap();
        for d in &mut self.data {
            encryptor.encrypt_chunk(d).unwrap();
        }

        Ok(())
    }

    async fn send_into_domain(&mut self) -> Result<(), SharingChunksError> {
        self.calc_hashes();

        let socket = ClientSocket::new().await.unwrap();
        socket.send(&mut self).await.unwrap();
        self.data.clear();

        Ok(())
    }

    async fn save_metadata(mut self, path: impl AsRef<Path>, mut other: Self) -> Result<(), SavingMetadataError> {
        self.data.append(&mut other.data);
        self.hashes.append(&mut other.hashes);

        let json = serde_json::to_vec(&self).unwrap();
        fs::write(path, &json).await.unwrap();
        Ok(())
    }

    async fn load_from_metadata(path: impl AsRef<Path>) -> Result<(Self, Self), LoadingMetadataError> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(false)
            .truncate(false)
            .open(path).await.unwrap();
        let mut buf = [0u8; 4 * 1024 * 1024 * 1024];
        let sz = file.read(&mut buf).await.unwrap();
        let parts: FileParts = serde_json::from_slice(&buf[..sz]).unwrap();

        let ((data, recovery), (data_hashes, recovery_hashes)) = (parts.data.split_at(parts.data.len() / 2), parts.hashes.split_at(parts.hashes.len() / 2));
        let data_parts = FileParts::new(data.to_vec(), data_hashes.to_vec());
        let recovery_parts = FileParts::new(recovery.to_vec(), recovery_hashes.to_vec());

        Ok((data_parts, recovery_parts))
    }

    async fn recv_from_domain(&mut self) -> Result<(), ReceivingChunksError> {
        let socket = ClientSocket::new().await.unwrap();
        socket.recv(&mut self).await.unwrap();
        Ok(())
    }

    async fn decrypt(&mut self, password: &str) -> Result<(), DecryptionError> {
        let decryptor = KuznechikEncryptor::new(password).await.unwrap();
        for d in &mut self.data {
            decryptor.decrypt_chunk(d).unwrap();
        }

        Ok(())
    }

    async fn restore_as_file(mut self, path: impl AsRef<Path>, mut other: Self) -> Result<(), RestoringFileError> {
        let sharer = ReedSolomonSecretSharer::new().unwrap();
        let mut file = OpenOptions::new()
        .read(false)
        .write(true)
        .truncate(true)
        .open(path)
        .await.unwrap();
        self.data.append(&mut other.data);
        let content = sharer.recover_from_chunks(self.data).unwrap();
        file.write_all(&content).await.unwrap();
        Ok(())
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn get_data_cloned(&self) -> Vec<Vec<u8>> {
        self.data.clone()
    }

    fn get_hashes_cloned(&self) -> Vec<Vec<u8>> {
        self.hashes.clone()
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
