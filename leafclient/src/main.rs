mod args;
mod meta;

use std::path::PathBuf;
use std::process;

use tokio::fs;
use clap::Parser;
use leaflibrary::*;
use meta::MetaFileInfo;

use futures::stream::{StreamExt, TryStreamExt};

use args::Args;

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
    let content = match fs::read(path).await {
        Ok(content) => content,
        Err(e) => {
            eprintln!("failed to read file: {}", e);
            process::exit(1);
        },
    };

    let sharer = ReedSolomonSecretSharer::new().unwrap();
    let chunks = sharer.split_into_chunks(&content).unwrap();

    let encrypted_chunks: Vec<Vec<Option<Vec<u8>>>> = futures::stream::iter(chunks.into_iter())
        .map(|chunk| futures::stream::iter(chunk.into_iter())
            .map(|x| tokio::spawn(async move {
                if let Some(y) = x {
                    Some(encryptor.encrypt_chunk(&y).await.unwrap())
                } else {
                    None
                }
            })))
        .buffered(num_cpus::get())
        .try_collect().await?;

    let client = BroadcastUdpClient::new("0.0.0.0:0", "255.255.255.255:62092").await.unwrap();
    let hashes: Vec<Vec<Option<Vec<u8>>>> = futures::stream::iter(encrypted_chunks.into_iter())
        .map(|chunk| futures::stream::iter(chunk.into_iter())
            .map(|x| tokio::task::spawn(async move {
                if let Some(y) = x {
                    Some(client.send_chunk(&y).await.unwrap())
                } else {
                    None
                }
            })))

    let meta = MetaFileInfo::new(hashes[0].clone(), hashes[1].clone());
    fs::write(path, serde_json::to_vec(&meta).unwrap()).await.unwrap();
}

async fn handle_recv(path: &PathBuf, decryptor: &KuznechikEncryptor) {
    let client = BroadcastUdpClient::new("0.0.0.0:0", "255.255.255.255:62092").await.unwrap();
    let (data, rec) = match fs::read(path).await {
        Ok(m) => MetaFileInfo::from(m).deconstruct(),
        Err(_) => process::exit(3),
    };

    let mut data_chunks: Vec<Option<Vec<u8>>> = data.iter().map(|x| async {
        if let Some(y) = x {
            Some(client.recv_chunk(y).await.unwrap())
        } else {
            None
        }
    }).collect();

    let mut rec_chunks = vec![None; data_chunks.len()];
    let positions = data_chunks.par_iter().positions(|x| !x.is_some()).collect::<Vec<_>>();
    if positions.len() != 0 {
        for pos in positions {
            if let Some(x) = &rec[pos] {
                rec_chunks[pos] = Some(client.recv_chunk(x).await.unwrap());
            }
        }
    }

    data_chunks.append(&mut rec_chunks);

    let data: Vec<Option<Vec<u8>>> = data_chunks.iter().map(|x| async {
        if let Some(y) = x {
            Some(decryptor.decrypt_chunk(y).await.unwrap())
        } else {
            None
        }
    }).collect();

    let sharer = ReedSolomonSecretSharer::new().unwrap();
    let content = sharer.recover_from_chunks(data).unwrap();

    fs::write(path, &content).await.unwrap();
}