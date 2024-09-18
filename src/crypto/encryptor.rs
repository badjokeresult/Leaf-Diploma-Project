use std::fmt::Debug;
use std::str;

use aes_gcm::{aead::{Aead, AeadCore, KeyInit, OsRng}, Aes256Gcm, Key, Nonce};
use kuznechik::{AlgOfb, KeyStore, Kuznechik};

use super::errors::*;
use super::initialization::*;

type Result<T> = std::result::Result<T, Box<dyn CryptoModuleError>>;

pub trait Encryptor {
    async fn encrypt_chunk(&self, chunk: &[u8]) -> Vec<u8>;
    async fn decrypt_chunk(&self, chunk: &[u8]) -> Vec<u8>;
}

pub struct Aes256Encryptor {
    password_file: PasswordFilePathWrapper,
}

impl Aes256Encryptor {
    pub fn new() -> Result<Aes256Encryptor> {
        let encryptor = Aes256Encryptor {
            password_file: match PasswordFilePathWrapper::new() {
                Ok(p) => p,
                Err(e) => return Err(e),
            },
        };
        Ok(encryptor)
    }
}

impl Encryptor for Aes256Encryptor {
    async fn encrypt_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>> {
        let passwd = self.password_file.load_passwd().await?;
        let key = Key::<Aes256Gcm>::from_slice(&passwd);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let cipher = Aes256Gcm::new(key);

        let ciphered_data = match cipher.encrypt(&nonce, chunk) {
            Ok(d) => d,
            Err(e) => return Err(Box::new(DataEncryptionError(e.to_string()))),
        };

        let mut encrypted_data = nonce.to_vec();
        encrypted_data.extend_from_slice(&ciphered_data);

        Ok(encrypted_data)
    }

    async fn decrypt_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>> {
        let passwd = self.password_file.load_passwd().await?;
        let key = Key::<Aes256Gcm>::from_slice(&passwd);
        let (nonce_arr, ciphered_data) = chunk.split_at(12);
        let nonce = Nonce::from_slice(nonce_arr);

        let cipher = Aes256Gcm::new(key);

        let decrypted_chunk = match cipher.decrypt(nonce, ciphered_data) {
            Ok(d) => d,
            Err(e) => return Err(Box::new(DataDecryptionError(e.to_string()))),
        };

        Ok(decrypted_chunk)
    }
}

pub struct KuznechikEncryptor {
    password_file: PasswordFilePathWrapper,
    gamma_file: GammaFilePathWrapper,
}

impl KuznechikEncryptor {
    pub fn new() -> Result<KuznechikEncryptor> {
        let encryptor = KuznechikEncryptor {
            password_file: match PasswordFilePathWrapper::new() {
                Ok(p) => p,
                Err(e) => return Err(e),
            },
            gamma_file: match GammaFilePathWrapper::new() {
                Ok(g) => g,
                Err(e) => return Err(e),
            },
        };
        Ok(encryptor)
    }
}

impl Encryptor for KuznechikEncryptor {
    async fn encrypt_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>> {
        let mut cipher = self.generate_key().await?;

        let data = Vec::from(chunk);
        let encrypted_chunk = cipher.encrypt(data);

        Ok(encrypted_chunk)
    }

    async fn decrypt_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>> {
        let mut cipher = self.generate_key().await?;

        let data = Vec::from(chunk);
        let decrypted_chunk = cipher.decrypt(data);

        Ok(decrypted_chunk)
    }
}

impl KuznechikEncryptor {
    async fn generate_key(&self) -> Result<AlgOfb> {
        let password = self.password_file.load_passwd().await?;
        let gamma = self.gamma_file.load_gamma().await?;

        let password_str = match str::from_utf8(&password) {
            Ok(s) => s,
            Err(e) => return Err(Box::new(PasswordFromUtf8Error(e.to_string()))),
        };

        let key = KeyStore::with_password(password_str);
        let cipher = AlgOfb::new(&key).gamma(gamma.to_vec());

        Ok(cipher)
    }
}
