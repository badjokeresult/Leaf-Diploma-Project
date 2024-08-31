use std::ops::Deref;

use async_once::AsyncOnce;
use lazy_static::lazy_static;

use peer::client::{BroadcastClientPeer, ClientPeer};
use peer::server::{BroadcastServerPeer, ServerPeer};

mod peer;
mod storage;
mod messages;
mod hash;
mod codec;

lazy_static! {
    static ref PEER_SERVER: AsyncOnce<BroadcastServerPeer> = AsyncOnce::new(async {
        BroadcastServerPeer::new(62092).await
    });
    static ref PEER_CLIENT: AsyncOnce<BroadcastClientPeer> = AsyncOnce::new(async {
        BroadcastClientPeer::new(62092).await
    });
}

pub async fn init() {
    PEER_SERVER.get().await.listen().await;
}

pub async fn send_chunks_to_peers(chunks: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    let mut hashes = vec![];
    for chunk in &chunks {
        hashes.push(PEER_CLIENT.deref().await.send(chunk).await);
    }
    hashes
}

pub async fn receive_chunks_from_peers(hashes: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    let mut chunks = vec![];
    for hash in &hashes {
        chunks.push(PEER_CLIENT.deref().await.receive(hash).await);
    }
    chunks
}

pub async fn shutdown() {
    PEER_SERVER.get().await.proper_shutdown().await;
}