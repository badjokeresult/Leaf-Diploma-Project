#![allow(unused_variables)]
#![allow(dead_code)]

#[derive(Serialize, Deserialize)]
struct FileParts {
    data: Vec<Vec<u8>>,
    hashes: Vec<Vec<u8>>,
}

impl FileParts {
    pub fn new(data: Vec<Vec<u8>>, hashes: Vec<Vec<u8>>) -> FileParts {
        FileParts { data, hashes }
    }
}

mod args;
// mod parts;
// mod socket;

use std::path::PathBuf;

use common::{
    Encryptor, Hasher, KuznechikEncryptor, Message, ReedSolomonSecretSharer, SecretSharer,
    StreebogHasher,
};
use serde::{Deserialize, Serialize};
use tokio::{fs::OpenOptions, io::AsyncReadExt, net::UdpSocket};

// use crate::parts::{FileParts, Parts};
use args::{load_args, Action};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = load_args();

    let path = args.get_file();
    match args.get_action() {
        Action::Send => send_file(path).await,
        Action::Receive => recv_file(path).await,
    }
}

async fn send_file(file: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .truncate(false)
        .open(&file)
        .await?;

    let mut content_buf = vec![0u8; file.metadata().await?.len() as usize];
    file.read(&mut content_buf).await?;

    let sharer = ReedSolomonSecretSharer::new().unwrap();
    let full_data = sharer.split_into_chunks(&content_buf).unwrap();
    let (data, recovery) = full_data.split_at(full_data.len() / 2);
    let mut data = data.to_vec();
    let mut recovery = recovery.to_vec();

    let password = "Hello world"; // Properly ask for password
    let encryptor = KuznechikEncryptor::new(password).await.unwrap();
    for chunk in &mut data {
        encryptor.encrypt_chunk(chunk).unwrap();
    }
    for chunk in &mut recovery {
        encryptor.encrypt_chunk(chunk).unwrap();
    }

    let hasher = StreebogHasher::new();
    let (mut data_with_hashes, mut recovery_with_hashes) = (vec![], vec![]);
    for chunk in data.iter() {
        data_with_hashes.push((chunk.to_vec(), hasher.calc_hash_for_chunk(chunk)));
    }
    for chunk in recovery.iter() {
        recovery_with_hashes.push((chunk.to_vec(), hasher.calc_hash_for_chunk(chunk)));
    }

    let socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();
    socket.set_broadcast(true).unwrap();
    let length = data_with_hashes.len();
    for i in 0..length {
        let hash = data_with_hashes[i].1.clone();
        let data = data_with_hashes[i].0.clone();
        let req: Vec<u8> = Message::SendingReq(hash.clone()).into();
        socket.send_to(&req, "255.255.255.255:62092").await.unwrap();
        let mut ack = [0u8; 4096];
        if let Ok((sz, addr)) = socket.recv_from(&mut ack).await {
            let ack = Message::from(ack[..sz].to_vec());
            if let Message::SendingAck(h) = ack {
                if h.iter().eq(&hash) {
                    let stream = Message::generate_stream_for_chunk(&hash, &data).unwrap();
                    for msg in stream {
                        let message_bin: Vec<u8> = msg.into();
                        socket.send_to(&message_bin, addr).await?;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn recv_file(file: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    todo!()
}
