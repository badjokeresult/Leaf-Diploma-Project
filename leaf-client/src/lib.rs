mod peer;

use leaf_common::{Encryptor, KuznechikEncryptor, ReedSolomonSecretSharer, SecretSharer};
use peer::{BroadcastClientPeer, ClientPeer};

pub async fn send_file(file_content: Vec<u8>) -> Vec<Vec<u8>> {
    let sharer = ReedSolomonSecretSharer::new();
    let chunks = match sharer.split_into_chunks(&file_content) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("ERROR SPLITTING INTO CHUNKS");
            return vec![vec![]];
        },
    };
    eprintln!("LEN OF CHUNKS : {}", chunks.len());
    let encryptor = match KuznechikEncryptor::new() {
        Ok(e) => e,
        Err(_) => {
            eprintln!("ERROR INIT ENCRYPTOR");
            return vec![vec![]];
        },
    };
    let mut encrypted_chunks = vec![];

    for chunk in chunks {
        encrypted_chunks.push(match encryptor.encrypt_chunk(&chunk).await {
            Ok(c) => c,
            Err(_) => {
                eprintln!("ERROR ENCRYPTING CHUNK");
                return vec![vec![]];
            },
        });
    }

    for chunk in &encrypted_chunks {
        eprintln!("SIZE OF ENCRYPTED CHUNK : {}", chunk.len());
    }

    let client = match BroadcastClientPeer::new().await {
        Ok(c) => c,
        Err(_) => {
            eprintln!("ERROR INIT CLIENT");
            return vec![vec![]];
        },
    };
    let mut hashes = vec![];
    for chunk in encrypted_chunks {
        hashes.push(match client.send(&chunk).await {
            Ok(h) => h,
            Err(_) => {
                eprintln!("Error sending chunk into domain: {:#?}", &chunk);
                continue;
            },
        });
    }
    hashes
}

pub async fn recv_file(parts_hashes: Vec<Vec<u8>>) -> Vec<u8> {
    let client = match BroadcastClientPeer::new().await {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let mut chunks = vec![];
    for hash in parts_hashes {
        chunks.push(match client.recv(&hash).await {
            Ok(c) => c,
            Err(_) => {
                eprintln!("Error receiving data by hash: {:#?}", &hash);
                continue;
            },
        });
    }

    let encryptor = match KuznechikEncryptor::new() {
        Ok(e) => e,
        Err(_) => return vec![],
    };
    let mut decrypted_chunks = vec![];
    for chunk in chunks {
        decrypted_chunks.push(match encryptor.decrypt_chunk(&chunk).await {
            Ok(c) => c,
            Err(_) => return vec![],
        });
    }

    let sharer = ReedSolomonSecretSharer::new();
    let content = match sharer.recover_from_chunks(decrypted_chunks) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    content
}
