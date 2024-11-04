use std::process;
use tokio::fs;
use leaflibrary::{BroadcastUdpServer, Encryptor, Hasher, KuznechikEncryptor, ReedSolomonSecretSharer, SecretSharer, StreebogHasher};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct MetaFileInfo {
    recovering_level: usize,
    data_parts_hashes: Vec<Option<Vec<u8>>>,
    recovery_parts_hashes: Vec<Vec<Option<Vec<u8>>>>,
}

impl MetaFileInfo {
    pub fn new(recovering_level: usize, data: Vec<Option<Vec<u8>>>, recovery: Vec<Vec<Option<Vec<u8>>>>) -> MetaFileInfo {
        MetaFileInfo {
            recovering_level,
            data_parts_hashes: data,
            recovery_parts_hashes: recovery,
        }
    }

    pub fn deconstruct(self) -> (Vec<Option<Vec<u8>>>, Vec<Vec<Option<Vec<u8>>>>) {
        (self.data_parts_hashes, self.recovery_parts_hashes)
    }
}

impl From<Vec<u8>> for MetaFileInfo {
    fn from(value: Vec<u8>) -> Self {
        let obj: MetaFileInfo = serde_json::from_slice(&value).unwrap();
        obj
    }
}

impl Into<Vec<u8>> for MetaFileInfo {
    fn into(self) -> Vec<u8> {
        serde_json::to_vec(&self).unwrap()
    }
}

#[tokio::main]
async fn main() {
    let recovering_level = 1;
    let content = fs::read("~/snmp.txt").await.unwrap();

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

    match fs::write("~/snmp.txt", <MetaFileInfo as Into<Vec<u8>>>::into(meta)).await {
        Ok(_) => {},
        Err(e) => {
            eprintln!("{}", e.to_string());
            process::exit(9);
        },
    };
}
