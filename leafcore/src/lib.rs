mod shared_secret;
mod crypto;

use std::str::FromStr;

use shared_secret::{ShamirSecretSharer, SecretSharer};
use crypto::{Aes256Encryptor, Encryptor};

use leafpeer;

pub async fn init() -> (Aes256Encryptor, ShamirSecretSharer) {
    let encryptor = Aes256Encryptor::new().await;
    let sharer = ShamirSecretSharer::new(5);
    leafpeer::init().await;
    (encryptor, sharer)
}

pub async fn store_file(content: Vec<u8>, sharer: &ShamirSecretSharer, encryptor: &mut Aes256Encryptor) -> Vec<Vec<u8>> {
    let chunks = sharer.split_into_chunks(content);

    let mut encrypted_chunks = vec![];
    for chunk in &chunks {
        encrypted_chunks.push(encryptor.encrypt_chunk(chunk));
    }

    let sent_hashes = leafpeer::send_chunks_to_peers(encrypted_chunks).await;
    sent_hashes
}

pub async fn receive_file(hashes: Vec<Vec<u8>>, sharer: &ShamirSecretSharer, encryptor: &mut Aes256Encryptor) -> Vec<u8> {
    let encrypted_chunks = leafpeer::receive_chunks_from_peers(hashes).await;
    let mut chunks = vec![];
    for chunk in &encrypted_chunks {
        chunks.push(encryptor.decrypt_chunk(chunk));
    }

    let content = sharer.recover_from_chunks(chunks);
    content
}

pub async fn shutdown() {
    leafpeer::shutdown().await;
}