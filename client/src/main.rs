mod args;

use std::path::PathBuf;

use base64::{prelude::BASE64_STANDARD, Engine};
use common::{
    Encryptor, Hasher, KuznechikEncryptor, Message, ReedSolomonChunks, ReedSolomonSecretSharer,
    SecretSharer, StreebogHasher,
};
use serde::{Deserialize, Serialize};
use tokio::{fs, net::UdpSocket};

use args::{load_args, Action};

#[derive(Serialize, Deserialize)]
struct Metadata {
    data: Vec<String>,
    recovery: Vec<String>,
}

impl Metadata {
    pub fn new(data: Vec<String>, recovery: Vec<String>) -> Metadata {
        Metadata { data, recovery }
    }

    pub fn get_data(&self) -> Vec<String> {
        self.data.clone()
    }

    pub fn get_recv(&self) -> Vec<String> {
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

    let password = std::env::var("PASSWORD").unwrap();
    let encryptor = KuznechikEncryptor::new(&password).await.unwrap();
    for c in data.iter_mut() {
        encryptor.encrypt_chunk(c).unwrap();
    }
    for c in recovery.iter_mut() {
        encryptor.encrypt_chunk(c).unwrap();
    }

    let hasher = StreebogHasher::new();
    let (mut data_hash, mut recv_hash) = (vec![], vec![]);
    for c in data.iter_mut() {
        data_hash.push(hex::encode(hasher.calc_hash_for_chunk(c)));
    }
    for c in recovery.iter_mut() {
        recv_hash.push(hex::encode(hasher.calc_hash_for_chunk(c)));
    }

    let metadata = Metadata::new(data_hash, recv_hash);

    let socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();
    socket.set_broadcast(true).unwrap();
    let data_hashes = metadata.get_data();
    let recv_hashes = metadata.get_recv();
    for i in 0..data.len() {
        println!("{:?} = {:?}", &data_hashes[i], &data[i]);
        send_chunk(&socket, &data_hashes[i], &data[i]).await;
        println!("DATA CHUNK WITH INDEX {} was sent", i);
    }
    for i in 0..recovery.len() {
        println!("{:?} = {:?}", &recv_hashes[i], &recovery[i]);
        send_chunk(&socket, &recv_hashes[i], &recovery[i]).await;
        println!("RECV CHUNK WITH INDEX {} was sent", i);
    }

    let json = serde_json::to_vec(&metadata).unwrap();
    let b64 = BASE64_STANDARD.encode(json);
    fs::write(filepath, &b64).await.unwrap();
    Ok(())
}

async fn send_chunk(socket: &UdpSocket, hash: &str, data: &[u8]) {
    let req: Vec<u8> = Message::SendingReq(hash.to_string()).into();
    socket.send_to(&req, "255.255.255.255:62092").await.unwrap();
    let mut ack = [0u8; 4096];
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    if let Ok((sz, addr)) = socket.recv_from(&mut ack).await {
        let ack = Message::from(ack[..sz].to_vec());
        if let Message::SendingAck(h) = ack {
            if h.eq(hash) {
                let content: Vec<u8> =
                    Message::ContentFilled(hash.to_string(), data.to_vec()).into();
                socket.send_to(&content, addr).await.unwrap();
            }
        }
    }
}

async fn recv_chunk(socket: &UdpSocket, hash: &str) -> Vec<u8> {
    let req: Vec<u8> = Message::RetrievingReq(hash.to_string()).into();
    socket.send_to(&req, "255.255.255.255:62092").await.unwrap();
    let mut content = [0u8; 4096];
    if let Ok((sz, _)) = socket.recv_from(&mut content).await {
        let content = Message::from(content[..sz].to_vec());
        if let Message::ContentFilled(h, d) = content {
            if h.eq(hash) {
                return d;
            }
        }
    }
    vec![]
}

async fn recv_file(filepath: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(&filepath).await.unwrap();
    let json = BASE64_STANDARD.decode(content).unwrap();
    let metadata: Metadata = serde_json::from_slice(&json).unwrap();

    let mut data = vec![];
    let mut recv = vec![];
    let socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();
    socket.set_broadcast(true).unwrap();
    for h in metadata.get_data().iter() {
        data.push(recv_chunk(&socket, h).await);
    }
    for h in metadata.get_recv().iter() {
        recv.push(recv_chunk(&socket, h).await);
    }

    for d in &data {
        println!("{}", d.len());
    }
    for d in &recv {
        println!("{}", d.len());
    }

    let password = std::env::var("PASSWORD").unwrap();
    let decryptor = KuznechikEncryptor::new(&password).await.unwrap();
    for c in data.iter_mut() {
        decryptor.decrypt_chunk(c).unwrap();
    }
    for c in recv.iter_mut() {
        decryptor.decrypt_chunk(c).unwrap();
    }

    // let chunks = ReedSolomonChunks::new(data, recv);
    // let sharer = ReedSolomonSecretSharer::new().unwrap();
    // let final_content = sharer.recover_from_chunks(chunks).unwrap();
    let final_content = data.iter().flatten().cloned().collect::<Vec<u8>>();
    fs::write(filepath, final_content).await.unwrap();
    Ok(())
}
