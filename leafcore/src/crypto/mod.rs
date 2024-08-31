use std::env;
use tokio::fs;
use std::path::PathBuf;

use openssl::aes::{AesKey, aes_ige};
use openssl::symm::Mode;
use openssl::rand::rand_priv_bytes;
use rand::{Rng, thread_rng};

pub trait Encryptor {
    fn encrypt_chunk(&mut self, chunk: &[u8]) -> Vec<u8>;
    fn decrypt_chunk(&mut self, chunk: &[u8]) -> Vec<u8>;
}

const PASSWORD_FILE_NAME: &str = "passwd.txt";

pub struct Aes256Encryptor {
    enc_key: AesKey,
    dec_key: AesKey,
    iv: [u8; 32],
}

impl Aes256Encryptor {
    pub async fn new() -> Aes256Encryptor {
        let home_dir = PathBuf::from(env::var("HOME").unwrap()).join(".leaf");
        let files = home_dir.read_dir();
        for file in files.unwrap() {
            let filename = file.unwrap().file_name();
            if filename.eq(PASSWORD_FILE_NAME) {
                return Self::from_file(&home_dir.join(filename)).await;
            }
        }

        let filepath = home_dir.join(".leaf").join(PASSWORD_FILE_NAME);
        Self::init_encryption_in_first_time(&filepath).await;
        Self::from_file(&filepath).await
    }

    async fn init_encryption_in_first_time(file_to_write: &PathBuf) {
        let password = Self::generate_new_password(14);
        fs::write(file_to_write, &password).await.unwrap();
    }

    fn generate_new_password(len: usize) -> Vec<u8> {
        let password: String = thread_rng()
            .sample_iter::<u8, _>(rand::distributions::Alphanumeric)
            .take(len)
            .map(|x| x as char)
            .collect();

        let password_as_bytes = password.as_bytes();
        let password_as_bytes_vec = password_as_bytes.to_vec();
        password_as_bytes_vec
    }

    async fn from_file(path: &PathBuf) -> Aes256Encryptor {
        let password = fs::read_to_string(path).await.unwrap();
        let enc_key = AesKey::new_encrypt(password.as_bytes()).unwrap();
        let dec_key = AesKey::new_decrypt(password.as_bytes()).unwrap();

        let mut iv = [0u8; 32];
        rand_priv_bytes(&mut iv).unwrap();

        Aes256Encryptor {
            enc_key,
            dec_key,
            iv,
        }
    }
}

impl Encryptor for Aes256Encryptor {
    fn encrypt_chunk(&mut self, chunk: &[u8]) -> Vec<u8> {
        let mut buf = [0u8; 4096];
        aes_ige(chunk, &mut buf, &self.enc_key, self.iv.as_mut_slice(), Mode::Encrypt);
        let result = buf.to_vec();
        result
    }

    fn decrypt_chunk(&mut self, chunk: &[u8]) -> Vec<u8> {
        let mut buf = [0u8; 4096];
        aes_ige(chunk, &mut buf, &self.dec_key, self.iv.as_mut_slice(), Mode::Decrypt);
        let result = buf.to_vec();
        result
    }
}
