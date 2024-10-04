mod peer;

use std::io::{Error, ErrorKind};

use peer::{BroadcastClientPeer, ClientPeer};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub async fn send_file(file_chunks: Vec<Vec<u8>>) -> Result<Vec<Vec<u8>>> {
    let client = match BroadcastClientPeer::new().await {
        Ok(c) => c,
        Err(_) => return Err(Box::new(Error::new(ErrorKind::NotConnected, "Error starting client part"))),
    };
    let mut hashes = vec![];
    for chunk in file_chunks {
        hashes.push(match client.send(&chunk).await {
            Ok(h) => h,
            Err(_) => {
                eprintln!("Error sending chunk into domain: {:#?}", &chunk);
                continue;
            },
        });
    }
    Ok(hashes)
}

pub async fn recv_file(parts_hashes: Vec<Vec<u8>>) -> Result<Vec<Vec<u8>>> {
    let client = match BroadcastClientPeer::new().await {
        Ok(c) => c,
        Err(_) => return Err(Box::new(Error::new(ErrorKind::NotConnected, "Error starting client part"))),
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
    Ok(chunks)
}
