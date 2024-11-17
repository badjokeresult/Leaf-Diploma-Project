mod args;
mod meta;

use std::path::PathBuf;

use tokio::fs;
use clap::Parser;

use leaflibrary::*;
use args::Args;
use meta::MetaFileInfo;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let encryptor = KuznechikEncryptor::new(
        &dirs::home_dir().unwrap().join(".leaf").join("password.txt"),
        &dirs::home_dir().unwrap().join(".leaf").join("gamma.bin"),
    ).unwrap();

    match args.action.as_str() {
        "send" => handle_send(&args.file, &encryptor).await,
        "recv" => handle_recv(&args.file, &encryptor).await,
        _ => {},
    }
}

async fn handle_send(path: &PathBuf, encryptor: &KuznechikEncryptor) {
    let content = fs::read(path).await.unwrap();
    let sharer = ReedSolomonSecretSharer::new().unwrap();
    let (mut data_chunks, mut rec_chunks) = sharer.split_into_chunks(&content).unwrap();

    encrypt_data(&mut data_chunks, encryptor).await;
    encrypt_data(&mut rec_chunks, encryptor).await;

    let client = BroadcastUdpClient::new(
        "0.0.0.0:0",
        "255.255.255.255:62092",
    ).await.unwrap();
    let data_hashes = send_data(&data_chunks, &client).await;
    let rec_hashes = send_data(&rec_chunks, &client).await;

    let metainfo = MetaFileInfo::new(data_hashes, rec_hashes);
    fs::write(path, serde_json::to_vec(&metainfo).unwrap()).await.unwrap();
}

async fn encrypt_data(data: &mut Vec<Option<Vec<u8>>>, encryptor: &KuznechikEncryptor) {
    for i in 0..data.len() {
        if let Some(c) = data[i].clone() {
            data[i] = Some(encryptor.encrypt_chunk(&c).await.unwrap());
        } else {
            data[i] = None;
        }
    }
}

async fn send_data(data: &Vec<Option<Vec<u8>>>, client: &BroadcastUdpClient) -> Vec<Option<Vec<u8>>> {
    let mut hashes = Vec::with_capacity(data.len());
    for d in data {
        if let Some(c) = d {
            hashes.push(Some(client.send_chunk(c).await.unwrap()));
        } else {
            hashes.push(None);
        }
    }
    hashes
}

async fn handle_recv(path: &PathBuf, decryptor: &KuznechikEncryptor) {
    let (data_hashes, rec_hashes) = serde_json::from_str::<MetaFileInfo>(&fs::read_to_string(path).await.unwrap()).unwrap().deconstruct();

    let client = BroadcastUdpClient::new(
        "0.0.0.0:0",
        "255.255.255.255:62092",
    ).await.unwrap();
    let mut data_chunks = recv_data(&data_hashes, &client).await;
    let mut rec_chunks = recv_data(&rec_hashes, &client).await;

    decrypt_data(&mut data_chunks, &decryptor).await;
    decrypt_data(&mut rec_chunks, &decryptor).await;

    data_chunks.append(&mut rec_chunks);
    let sharer = ReedSolomonSecretSharer::new().unwrap();
    let content = sharer.recover_from_chunks(data_chunks).unwrap();
    fs::write(path, &content).await.unwrap();
}

async fn recv_data(hashes: &Vec<Option<Vec<u8>>>, client: &BroadcastUdpClient) -> Vec<Option<Vec<u8>>> {
    let mut data = Vec::with_capacity(hashes.len());
    for h in hashes {
        if let Some(c) = h {
            data.push(Some(client.recv_chunk(c).await.unwrap()));
        } else {
            data.push(None);
        }
    }
    data
}

async fn decrypt_data(data: &mut Vec<Option<Vec<u8>>>, decryptor: &KuznechikEncryptor) {
    for i in 0..data.len() {
        if let Some(c) = data[i].clone() {
            data[i] = Some(decryptor.decrypt_chunk(&c).await.unwrap());
        } else {
            data[i] = None;
        }
    }
}