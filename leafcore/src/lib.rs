use lazy_static::lazy_static;

mod codec;
mod crypto;
mod hash;
mod messages;
mod peer;
mod shared_secret;
mod storage;

use async_once::AsyncOnce;
use tokio::task;
use peer::server::BroadcastServerPeer;
use crate::peer::client::{BroadcastClientPeer, ClientPeer};
use crate::peer::server::ServerPeer;
use crate::shared_secret::{SecretSharer, ShamirSecretSharer};

lazy_static! {
    static ref PEER_SERVER: AsyncOnce<BroadcastServerPeer> = AsyncOnce::new(async {
        BroadcastServerPeer::new(62092).await
    });
}

pub async fn init() {
    task::spawn(async {
        PEER_SERVER.get().await.listen().await;
    }).await.unwrap();
}

const AMOUNT_OF_CHUNKS: u8 = 5;

pub async fn store_file(content: &[u8]) -> Vec<Vec<u8>> {
    let sharer = ShamirSecretSharer::new(AMOUNT_OF_CHUNKS);
    let chunks = sharer.split_into_chunks(content);

    let mut encrypted_chunks = vec![];
    for chunk in &chunks {
        let encrypted_chunk = crypto::encrypt_chunk(chunk).await;
        encrypted_chunks.push(encrypted_chunk);
    }

    let client = BroadcastClientPeer::new(62092).await;
    let mut hashes = vec![];
    for chunk in &encrypted_chunks {
        let hash = client.send(chunk).await;
        hashes.push(hash);
    }

    hashes
}

pub async fn receive_file(hashes: &[[&u8]]) -> Vec<u8> {
    let client = BroadcastClientPeer::new(62092).await;
    let mut encrypted_chunks = vec![];
    for hash in hashes {
        let chunk = client.receive(hash).await;
        encrypted_chunks.push(chunk);
    }

    let mut chunks = vec![];
    for chunk in &encrypted_chunks {
        let decrypted_chunk = crypto::decrypt_chunk(chunk).await;
        chunks.push(decrypted_chunk);
    }

    let sharer = ShamirSecretSharer::new(AMOUNT_OF_CHUNKS);
    let content = sharer.recover_from_chunks(chunks);
    content
}