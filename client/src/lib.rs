mod peer;

use std::fmt::Display;

use peer::{ClientPeer, BroadcastClientPeer};
use common::{Encryptor, KuznechikEncryptor, ReedSolomonSecretSharer, SecretSharer};

pub async fn send_file(content: Vec<u8>) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error>> {
    // 1. Split into chunks
    let sharer = ReedSolomonSecretSharer::new();
    println!("SHARER CREATED");
    let chunks = match sharer.split_into_chunks(&content) {
        Ok(c) => c,
        Err(e) => panic!("{}", e.to_string()),
    };
    println!("CHUNKS WERE CREATED: CHUNKS LEN : {}", chunks.len());
    // 2. Encrypt each chunk
    let mut encrypted_chunks = vec![];
    let encryptor = match KuznechikEncryptor::new() {
        Ok(e) => e,
        Err(e) => panic!("{}", e.to_string()),
    };
    println!("ENCRYPTOR WAS CREATED");
    for chunk in &chunks {
        let encrypted_chunk = match encryptor.encrypt_chunk(chunk).await {
            Ok(c) => c,
            Err(e) => panic!("{}", e.to_string()),
        };
        encrypted_chunks.push(encrypted_chunk);
    }
    println!("CHUNKS WERE ENCRYPTED! CHUNKS LEN : {}", encrypted_chunks.len());
    // 3. Send each encrypted chunk and save it as hash
    let mut hashes = vec![];
    let client = match BroadcastClientPeer::new().await {
        Ok(c) => c,
        Err(e) => panic!("{}", e.to_string()),
    };
    println!("CLIENT WAS CREATED");
    for chunk in &encrypted_chunks {
        let hash = match client.send(chunk).await {
            Ok(h) => h,
            Err(e) => panic!("{}", e.to_string()),
        };
        hashes.push(hash);
    }
    println!("HASHES WERE CALCULATED");
    // 4. Returns hashes
    Ok(hashes)
}

pub async fn recv_file(hashes: Vec<Vec<u8>>) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // 1. Receive chunks
    let client = match BroadcastClientPeer::new().await {
        Ok(c) => c,
        Err(e) => panic!("{}", e.to_string()),
    };
    let mut encrypted_chunks = vec![];
    for hash in &hashes {
        let chunk = match client.recv(hash).await {
            Ok(c) => c,
            Err(e) => panic!("{}", e.to_string()),
        };
        encrypted_chunks.push(chunk);
    }
    // 2. Decrypt chunks
    let mut decrypted_chunks = vec![];
    let decryptor = match KuznechikEncryptor::new() {
        Ok(d) => d,
        Err(e) => panic!("{}", e.to_string()),
    };
    for chunk in &encrypted_chunks {
        let decrypted_chunk = match decryptor.decrypt_chunk(chunk).await {
            Ok(c) => c,
            Err(e) => panic!("{}", e.to_string()),
        };
        decrypted_chunks.push(decrypted_chunk);
    }
    // 3. Restore original file from chunks
    let sharer = ReedSolomonSecretSharer::new();
    let content = match sharer.recover_from_chunks(decrypted_chunks) {
        Ok(c) => c,
        Err(e) => panic!("{}", e.to_string()),
    };
    // 4. Returns content
    Ok(content)
}