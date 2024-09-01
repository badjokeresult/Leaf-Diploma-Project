use std::io;
use std::path::PathBuf;

use tokio::fs;
use openssl::aes::{AesKey, aes_ige};
use openssl::symm::Mode;
use openssl::rand::rand_priv_bytes;
use rand::{Rng, thread_rng};
use dirs;

struct PasswordFilePathWrapper(pub PathBuf);

impl PasswordFilePathWrapper {
    fn new() -> PasswordFilePathWrapper {
        PasswordFilePathWrapper {
            0: dirs::home_dir().unwrap().join(".leaf").join("passwd.txt"),
        }
    }
}

pub async fn encrypt_chunk(chunk: &[u8]) -> Vec<u8> {
    let password_file = PasswordFilePathWrapper::new();
    let passwd = load_passwd(&password_file.0).await.unwrap();
    let key = AesKey::new_encrypt(&passwd).unwrap();

    let mut buf = [0u8; 4096];
    let mut iv = [0u8; 32];
    rand_priv_bytes(&mut iv).unwrap();

    aes_ige(chunk, &mut buf, &key, &mut iv, Mode::Encrypt);
    let buf_vec = buf.to_vec();
    buf_vec
}

pub async fn decrypt_chunk(chunk: &[u8]) -> Vec<u8> {
    let password_file = PasswordFilePathWrapper::new();
    let passwd = load_passwd(&password_file.0).await.unwrap();
    let key = AesKey::new_decrypt(&passwd).unwrap();

    let mut buf = [0u8; 4096];
    let mut iv = [0u8; 32];

    aes_ige(chunk, &mut buf, &key, &mut iv, Mode::Decrypt);
    let buf_vec = buf.to_vec();
    buf_vec
}

async fn load_passwd(filename: &PathBuf) -> io::Result<Vec<u8>> {
    if let Err(_) = fs::read_to_string(filename).await {
        init_password_at_first_launch(filename, 64).await;
    }
    let binding = fs::read_to_string(filename).await.unwrap();
    let binding = binding.as_bytes();
    let content = binding.to_vec();

    Ok(content)
}

async fn init_password_at_first_launch(filename: &PathBuf, len: usize) {
    let password: String = thread_rng()
        .sample_iter::<u8, _>(rand::distributions::Alphanumeric)
        .take(len)
        .map(|x| x as char)
        .collect();

    let password_as_bytes = password.as_bytes();

    fs::write(filename, password_as_bytes).await.unwrap();
}
