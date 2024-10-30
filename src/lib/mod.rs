use std::io::Error;
use std::net::SocketAddr;

use tokio::sync::mpsc::{channel, Receiver};
use tokio::task;

use crypto::{Encryptor, KuznechikEncryptor};
use hash::{Hasher, StreebogHasher};
use message::Message;
use server::BroadcastUdpServer;
use shared_secret::{ReedSolomonSecretSharer, SecretSharer};

pub mod codec;
pub mod crypto;
pub mod hash;
pub mod message;
pub mod shared_secret;
pub mod server;

pub mod consts {
    pub const WORKING_FOLDER_NAME: &str = ".leaf";
    pub const PASSWORD_FILE_NAME: &str = "passwd.txt";
    pub const GAMMA_FILE_NAME: &str = "gamma.bin";
    pub const SENDING_REQUEST_TYPE: u8 = 0;
    pub const SENDING_ACKNOWLEDGEMENT_TYPE: u8 = 1;
    pub const RETRIEVING_REQUEST_TYPE: u8 = 2;
    pub const RETRIEVING_ACKNOWLEDGEMENT_TYPE: u8 = 3;
    pub const CONTENT_FILLED_TYPE: u8 = 4;
    pub const EMPTY_TYPE: u8 = 5;
    pub const MAX_MESSAGE_SIZE: usize = 65243;
    pub const MAX_DATAGRAM_SIZE: usize = 65507;
    pub const DEFAULT_STOR_FILE_NAME: &str = "stor.bin";
}

pub fn init(addr: &str, broadcast_addr: &str, num_threads: usize) -> (Receiver<(Message, SocketAddr)>, BroadcastUdpServer) {
    let (tx, rx) = channel::<(Message, SocketAddr)>(1024);
    let server = BroadcastUdpServer::new(addr, broadcast_addr, tx.clone());

    for _ in 0..num_threads {
        let server = server.clone();
        task::spawn(async move {
            server.listen().await;
        });
    };

    (rx, server)
}

pub async fn send_file(content: Vec<u8>, server: &BroadcastUdpServer, receiver: &mut Receiver<(Message, SocketAddr)>) -> Result<Vec<Option<Vec<u8>>>, Error> {
    let sharer = ReedSolomonSecretSharer::new();
    let chunks = sharer.split_into_chunks(&content).unwrap();

    let encryptor = KuznechikEncryptor::new().unwrap();
    let mut encrypted_chunks = vec![];
    for chunk in chunks {
        if let Some(c) = chunk {
            encrypted_chunks.push(Some(encryptor.encrypt_chunk(&c).unwrap()));
        } else {
            encrypted_chunks.push(None);
        }
    }

    let hasher = StreebogHasher::new();
    let mut hashes = vec![];
    for chunk in &encrypted_chunks {
        if let Some(c) = chunk {
            hashes.push(Some(hasher.calc_hash_for_chunk(c)));
        } else {
            hashes.push(None);
        }
    }

    let mid = encrypted_chunks.len() / 2;

    for i in 0..mid {
        let (chunk, hash) = match encrypted_chunks.get(i).unwrap() {
            Some(c) => match hashes.get(i).unwrap() {
                Some(h) => (c, h),
                None => panic!(),
            },
            None => match encrypted_chunks.get(mid + i).unwrap() {
                Some(c) => match hashes.get(mid + i).unwrap() {
                    Some(h) => (c, h),
                    None => panic!(),
                },
                None => panic!(),
            }
        };
        server.send_chunk(hash, chunk, receiver).await?;
    }

    Ok(hashes)
}

pub async fn recv_content(hashes: Vec<Option<Vec<u8>>>, server: &BroadcastUdpServer, receiver: &mut Receiver<(Message, SocketAddr)>) -> Result<Vec<u8>, Error> {
    let mut chunks = vec![];
    for hash in hashes {
        if let Some(c) = hash {
            chunks.push(Some(server.recv_chunk(&c, receiver).await?));
        } else {
            chunks.push(None);
        }
    }

    let decryptor = KuznechikEncryptor::new().unwrap();
    let mut decrypted_chunks = vec![];
    for chunk in chunks {
        if let Some(c) = chunk {
            decrypted_chunks.push(Some(decryptor.decrypt_chunk(&c).unwrap()));
        } else {
            decrypted_chunks.push(None);
        }
    }

    let sharer = ReedSolomonSecretSharer::new();
    let content = sharer.recover_from_chunks(decrypted_chunks).unwrap();

    Ok(content)
}

pub async fn shutdown(server: BroadcastUdpServer) -> Result<(), Error> {
    server.shutdown().await;
    Ok(())
}