mod args;
mod meta;

use std::path::PathBuf;
use std::process;

use tokio::fs;
use clap::Parser;
use leaflibrary::*;
use meta::MetaFileInfo;

use rayon::prelude::*;
use args::Args;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    match args.action.as_str() {
        "send" => handle_send(&args.file).await,
        "recv" => handle_recv(&args.file).await,
        _ => {},
    }
}

async fn handle_send(path: &PathBuf) {
    let content = match fs::read(path).await {
        Ok(content) => content,
        Err(e) => {
            eprintln!("failed to read file: {}", e);
            process::exit(1);
        },
    };

    let sharer = ReedSolomonSecretSharer::new().unwrap();
    let chunks = sharer.split_into_chunks(&content).unwrap();

    let encryptor = KuznechikEncryptor::new(
        &dirs::home_dir().unwrap().join(".leaf").join("password.txt"),
        &dirs::home_dir().unwrap().join(".leaf").join("gamma.bin"),
    ).unwrap();

    let encrypted_chunks = chunks.par_iter().map(|x| x.par_iter().map(|y| async {
        if let Some(z) = y {
            Some(encryptor.encrypt_chunk(z).await.unwrap())
        } else {
            None
        }
    }).collect::<Vec<Option<Vec<_>>>>()).collect::<Vec<_>>();

    let client = BroadcastUdpClient::new("0.0.0.0:0", "255.255.255.255:62092").await.unwrap();
    let hashes = encrypted_chunks.par_iter().map(|x| x.par_iter().map(|y| async {
        if let Some(z) = y {
            Some(client.send_chunk(z).await.unwrap())
        } else {
            None
        }
    }).collect::<Vec<Option<Vec<_>>>>()).collect::<Vec<_>>();

    let meta = MetaFileInfo::new(hashes[0].clone(), hashes[1].clone());
    fs::write(path, serde_json::to_vec(&meta).unwrap()).await.unwrap();
}

async fn handle_recv(path: &PathBuf) {
    let client = BroadcastUdpClient::new("0.0.0.0:0", "255.255.255.255:62092").await.unwrap();
    let (data, rec) = match fs::read(path).await {
        Ok(m) => MetaFileInfo::from(m).deconstruct(),
        Err(_) => process::exit(3),
    };

    let mut data_chunks = data.par_iter().map(|x| async {
        if let Some(y) = x {
            Some(client.recv_chunk(y).await.unwrap())
        } else {
            None
        }
    }).collect::<Vec<Option<Vec<_>>>>();

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

    let decryptor = KuznechikEncryptor::new(
        &dirs::home_dir().unwrap().join(".leaf").join("password.txt"),
        &dirs::home_dir().unwrap().join(".leaf").join("gamma.bin"),
    ).unwrap();

    let data = data_chunks.par_iter().map(|x| async {
        if let Some(y) = x {
            Some(decryptor.decrypt_chunk(y).await.unwrap())
        } else {
            None
        }
    }).collect::<Vec<Option<Vec<_>>>>();

    let sharer = ReedSolomonSecretSharer::new().unwrap();
    let content = sharer.recover_from_chunks(data).unwrap();

    fs::write(path, &content).await.unwrap();
}