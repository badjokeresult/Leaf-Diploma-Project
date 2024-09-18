mod codec;
mod crypto;
mod hash;
mod messages;
mod peer;
mod shared_secret;

use lazy_static::lazy_static;
use async_once::AsyncOnce;
use tokio::task;
use tokio::task::JoinHandle;

use peer::client::peer::*;
use peer::server::peer::*;
use shared_secret::sharer::*;
use crypto::encryptor::*;
use crate::hash::hasher::gost::StreebogHasher;
use crate::hash::hasher::Hasher;
use crate::hash::hasher::sha::Sha3_256Hasher;

pub async fn init<C,E,H>(gost: bool) -> (C, E, H)
    where C: ClientPeer, E: Encryptor, H: Hasher
{
    task::spawn(async {
        let peer_server = BroadcastServerPeer::new(62092).await;
        peer_server.listen().await;
    }).await.unwrap();

    let client = BroadcastClientPeer::new(62092).await.unwrap();

    return if gost {
        (client, KuznechikEncryptor::new().unwrap(), StreebogHasher)
    } else {
        (client, Aes256Encryptor::new().unwrap(), Sha3_256Hasher)
    }
}

pub async fn store_file<C,E,H>(content: Vec<u8>, client: &C, encryptor: &E, hasher: &H) -> Vec<Vec<u8>>
    where C: ClientPeer, E: Encryptor, H: Hasher
{
    let mut sharer = ReedSolomonSecretSharer::new();
    let mut chunks = sharer.split_into_chunks(&content);

    let mut encrypted_chunks = vec![];
    for chunk in &mut chunks {
        let encrypted_chunk = encryptor.encrypt_chunk(chunk).await;
        encrypted_chunks.push(encrypted_chunk);
    }

    let mut hashes = vec![];
    for chunk in &encrypted_chunks {
        let hash = client.send(chunk, hasher).await;
        hashes.push(hash);
    }

    hashes
}

pub async fn receive_file<T>(hashes: Vec<Vec<u8>>, client: &T) -> Vec<u8>
    where T: ClientPeer
{
    let mut encrypted_chunks = vec![];
    for hash in &hashes {
        let chunk = client.receive(hash).await;
        encrypted_chunks.push(chunk);
    }

    let mut chunks = vec![];
    let encryptor = Aes256Encryptor::new().unwrap();
    for chunk in &mut encrypted_chunks {
        let decrypted_chunk = encryptor.decrypt_chunk(chunk).await.unwrap();
        chunks.push(decrypted_chunk);
    }

    let sharer = ShamirSecretSharer::new(chunks.len());
    let content = sharer.recover_from_chunks(chunks);
    content
}

pub async fn shutdown() {
    todo!()
}