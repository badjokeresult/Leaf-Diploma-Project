use std::net::IpAddr;
use std::str::FromStr;

use crate::client::BroadcastUdpClient;
use crate::crypto::{Encryptor, KuznechikEncryptor};
use crate::hash::{Hasher, StreebogHasher};
use crate::shared_secret::{ReedSolomonSecretSharer, SecretSharer};

mod client;
mod codec;
mod crypto;
mod hash;
mod message;
mod peer;
mod server;
mod shared_secret;

pub fn init(local_ip: &str, local_broadcast: &str, num_threads: usize) -> BroadcastUdpClient {
    let local_ip = IpAddr::from_str(local_ip).unwrap();
    let local_broadcast = IpAddr::from_str(local_broadcast).unwrap();
    BroadcastUdpClient::new(num_threads, local_ip, local_broadcast)
}

pub fn send_file(content: Vec<u8>, client: &BroadcastUdpClient) -> Vec<Option<Vec<u8>>> {
    let sharer = ReedSolomonSecretSharer::new();
    let chunks = sharer.split_into_chunks(&content).unwrap();

    let encryptor = KuznechikEncryptor::new().unwrap();
    let mut enc_chunks = vec![];
    for chunk in chunks {
        if let Some(x) = chunk {
            let enc_data_chunk = encryptor.encrypt_chunk(&x).unwrap();
            enc_chunks.push(Some(enc_data_chunk));
        } else {
            enc_chunks.push(None);
        }
    }

    let hasher = StreebogHasher::new();
    let mut errors_amount = 0;
    let errors_crit_amount = enc_chunks.len() / 2 + 1;
    let mut hashes = vec![];
    for chunk in enc_chunks {
        if let Some(x) = chunk {
            let hash = hasher.calc_hash_for_chunk(&x);
            match client.send(&hash, &x) {
                Ok(_) => hashes.push(Some(hash)),
                Err(_) => {
                    errors_amount += 1;
                    hashes.push(None);
                    continue;
                },
            };
            if errors_amount > errors_crit_amount {
                panic!("Error sending chunks");
            }
        }
    }

    hashes
}

pub fn recv_content(hashes: Vec<Option<Vec<u8>>>, client: &BroadcastUdpClient) -> Vec<u8> {
    let mut chunks = vec![None; hashes.len()];
    for i in 0..hashes.len() {
        if let Some(hash) = &hashes[i] {
            let chunk = client.recv(hash).unwrap();
            chunks[i] = Some(chunk);
        }
    }

    let decryptor = KuznechikEncryptor::new().unwrap();
    let mut dec_chunks = vec![None; chunks.len()];
    for i in 0..chunks.len() {
        if let Some(chunk) = &chunks[i] {
            let dec_chunk = decryptor.decrypt_chunk(&chunk).unwrap();
            dec_chunks[i] = Some(dec_chunk);
        }
    }

    let sharer = ReedSolomonSecretSharer::new();
    let content = sharer.recover_from_chunks(dec_chunks).unwrap();

    content
}

pub fn shutdown(client: BroadcastUdpClient) {
    client.shutdown();
}