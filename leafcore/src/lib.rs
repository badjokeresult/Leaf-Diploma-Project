mod codec;
mod crypto;
mod hash;
mod messages;
mod peer;
mod shared_secret;
mod storage;

use lazy_static::lazy_static;
use async_once::AsyncOnce;
use tokio::task;
use tokio::task::JoinHandle;
use peer::server::BroadcastServerPeer;
use peer::client::{BroadcastClientPeer, ClientPeer};
use peer::server::ServerPeer;
use shared_secret::{SecretSharer, ShamirSecretSharer};

lazy_static! {
    static ref PEER_SERVER: AsyncOnce<BroadcastServerPeer> = AsyncOnce::new(async {
        BroadcastServerPeer::new(62092).await
    });

    static ref PEER_CLIENT: AsyncOnce<BroadcastClientPeer> = AsyncOnce::new(async {
        BroadcastClientPeer::new(62092).await
    });
}

pub async fn init() -> JoinHandle<()> {
    task::spawn(async {
        PEER_SERVER.get().await.listen().await;
    })
}

const AMOUNT_OF_CHUNKS: u8 = 5;

pub async fn store_file(content: Vec<u8>) -> Vec<Vec<u8>> {
    let sharer = ShamirSecretSharer::new(AMOUNT_OF_CHUNKS);
    let chunks = sharer.split_into_chunks(&content);

    let mut encrypted_chunks = vec![];
    for chunk in &chunks {
        let encrypted_chunk = crypto::encrypt_chunk(chunk).await;
        encrypted_chunks.push(encrypted_chunk);
    }

    let mut hashes = vec![];
    for chunk in &encrypted_chunks {
        let hash = PEER_CLIENT.get().await.send(chunk).await;
        hashes.push(hash);
    }

    hashes
}

pub async fn receive_file(hashes: Vec<Vec<u8>>) -> Vec<u8> {
    let mut encrypted_chunks = vec![];
    for hash in &hashes {
        let chunk = PEER_CLIENT.get().await.receive(hash).await;
        encrypted_chunks.push(chunk);
    }

    let mut chunks = vec![];
    for chunk in &encrypted_chunks {
        let decrypted_chunk = crypto::decrypt_chunk(chunk).await;
        chunks.push(decrypted_chunk);
    }

    let sharer = ShamirSecretSharer::new(AMOUNT_OF_CHUNKS);
    let content = sharer.recover_from_chunks(&chunks);
    content
}