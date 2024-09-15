use std::fmt::Debug;

use aes_gcm::{aead::{Aead, AeadCore, KeyInit, OsRng}, Aes256Gcm, Key, Nonce};

use crate::crypto::errors::{CryptoModuleError, DataDecryptionError, DataEncryptionError};
use super::initialization::PasswordFilePathWrapper;
use super::encryptor::Encryptor;

pub struct Aes256Encryptor {
    password_file: PasswordFilePathWrapper,
}

impl Aes256Encryptor {
    pub fn new() -> Aes256Encryptor {
        Aes256Encryptor {
            password_file: PasswordFilePathWrapper::new(),
        }
    }
}

impl Encryptor for Aes256Encryptor {
    async fn encrypt_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>, Box<dyn CryptoModuleError>> {
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

    async fn decrypt_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>, Box<dyn CryptoModuleError>> {
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
