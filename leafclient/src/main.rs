mod args;
mod meta;

use std::path::PathBuf;
use std::process;

use tokio::fs;
use clap::Parser;
use leaflibrary::{BroadcastUdpClient, Encryptor, KuznechikEncryptor, ReedSolomonSecretSharer, SecretSharer};
use meta::MetaFileInfo;

use args::Args;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    match args.action.as_str() {
        "send" => handle_send(&args.file, args.recovering_level).await,
        "recv" => handle_recv(&args.file, args.recovering_level).await,
        _ => {},
    }
}

async fn handle_send(path: &PathBuf, recovering_level: usize) {
    let content = match fs::read(path).await {
        Ok(content) => content,
        Err(e) => {
            eprintln!("failed to read file: {}", e);
            process::exit(1);
        },
    };

    let sharer = ReedSolomonSecretSharer::new(recovering_level).unwrap();
    let chunks = sharer.split_into_chunks(&content).unwrap();

    let (mut enc_data, mut enc_rec) = (vec![], vec![]);
    let encryptor = KuznechikEncryptor::new(
        &dirs::home_dir().unwrap().join(".leaf").join("password.txt"),
        &dirs::home_dir().unwrap().join(".leaf").join("gamma.bin"),
    ).unwrap();
    for chunk in &chunks[0] {
        if let Some(c) = chunk {
            enc_data.push(Some(encryptor.encrypt_chunk(c).await.unwrap()));
        } else {
            enc_data.push(None);
        }
    }
    for chunk in &chunks[1..] {
        let mut crypt = vec![];
        for inner in chunk {
            if let Some(c) = inner {
                crypt.push(Some(encryptor.encrypt_chunk(c).await.unwrap()));
            } else {
                crypt.push(None);
            }
        }
        enc_rec.append(&mut crypt);
    }

    let client = BroadcastUdpClient::new("0.0.0.0:0", "255.255.255.255:62092").await;
    let (mut data_hashes, mut rec_hashes) = (vec![], vec![]);
    for chunk in &chunks[0] {
        if let Some(c) = chunk {
            data_hashes.push(Some(client.send_data(c).await.unwrap()));
        } else {
            data_hashes.push(None);
        }
    }
    for chunk in &chunks[1..] {
        let mut hashes = vec![];
        for inner in chunk {
            if let Some(c) = inner {
                hashes.push(Some(client.send_data(c).await.unwrap()));
            } else {
                hashes.push(None);
            }
        }
        rec_hashes.push(hashes);
    }

    let meta = MetaFileInfo::new(recovering_level, data_hashes, rec_hashes);
    fs::write(path, serde_json::to_vec(&meta).unwrap()).await.unwrap();
}

async fn handle_recv(path: &PathBuf, recovering_level: usize) {
    let client = BroadcastUdpClient::new("0.0.0.0:0", "255.255.255.255:62092").await;
    let meta: MetaFileInfo = match fs::read(path).await {
        Ok(m) => serde_json::from_slice(&m).unwrap(),
        Err(_) => process::exit(3),
    };
    let (data_hashes, rec_hashes) = meta.deconstruct();
    let (mut enc_data, mut enc_rec) = (vec![], vec![]);
    for hash in data_hashes {
        if let Some(h) = hash {
            enc_data.push(Some(client.recv_data(&h).await.unwrap()));
        } else {
            enc_data.push(None);
        }
    }
    for hash in rec_hashes {
        let mut chunks = vec![];
        for inner in hash {
            if let Some(h) = inner {
                chunks.push(Some(client.recv_data(&h).await.unwrap()));
            } else {
                chunks.push(None);
            }
        }
        enc_rec.push(chunks);
    }

    let (mut data, mut rec) = (vec![], vec![]);
    let decryptor = KuznechikEncryptor::new(
        &dirs::home_dir().unwrap().join(".leaf").join("password.txt"),
        &dirs::home_dir().unwrap().join(".leaf").join("gamma.bin"),
    ).unwrap();
    for chunk in enc_data {
        if let Some(c) = chunk {
            data.push(Some(decryptor.decrypt_chunk(&c).await.unwrap()));
        } else {
            data.push(None);
        }
    }
    for chunk in enc_rec {
        let mut chunks = vec![];
        for inner in chunk {
            if let Some(c) = inner {
                chunks.push(Some(decryptor.decrypt_chunk(&c).await.unwrap()));
            } else {
                chunks.push(None);
            }
        }
        rec.push(chunks);
    }
    for chunk in &mut rec {
        data.append(chunk);
    }

    let sharer = ReedSolomonSecretSharer::new(recovering_level).unwrap();
    let content = sharer.recover_from_chunks(data).unwrap();

    fs::write(path, &content).await.unwrap();
}