use std::path::PathBuf;
use std::str;
use std::cell::RefCell;
use std::future::Future;
use std::io::Write;

use tokio::fs;
use kuznechik::{AlgOfb, KeyStore, Kuznechik};

use streebog::digest::consts::U32;
use streebog::digest::core_api::{CoreWrapper, CtVariableCoreWrapper};
use streebog::{Digest, Oid256, StreebogVarCore};

use errors::*;
use crate::FileParts;

struct Credentials {
    pub password: String,
    pub gamma: Vec<u8>,
}

impl Credentials {
    fn new(password: String, gamma: Vec<u8>) -> Credentials {
        Credentials { password, gamma }
    }
}

pub trait Encryptor {
    fn encrypt_parts(&self, parts: &mut FileParts) -> impl std::future::Future<Output = Result<FileParts, DataEncryptionError>> + Send;
    fn decrypt_parts(&self, parts: &mut FileParts) -> impl std::future::Future<Output = Result<FileParts, DataDecryptionError>> + Send;
}

pub struct KuznechikEncryptor {
    password_file: PathBuf,
    gamma_file: PathBuf,
}

impl KuznechikEncryptor {
    pub fn new(password_file: PathBuf, gamma_file: PathBuf) -> Result<KuznechikEncryptor, InitializeEncryptorError> {
        Ok(KuznechikEncryptor {
            password_file,
            gamma_file,
        })
    }

    async fn load_cipher(&self) -> Result<AlgOfb, LoadingCredentialsError> {
        let password = match fs::read(&self.password_file).await {
            Ok(p) => p,
            Err(e) => return Err(LoadingCredentialsError(e.to_string())),
        };
        let gamma = match fs::read(&self.gamma_file).await {
            Ok(g) => g,
            Err(e) => return Err(LoadingCredentialsError(e.to_string())),
        };

        let password = match str::from_utf8(&password) {
            Ok(s) => String::from(s),
            Err(e) => return Err(LoadingCredentialsError(e.to_string())),
        };

        let credentials = Credentials::new(password, gamma);
        let key = KeyStore::with_password(&credentials.password);
        let cipher = AlgOfb::new(&key).gamma(credentials.gamma);
        Ok(cipher)
    }
}

impl Encryptor for KuznechikEncryptor {
    async fn encrypt_parts(&self, parts: FileParts) -> Result<FileParts, DataEncryptionError> {
        let mut cipher = self.load_cipher().await.unwrap();

        let data = parts.get_data_parts();
        let mut data_result = Vec::with_capacity(data.len());
        for part in data {
            if let Some(d) = part {
                let dt = d.clone();
                data_result.push(Some(cipher.encrypt(dt)));
            } else {
                data_result.push(None)
            }
        }

        let rec = parts.get_recovery_parts();
        let mut rec_result = Vec::with_capacity(rec.len());
        for part in rec {
            if let Some(d) = part {
                let dt = d.clone();
                rec_result.push(Some(cipher.encrypt(dt)));
            } else {
                rec_result.push(None);
            }
        }

        let result = FileParts::new(data_result, rec_result);
        Ok(result)
    }

    async fn decrypt_parts(&self, parts: &FileParts) -> Result<FileParts, DataDecryptionError> {
        let mut cipher = self.load_cipher().await.unwrap();

        let data = parts.get_data_parts();
        let mut data_result = Vec::with_capacity(data.len());
        for part in data {
            if let Some(d) = part {
                let dt = d.clone();
                data_result.push(Some(cipher.decrypt(dt)));
            } else {
                data_result.push(None)
            }
        }

        let rec = parts.get_recovery_parts();
        let mut rec_result = Vec::with_capacity(rec.len());
        for part in rec {
            if let Some(d) = part {
                let dt = d.clone();
                rec_result.push(Some(cipher.decrypt(dt)));
            } else {
                rec_result.push(None);
            }
        }

        let result = FileParts::new(data_result, rec_result);
        Ok(result)
    }
}

pub mod errors {
    use std::fmt;
    use std::fmt::Formatter;

    #[derive(Debug, Clone)]
    pub struct InitializeEncryptorError(pub String);

    impl fmt::Display for InitializeEncryptorError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error initialize encryptor: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct LoadingCredentialsError(pub String);

    impl fmt::Display for LoadingCredentialsError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error loading credentials: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct DataEncryptionError(pub String);

    impl fmt::Display for DataEncryptionError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error encrypting data: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct DataDecryptionError(pub String);

    impl fmt::Display for DataDecryptionError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error decrypting data: {}", self.0)
        }
    }
}

pub trait Hasher {
    fn calc_hash_for_parts(&self, parts: &FileParts) -> FileParts;
}

pub struct StreebogHasher {
    hasher: RefCell<CoreWrapper<CtVariableCoreWrapper<StreebogVarCore, U32, Oid256>>>,
}

impl StreebogHasher {
    pub fn new() -> StreebogHasher {
        StreebogHasher {
            hasher: RefCell::new(streebog::Streebog256::new()),
        }
    }
}

impl Hasher for StreebogHasher {
    fn calc_hash_for_parts(&self, parts: &FileParts) -> FileParts {
        let data = parts.get_data_parts();
        let mut data_hashes = Vec::with_capacity(data.len());
        let rec = parts.get_recovery_parts();
        let mut rec_hashes = Vec::with_capacity(rec.len());

        for part in data {
            if let Some(d) = part {
                self.hasher.borrow_mut().update(d);
                let result = self.hasher.borrow_mut().clone().finalize();

                let result = result.to_vec();
                self.hasher.borrow_mut().flush().unwrap();

                data_hashes.push(Some(result));
            } else {
                data_hashes.push(None);
            }
        }

        for part in rec {
            if let Some(d) = part {
                self.hasher.borrow_mut().update(d);
                let result = self.hasher.borrow_mut().clone().finalize();

                let result = result.to_vec();
                self.hasher.borrow_mut().flush().unwrap();

                rec_hashes.push(Some(result));
            } else {
                rec_hashes.push(None);
            }
        }

        let parts = FileParts::new(data_hashes, rec_hashes);
        parts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streebog_calc_hash_for_chunk_successful_calculation() {
        let hasher = StreebogHasher::new();
        let message = b"Hello World";
        let result = vec![102, 108, 244, 251, 247, 78, 198, 138, 102, 232, 221, 61, 48, 97, 176, 51, 117, 104, 206, 33, 161, 4, 84, 29, 77, 238, 3, 245, 68, 140, 41, 175];

        let hash = hasher.calc_hash_for_chunk(message);

        assert_eq!(hash, result);
    }
}
