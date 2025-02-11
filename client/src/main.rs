mod args;
// mod parts;
// mod socket;

use std::path::PathBuf;

use base64::{prelude::BASE64_STANDARD, Engine};
use common::{
    Encryptor, Hasher, KuznechikEncryptor, Message, ReedSolomonChunks, ReedSolomonSecretSharer,
    SecretSharer, StreebogHasher,
};
use serde::{Deserialize, Serialize};
use tokio::{fs, net::UdpSocket};

// use crate::parts::{FileParts, Parts};
use args::{load_args, Action};

#[derive(Serialize, Deserialize)]
struct Metadata {
    data: Vec<u8>,
    recovery: Vec<u8>,
}

impl Metadata {
    pub fn new(data: Vec<u8>, recovery: Vec<u8>) -> Metadata {
        Metadata { data, recovery }
    }

    pub fn get_data(&self) -> Vec<u8> {
        self.data.clone()
    }

    pub fn get_recv(&self) -> Vec<u8> {
        self.recovery.clone()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = load_args();

    let path = args.get_file();
    match args.get_action() {
        Action::Send => send_file(path).await,
        Action::Receive => recv_file(path).await,
    }
}

async fn send_file(filepath: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read(&filepath).await.unwrap();
    let sharer = ReedSolomonSecretSharer::new().unwrap();
    let chunks = sharer.split_into_chunks(&content).unwrap();
    let (mut data, mut recovery) = chunks.deconstruct();

    let password = "n0tp3nt3$t";
    let encryptor = KuznechikEncryptor::new(password).await.unwrap();
    encryptor.encrypt_chunk(&mut data).unwrap();
    encryptor.encrypt_chunk(&mut recovery).unwrap();

    let hasher = StreebogHasher::new();
    let data_hash = hasher.calc_hash_for_chunk(&data);
    let recv_hash = hasher.calc_hash_for_chunk(&recovery);
    let metadata = Metadata::new(data_hash, recv_hash);

    let socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();
    socket.set_broadcast(true).unwrap();
    let req: Vec<u8> = Message::SendingReq(metadata.get_data()).into();
    socket.send_to(&req, "255.255.255.255:62092").await.unwrap();
    let mut ack = [0u8; 4096];
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    if let Ok((sz, addr)) = socket.recv_from(&mut ack).await {
        let ack = Message::from(ack[..sz].to_vec());
        if let Message::SendingAck(h) = ack {
            if h.iter().eq(&metadata.get_data()) {
                let content: Vec<u8> = Message::ContentFilled(metadata.get_data(), data).into();
                socket.send_to(&content, addr).await.unwrap();
            }
        }
    }

    let json = serde_json::to_vec(&metadata).unwrap();
    let mut result = [0u8; 4096];
    BASE64_STANDARD.encode_slice(&json, &mut result).unwrap();
    fs::write(filepath, &result).await.unwrap();
    Ok(())
}

async fn recv_file(filepath: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read(&filepath).await.unwrap();
    let json = BASE64_STANDARD.decode(&content).unwrap();
    let metadata: Metadata = serde_json::from_slice(&json).unwrap();

    let socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();
    socket.set_broadcast(true).unwrap();
    let req: Vec<u8> = Message::RetrievingReq(metadata.get_data()).into();
    socket.send_to(&req, "255.255.255.255:62092").await.unwrap();
    let mut data = vec![];
    let mut recv = vec![];
    let mut content = vec![];
    if let Ok((sz, _)) = socket.recv_from(&mut content).await {
        let content = Message::from(content[..sz].to_vec());
        if let Message::ContentFilled(h, d) = content {
            if h.iter().eq(&metadata.get_data()) {
                data = d;
            }
        }
    }
    let req: Vec<u8> = Message::RetrievingReq(metadata.get_recv()).into();
    socket.send_to(&req, "255.255.255.255:62092").await.unwrap();
    if let Ok((sz, _)) = socket.recv_from(&mut content).await {
        let content = Message::from(content[..sz].to_vec());
        if let Message::ContentFilled(h, d) = content {
            if h.iter().eq(&metadata.get_recv()) {
                recv = d;
            }
        }
    }

    let password = "n0tp3nt3$t";
    let decryptor = KuznechikEncryptor::new(password).await.unwrap();
    decryptor.decrypt_chunk(&mut data).unwrap();
    decryptor.decrypt_chunk(&mut recv).unwrap();

    let chunks = ReedSolomonChunks::new(data, recv);
    let sharer = ReedSolomonSecretSharer::new().unwrap();
    let final_content = sharer.recover_from_chunks(chunks).unwrap();
    fs::write(filepath, final_content).await.unwrap();
    Ok(())
}
