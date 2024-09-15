use std::str;

use kuznechik::{AlgOfb, KeyStore, Kuznechik};
use crate::crypto::errors::{CryptoModuleError, PasswordFromUtf8Error};
use super::initialization::*;
use super::encryptor::Encryptor;

pub struct KuznechikEncryptor {
    password_file: PasswordFilePathWrapper,
    gamma_file: GammaFilePathWrapper,
}

impl KuznechikEncryptor {
    pub fn new() -> KuznechikEncryptor {
        KuznechikEncryptor {
            password_file: PasswordFilePathWrapper::new(),
            gamma_file: GammaFilePathWrapper::new(),
        }
    }
}

impl Encryptor for KuznechikEncryptor {
    async fn encrypt_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>, Box<dyn CryptoModuleError>> {
        let mut cipher = self.generate_key().await?;

        let data = Vec::from(chunk);
        let encrypted_chunk = cipher.encrypt(data);

        Ok(encrypted_chunk)
    }

    async fn decrypt_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>, Box<dyn CryptoModuleError>> {
        let mut cipher = self.generate_key().await?;

        let data = Vec::from(chunk);
        let decrypted_chunk = cipher.decrypt(data);

        Ok(decrypted_chunk)
    }
}

impl KuznechikEncryptor {
    async fn generate_key(&self) -> Result<AlgOfb, Box<dyn CryptoModuleError>> {
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
