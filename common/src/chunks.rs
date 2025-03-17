#![allow(refining_impl_trait)]

use std::error::Error;
use std::future::Future;
use std::net::IpAddr;
use std::path::Path;
use std::time::Duration;

use base64::{prelude::BASE64_STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::net::UdpSocket;
use tokio::time;

use crate::crypto::{Encryptor, Hasher};
use crate::message::Message;
use crate::shards::SecretSharer;

use consts::*;
use errors::*;

mod consts {
    pub const BROADCAST_ADDR: &str = "255.255.255.255:62092";
    pub const MAX_UDP_PACKET_SIZE: usize = 65535;
}

pub trait ChunkHash {
    fn from_chunk(chunk: &[u8], hasher: &Box<dyn Hasher>) -> impl ChunkHash;
    fn get_value(&self) -> String;
    fn get_size(&self) -> usize;
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct ReedSolomonChunkHash {
    value: String,
    size: usize,
}

impl ChunkHash for ReedSolomonChunkHash {
    fn from_chunk(chunk: &[u8], hasher: &Box<dyn Hasher>) -> ReedSolomonChunkHash {
        let value = hasher.calc_hash_for_chunk(chunk);
        ReedSolomonChunkHash {
            value,
            size: chunk.len(),
        }
    }

    fn get_value(&self) -> String {
        self.value.clone()
    }

    fn get_size(&self) -> usize {
        self.size
    }
}

pub trait Chunk {
    fn encrypt(&mut self, encryptor: &Box<dyn Encryptor>) -> Result<(), Box<dyn Error>>;
    fn decrypt(&mut self, decryptor: &Box<dyn Encryptor>) -> Result<(), Box<dyn Error>>;
    fn update_hash(&mut self, hasher: &Box<dyn Hasher>) -> Result<(), Box<dyn Error>>;
    fn send(
        self,
        socket: &UdpSocket,
        localaddr: IpAddr,
    ) -> impl Future<Output = Result<impl ChunkHash, Box<dyn Error>>>;
    fn recv(
        socket: &UdpSocket,
        hash: &impl ChunkHash,
    ) -> impl Future<Output = Result<impl Chunk, Box<dyn Error>>>;
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct ReedSolomonChunk {
    value: Vec<u8>,
    hash: Option<ReedSolomonChunkHash>,
}

impl Chunk for ReedSolomonChunk {
    fn encrypt(&mut self, encryptor: &Box<dyn Encryptor>) -> Result<(), Box<dyn Error>> {
        self.value = encryptor.encrypt_chunk(&self.value);
        Ok(())
    }

    fn decrypt(&mut self, decryptor: &Box<dyn Encryptor>) -> Result<(), Box<dyn Error>> {
        self.value = decryptor.decrypt_chunk(&self.value)?;
        Ok(())
    }

    fn update_hash(&mut self, hasher: &Box<dyn Hasher>) -> Result<(), Box<dyn Error>> {
        self.hash = Some(ReedSolomonChunkHash {
            value: hasher.calc_hash_for_chunk(&self.value),
            size: self.value.len(),
        });
        Ok(())
    }

    async fn send(
        self,
        socket: &UdpSocket,
        localaddr: IpAddr,
    ) -> Result<ReedSolomonChunkHash, Box<dyn Error>> {
        let req: Vec<u8> =
            Message::SendingReq(self.hash.clone().unwrap().get_value()).into_bytes()?;
        socket.send_to(&req, BROADCAST_ADDR).await?;
        let mut ack = [0u8; MAX_UDP_PACKET_SIZE];
        while let Ok((sz, addr)) =
            time::timeout(Duration::from_secs(5), socket.recv_from(&mut ack)).await?
        {
            let ack = Message::from_bytes(ack[..sz].to_vec())?;
            if !localaddr.eq(&addr.ip()) {
                if let Message::SendingAck(h) = ack {
                    if h.eq(&self.hash.clone().unwrap().get_value()) {
                        let content: Vec<u8> = Message::ContentFilled(
                            self.hash.clone().unwrap().get_value(),
                            self.value,
                        )
                        .into_bytes()?;
                        socket.send_to(&content, addr).await?;
                        return Ok(self.hash.unwrap());
                    }
                }
            }
        }
        Err(Box::new(SendingChunkError(String::from("Timeout"))))
    }

    async fn recv(
        socket: &UdpSocket,
        hash: &impl ChunkHash,
    ) -> Result<ReedSolomonChunk, Box<dyn Error>> {
        let req: Vec<u8> = Message::RetrievingReq(hash.get_value()).into_bytes()?; // Создание запроса на получение
        socket.send_to(&req, BROADCAST_ADDR).await?; // Отправка сообщения на широковещательный адрес
        let mut content = [0u8; MAX_UDP_PACKET_SIZE]; // Буфер для приема сообщения
        if let Ok((sz, _)) =
            time::timeout(Duration::from_secs(5), socket.recv_from(&mut content)).await?
        {
            let content = Message::from_bytes(content[..sz].to_vec())?; // Проверка корректности сообщения
            if let Message::ContentFilled(h, d) = content {
                // Проверка типа сообщения
                if h.eq(&hash.get_value()) {
                    // Проверка равенства хэш-сумм
                    if d.len() == hash.get_size() {
                        // Проверка равенства размеров блока данных
                        return Ok(ReedSolomonChunk {
                            value: d,
                            hash: None,
                        }); // Возврат данных
                    }
                    return Err(Box::new(ReceivingChunkError(String::from(
                        "Blocks sizes mismatch",
                    ))));
                }
            }
            return Err(Box::new(ReceivingChunkError(String::from(
                "Hash is incorrect",
            ))));
        }
        Err(Box::new(ReceivingChunkError(String::from("Timeout"))))
    }
}

pub trait Chunks {
    fn from_file(
        path: impl AsRef<Path>,
        sharer: &Box<dyn SecretSharer>,
    ) -> impl Future<Output = Result<impl Chunks, Box<dyn Error>>>;
    fn into_file(
        self,
        path: impl AsRef<Path>,
        sharer: &Box<dyn SecretSharer>,
    ) -> impl Future<Output = Result<(), Box<dyn Error>>>;
    fn encrypt(&mut self, encryptor: &Box<dyn Encryptor>) -> Result<(), Box<dyn Error>>;
    fn decrypt(&mut self, decryptor: &Box<dyn Encryptor>) -> Result<(), Box<dyn Error>>;
    fn update_hashes(&mut self, hasher: &Box<dyn Hasher>) -> Result<(), Box<dyn Error>>;
    fn send(self) -> impl Future<Output = Result<impl ChunksHashes, Box<dyn Error>>>;
    fn recv(hashes: impl ChunksHashes)
        -> impl Future<Output = Result<impl Chunks, Box<dyn Error>>>;
}

#[derive(Serialize, Deserialize)]
pub struct ReedSolomonChunks {
    data: Vec<ReedSolomonChunk>,
    recv: Vec<ReedSolomonChunk>,
}

impl Chunks for ReedSolomonChunks {
    async fn from_file(
        path: impl AsRef<Path>,
        sharer: &Box<dyn SecretSharer>,
    ) -> Result<ReedSolomonChunks, Box<dyn Error>> {
        let content = fs::read(path).await?;
        let (data, recv) = sharer.split_into_chunks(&content)?;
        Ok(ReedSolomonChunks {
            data: data
                .iter()
                .map(|x| ReedSolomonChunk {
                    value: x.clone(),
                    hash: None,
                })
                .collect::<Vec<_>>(),
            recv: recv
                .iter()
                .map(|x| ReedSolomonChunk {
                    value: x.clone(),
                    hash: None,
                })
                .collect::<Vec<_>>(),
        })
    }

    async fn into_file(
        self,
        path: impl AsRef<Path>,
        sharer: &Box<dyn SecretSharer>,
    ) -> Result<(), Box<dyn Error>> {
        let data = self
            .data
            .iter()
            .map(|x| x.value.clone())
            .collect::<Vec<_>>();
        let recv = self
            .recv
            .iter()
            .map(|x| x.value.clone())
            .collect::<Vec<_>>();

        let content = sharer.recover_from_chunks((data, recv))?;
        fs::write(path, content).await?;

        Ok(())
    }

    fn encrypt(&mut self, encryptor: &Box<dyn Encryptor>) -> Result<(), Box<dyn Error>> {
        for c in &mut self.data {
            c.encrypt(encryptor)?;
        }
        for c in &mut self.recv {
            c.encrypt(encryptor)?;
        }
        Ok(())
    }

    fn decrypt(&mut self, decryptor: &Box<dyn Encryptor>) -> Result<(), Box<dyn Error>> {
        for c in &mut self.data {
            c.decrypt(decryptor)?;
        }
        for c in &mut self.recv {
            c.decrypt(decryptor)?;
        }
        Ok(())
    }

    fn update_hashes(&mut self, hasher: &Box<dyn Hasher>) -> Result<(), Box<dyn Error>> {
        for c in &mut self.data {
            c.update_hash(hasher)?;
        }
        for c in &mut self.recv {
            c.update_hash(hasher)?;
        }
        Ok(())
    }

    async fn send(self) -> Result<ReedSolomonChunksHashes, Box<dyn Error>> {
        let localaddr = pnet::datalink::interfaces()
            .iter()
            .find(|i| !i.is_loopback() && !i.ips.is_empty())
            .map_or(
                Err(SendingChunkError(String::from("No interface found"))),
                |x| Ok(x),
            )?
            .ips
            .first()
            .map_or(Err(SendingChunkError(String::from("No IP found"))), |x| {
                Ok(x)
            })?
            .ip();

        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.set_broadcast(true)?;

        let mut data_hashes = Vec::with_capacity(self.data.len());
        for c in self.data {
            data_hashes.push(c.send(&socket, localaddr).await?);
        }
        let mut recv_hashes = Vec::with_capacity(self.recv.len());
        for c in self.recv {
            recv_hashes.push(c.send(&socket, localaddr).await?);
        }
        Ok(ReedSolomonChunksHashes {
            data: data_hashes,
            recv: recv_hashes,
        })
    }

    async fn recv(hashes: impl ChunksHashes) -> Result<ReedSolomonChunks, Box<dyn Error>> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.set_broadcast(true)?;
        let mut data = Vec::with_capacity(hashes.len());
        let mut non_received_data_indexes = Vec::with_capacity(hashes.len());
        for i in 0..hashes.len() {
            data.push(match ReedSolomonChunk::recv(&socket, &hashes.get_data_hash(i)).await {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Error receiving data chunk ({}), trying to receive a recovering one...", e.to_string());
                    non_received_data_indexes.push(i);
                    ReedSolomonChunk {
                        value: vec![0u8; hashes.get_data_hash(i).get_size()],
                        hash: None,
                    }
                },
            });
        }
        let mut recv = Vec::with_capacity(hashes.len());
        let mut is_all_recovery_received = true;
        for i in non_received_data_indexes {
            if !is_all_recovery_received {
                break;
            }
            recv.push(
                match ReedSolomonChunk::recv(&socket, &hashes.get_recv_hash(i)).await {
                    Ok(d) => d,
                    Err(_) => {
                        is_all_recovery_received = false;
                        ReedSolomonChunk {
                            value: vec![0u8; hashes.get_recv_hash(i).get_size()],
                            hash: None,
                        }
                    }
                },
            )
        }
        if !is_all_recovery_received {
            return Err(Box::new(ReceivingChunkError(String::from(
                "Could not receive both data and recovery chunks",
            ))));
        }
        Ok(ReedSolomonChunks { data, recv })
    }
}

pub trait ChunksHashes {
    fn save_to(self, path: impl AsRef<Path>) -> impl Future<Output = Result<(), Box<dyn Error>>>;
    fn load_from(
        path: impl AsRef<Path>,
    ) -> impl Future<Output = Result<impl ChunksHashes, Box<dyn Error>>>;
    fn len(&self) -> usize;
    fn get_data_hash(&self, index: usize) -> impl ChunkHash;
    fn get_recv_hash(&self, index: usize) -> impl ChunkHash;
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ReedSolomonChunksHashes {
    data: Vec<ReedSolomonChunkHash>,
    recv: Vec<ReedSolomonChunkHash>,
}

impl ChunksHashes for ReedSolomonChunksHashes {
    async fn save_to(self, path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
        let data = BASE64.encode(serde_json::to_vec(&self)?);
        fs::write(path, &data).await?;
        Ok(())
    }

    async fn load_from(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        let content = fs::read(path).await?;
        let obj = serde_json::from_slice(&BASE64.decode(&content)?)?;
        Ok(obj)
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn get_data_hash(&self, index: usize) -> impl ChunkHash {
        self.data[index].clone()
    }

    fn get_recv_hash(&self, index: usize) -> impl ChunkHash {
        self.recv[index].clone()
    }
}

mod errors {
    use std::error::Error;
    use std::fmt;
    use std::fmt::{Display, Formatter};

    #[derive(Debug, Clone)]
    pub struct SendingChunkError(pub String);

    impl Display for SendingChunkError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error sending chunk: {}", self.0)
        }
    }

    impl Error for SendingChunkError {}

    #[derive(Debug, Clone)]
    pub struct ReceivingChunkError(pub String);

    impl Display for ReceivingChunkError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error receiving chunk: {}", self.0)
        }
    }

    impl Error for ReceivingChunkError {}
}
