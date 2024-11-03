use std::path::PathBuf;
use std::process;

use tokio::fs;
use clap::Parser;

use leaflibrary::{BroadcastUdpServer, Encryptor, Hasher, KuznechikEncryptor, ReedSolomonSecretSharer, SecretSharer, StreebogHasher};
use meta::MetaFileInfo;

mod args;
mod meta;

#[tokio::main]
async fn main() {
    let args = args::Args::parse();

    let file_content = fs::read(&args.file).await.unwrap();
    match args.action.as_str() {
        "send" => handle_send_command(file_content, args.recovering_level, &args.file).await,
        "recv" => handle_recv_command(file_content, args.recovering_level, &args.file).await,
        _ => {
            eprintln!("Unknown command was provided, exit...");
            process::exit(1);
        }
    };
}

async fn handle_send_command(content: Vec<u8>, recovering_level: usize, path: &PathBuf) {
    let sharer = match ReedSolomonSecretSharer::new(recovering_level) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e.to_string());
            process::exit(2);
        },
    };
    let chunks = match sharer.split_into_chunks(&content) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e.to_string());
            process::exit(3);
        },
    };

    let (password_file, gamma_file) = (
        dirs::home_dir().unwrap().join("password.txt"),
        dirs::home_dir().unwrap().join("gamma.bin"));
    let encryptor = match KuznechikEncryptor::new(&password_file, &gamma_file) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{}", e.to_string());
            process::exit(4);
        },
    };
    let mut encrypted_data = vec![];
    let mut encrypted_rec = vec![];
    for chunk in &chunks[0] {
        if let Some(c) = chunk {
            encrypted_data.push(Some(match encryptor.encrypt_chunk(&c).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("{}", e.to_string());
                    process::exit(5);
                },
            }));
        } else {
            encrypted_data.push(None);
        }
    };
    for chunk in &chunks[1..] {
        let mut enc_vec = vec![];
        for inner in chunk {
            if let Some(c) = inner {
                enc_vec.push(Some(match encryptor.encrypt_chunk(c).await {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("{}", e.to_string());
                        process::exit(6);
                    },
                }));
            } else {
                enc_vec.push(None);
            }
        }
        encrypted_rec.push(enc_vec);
    }

    let hasher = StreebogHasher::new();
    let mut data_hashes = vec![];
    let mut recovery_hashes = vec![];
    let server = BroadcastUdpServer::new(
        &dirs::home_dir().unwrap().join("chunks"),
    ).await;
    for chunk in encrypted_data {
        if let Some(c) = chunk {
            let hash = hasher.calc_hash_for_chunk(&c);
            match server.send_chunk(&hash, &c).await {
                Ok(_) => {},
                Err(e) => {
                    eprintln!("{}", e.to_string());
                    process::exit(7);
                },
            };
            data_hashes.push(Some(hash));
        } else {
            data_hashes.push(None);
        }
    }
    for chunk in encrypted_rec {
        let mut hashes = vec![];
        for inner in chunk {
            if let Some(c) = inner {
                let hash = hasher.calc_hash_for_chunk(&c);
                match server.send_chunk(&hash, &c).await {
                    Ok(_) => {},
                    Err(e) => {
                        eprintln!("{}", e.to_string());
                        process::exit(8);
                    },
                };
                hashes.push(Some(hash));
            } else {
                hashes.push(None);
            }
        }
        recovery_hashes.push(hashes);
    };

    let meta = MetaFileInfo::new(recovering_level, data_hashes, recovery_hashes);

    match fs::write(path, <MetaFileInfo as Into<Vec<u8>>>::into(meta)).await {
        Ok(_) => {},
        Err(e) => {
            eprintln!("{}", e.to_string());
            process::exit(9);
        },
    };
}

async fn handle_recv_command(content: Vec<u8>, recovering_level: usize, path: &PathBuf) {
    let meta = MetaFileInfo::from(content);
    let (data_hashes, rec_hashes) = meta.deconstruct();

    let server = BroadcastUdpServer::new(
        &dirs::home_dir().unwrap().join("chunks"),
    ).await;
    let (mut enc_data, mut enc_rec) = (vec![], vec![]);
    for chunk in &data_hashes {
        if let Some(c) = chunk {
            let data = match server.recv_chunk(c).await {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("{}", e.to_string());
                    process::exit(10);
                },
            };
            enc_data.push(Some(data));
        } else {
            enc_data.push(None);
        }
    };
    if enc_data.len() != data_hashes.len() {
        for hash in rec_hashes {
            let mut rec = vec![];
            for inner in hash {
                if let Some(h) = inner {
                    let data = match server.recv_chunk(&h).await {
                        Ok(d) => d,
                        Err(e) => {
                            eprintln!("{}", e.to_string());
                            process::exit(11);
                        },
                    };
                    rec.push(Some(data));
                } else {
                    rec.push(None);
                }
            }
            enc_rec.push(rec);
        };
    };

    let (password_file, gamma_file) = (
        dirs::home_dir().unwrap().join("password.txt"),
        dirs::home_dir().unwrap().join("gamma.bin"),
    );
    let decryptor = match KuznechikEncryptor::new(&password_file, &gamma_file) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{}", e.to_string());
            process::exit(12);
        },
    };
    let mut chunks = vec![];
    for chunk in enc_data {
        if let Some(c) = chunk {
            chunks.push(Some(match decryptor.decrypt_chunk(&c).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("{}", e.to_string());
                    process::exit(13);
                },
            }));
        } else {
            chunks.push(None);
        }
    };
    for part in enc_rec {
        for inner in part {
            if let Some(c) = inner {
                chunks.push(Some(match decryptor.decrypt_chunk(&c).await {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("{}", e.to_string());
                        process::exit(14);
                    },
                }));
            } else {
                chunks.push(None);
            }
        }
    };

    let sharer = match ReedSolomonSecretSharer::new(recovering_level) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e.to_string());
            process::exit(15);
        },
    };
    let content = match sharer.recover_from_chunks(chunks) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e.to_string());
            process::exit(16);
        },
    };

    match fs::write(path, &content).await {
        Ok(_) => {},
        Err(e) => {
            eprintln!("{}", e.to_string());
            process::exit(17);
        },
    };
}
