use std::path::PathBuf;
use std::str;
use std::cell::RefCell;
use std::io::Write;

use tokio::fs;
use kuznechik::{AlgOfb, KeyStore, Kuznechik};

use streebog::digest::consts::U32;
use streebog::digest::core_api::{CoreWrapper, CtVariableCoreWrapper};
use streebog::{Digest, Oid256, StreebogVarCore};

use errors::*;

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
    fn encrypt_chunk(&self, chunk: &[u8]) -> impl std::future::Future<Output = Result<Vec<u8>, DataEncryptionError>> + Send;
    fn decrypt_chunk(&self, chunk: &[u8]) -> impl std::future::Future<Output = Result<Vec<u8>, DataDecryptionError>> + Send;
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

    async fn load_passwd_gamma(&self) -> Result<Credentials, LoadingCredentialsError> {
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

        Ok(Credentials::new(password, gamma))
    }
}

impl Encryptor for KuznechikEncryptor {
    async fn encrypt_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>, DataEncryptionError> {
        let credentials = match self.load_passwd_gamma().await {
            Ok(c) => c,
            Err(e) => return Err(DataEncryptionError(e.to_string())),
        };

        let key = KeyStore::with_password(&credentials.password);
        let mut cipher = AlgOfb::new(&key).gamma(credentials.gamma);

        let data = Vec::from(chunk);
        let encrypted_chunk = cipher.encrypt(data);

        Ok(encrypted_chunk)
    }

    async fn decrypt_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>, DataDecryptionError> {
        let credentials = match self.load_passwd_gamma().await {
            Ok(c) => c,
            Err(e) => return Err(DataDecryptionError(e.to_string())),
        };

        let key = KeyStore::with_password(&credentials.password);
        let mut cipher = AlgOfb::new(&key).gamma(credentials.gamma);

        let data = Vec::from(chunk);
        let decrypted_chunk = cipher.decrypt(data);

        Ok(decrypted_chunk)
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
    fn calc_hash_for_chunk(&self, chunk: &[u8]) -> Vec<u8>;
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
    fn calc_hash_for_chunk(&self, chunk: &[u8]) -> Vec<u8> {
        self.hasher.borrow_mut().update(chunk);
        let result = self.hasher.borrow_mut().clone().finalize();

        let result = result.to_vec();
        self.hasher.borrow_mut().flush().unwrap();

        result
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
