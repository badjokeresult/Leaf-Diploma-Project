use std::sync::Arc;

use tokio::net::UdpSocket;
use tokio::sync::Mutex;

use common::{Encryptor, KuznechikEncryptor, ReedSolomonSecretSharer, SecretSharer};

use peer::{BroadcastClientPeer, ClientPeer};

mod peer;

pub async fn send_content(content: Vec<u8>) -> Vec<Option<Vec<u8>>> {
    let sharer = ReedSolomonSecretSharer::new();
    let (data_chunks, rec_chunks) = sharer.split_into_chunks(&content).unwrap();

    let encryptor = KuznechikEncryptor::new().unwrap();
    let (mut enc_data_chunks, mut enc_rec_chunks) = (Vec::with_capacity(data_chunks.len()), Vec::with_capacity(rec_chunks.len()));
    for chunk in data_chunks {
        let enc_chunk = encryptor.encrypt_chunk(&chunk).await.unwrap();
        enc_data_chunks.push(enc_chunk);
    }
    for chunk in rec_chunks {
        let enc_chunk = encryptor.encrypt_chunk(&chunk).await.unwrap();
        enc_rec_chunks.push(enc_chunk);
    }

    let mut hashes = Vec::with_capacity(enc_rec_chunks.len() + enc_data_chunks.len());
    let client = BroadcastClientPeer::new().await;
    for chunk in enc_data_chunks {
        let hash = match client.send(&chunk).await {
            Ok(h) => Some(h),
            Err(_) => None,
        };
        hashes.push(hash);
    }
    for i in 0..enc_rec_chunks.len() {
        if hashes[i] == None {
            let hash = match client.send(&enc_rec_chunks[i]).await {
                Ok(h) => Some(h),
                Err(e) => panic!("Cannot send both data and recovery"),
            };
            hashes.push(hash);
        }
    }
    hashes
}

pub async fn recv_content(hashes: Vec<Option<Vec<u8>>>) -> Vec<u8> {
    let client = BroadcastClientPeer::new().await;
    let mut full_data = Vec::with_capacity(hashes.len());
    for hash in hashes {
        if hash == None {
            full_data.push(vec![0u8]);
        } else {
            let chunk = match client.recv(&hash.unwrap()).await {
                Ok(c) => c,
                Err(e) => panic!("Cannot receive both data and recovery"),
            };
            full_data.push(chunk);
        }
    }

    let decryptor = KuznechikEncryptor::new().unwrap();
    let mut chunk_len = 0;
    let (mut decrypted_data, mut decrypted_rec) = (Vec::with_capacity(full_data.len() / 2), Vec::with_capacity(full_data.len() / 2));
    for i in 0..full_data.len() / 2 {
        if full_data[i].len() > 1 {
            let chunk = decryptor.decrypt_chunk(&full_data[i]).await.unwrap();
            if chunk_len == 0 {
                chunk_len = chunk.len();
            }
            decrypted_data.push(chunk);
        } else {
            decrypted_data.push(vec![0u8; chunk_len]);
        }
    }
    for i in full_data.len() / 2..full_data.len() {
        if full_data[i].len() > 1 {
            let chunk = decryptor.decrypt_chunk(&full_data[i]).await.unwrap();
            if chunk_len == 0 {
                chunk_len = chunk.len();
            }
            decrypted_rec.push(chunk);
        } else {
            decrypted_rec.push(vec![0u8; chunk_len]);
        }
    }

    let sharer = ReedSolomonSecretSharer::new();
    let content = sharer.recover_from_chunks((decrypted_data, decrypted_rec)).unwrap();

    content
}