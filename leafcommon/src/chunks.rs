#![allow(refining_impl_trait)] // Разрешение на уточнение типов в реализациях трейтов

use std::error::Error; // Трейт ошибок стандартной библиотеки
use std::future::Future; // Трейт асинхронных операций стандартной библиотеки
use std::net::IpAddr; // Перечисление с типами IP-адресов
use std::path::Path; // Структура "сырого" файлового пути
use std::time::Duration; // Структура с длительностью ожидания

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _}; // BASE64-кодек
use serde::{Deserialize, Serialize}; // Трейты (де)сериализации
use tokio::fs; // Асинхронные операции с файловой системой
use tokio::net::UdpSocket; // Асинхронный UDP-сокет
use tokio::time; // Асинхронное ожидание

use crate::crypto::{Encryptor, Hasher}; // Трейты шифровальщика и хэш-вычислителя
use crate::message::Message; // Перечисление сообщений
use crate::shards::SecretSharer; // Трейт разделителя секрета

use consts::*; // Внутренние константы
use errors::*; // Внутренние ошибки

mod consts {
    pub const BROADCAST_ADDR: &str = "255.255.255.255:62092"; // Широковещательный адрес локальной сети с портом
    pub const MAX_UDP_PACKET_SIZE: usize = 65535; // Максимальный размер данных по UDP
}

pub trait ChunkHash<V, S> {
    // Трейт хэша одного чанка
    fn from_chunk(chunk: &[u8], hasher: &Box<dyn Hasher<String>>) -> Self
    where
        Self: Sized; // Метод получения хэша из чанка
    fn get_value(&self) -> V; // Получение значения хэша
    fn get_size(&self) -> S; // Получение размера чанка
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct ReedSolomonChunkHash {
    // Структура хэша чанка, полученного по Риду-Соломону
    value: String, // Значение хэша
    size: usize,   // Размер изначального чанка
}

impl ChunkHash<String, usize> for ReedSolomonChunkHash {
    fn from_chunk(chunk: &[u8], hasher: &Box<dyn Hasher<String>>) -> Self {
        let value = hasher.calc_hash_for_chunk(chunk); // Вычисление хэша
        ReedSolomonChunkHash {
            // Создание объекта структуры
            value,
            size: chunk.len(),
        }
    }

    fn get_value(&self) -> String {
        self.value.clone() // Получение глубокой копии значения
    }

    fn get_size(&self) -> usize {
        self.size // Получение размера чанка
    }
}

pub trait Chunk<H> {
    // Трейт чанка
    fn encrypt(&mut self, encryptor: &Box<dyn Encryptor<Vec<u8>>>) -> Result<(), Box<dyn Error>>; // Метод шифрования чанка
    fn decrypt(&mut self, decryptor: &Box<dyn Encryptor<Vec<u8>>>) -> Result<(), Box<dyn Error>>; // Метод дешифрования чанка
    fn update_hash(&mut self, hasher: &Box<dyn Hasher<String>>) -> Result<(), Box<dyn Error>>; // Метод обновления хэш-суммы чанка
    fn send(
        self,
        socket: &UdpSocket,
        localaddr: IpAddr,
    ) -> impl Future<Output = Result<H, Box<dyn Error>>>; // Метод отправки чанка в сеть
    fn recv(socket: &UdpSocket, hash: &H) -> impl Future<Output = Result<Self, Box<dyn Error>>>
    where
        Self: Sized; // Метод получения чанка из сети
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct ReedSolomonChunk {
    // Структура чанка по Риду-Соломону
    value: Vec<u8>,                     // Данные
    hash: Option<ReedSolomonChunkHash>, // Хэш чанка (при создании равен None)
}

impl Chunk<ReedSolomonChunkHash> for ReedSolomonChunk {
    fn encrypt(&mut self, encryptor: &Box<dyn Encryptor<Vec<u8>>>) -> Result<(), Box<dyn Error>> {
        self.value = encryptor.encrypt_chunk(&self.value); // Переписываем значение на созданное шифровальщиком
        Ok(())
    }

    fn decrypt(&mut self, decryptor: &Box<dyn Encryptor<Vec<u8>>>) -> Result<(), Box<dyn Error>> {
        self.value = decryptor.decrypt_chunk(&self.value)?; // Переписываем значение на созданное дешифровальщиком
        Ok(())
    }

    fn update_hash(&mut self, hasher: &Box<dyn Hasher<String>>) -> Result<(), Box<dyn Error>> {
        self.hash = Some(ReedSolomonChunkHash::from_chunk(&self.value, hasher)); // Получаем значение хэша в Some
        Ok(())
    }

    async fn send(
        self,
        socket: &UdpSocket,
        localaddr: IpAddr,
    ) -> Result<ReedSolomonChunkHash, Box<dyn Error>> {
        let req: Vec<u8> =
            Message::SendingReq(self.hash.clone().unwrap().get_value()).into_bytes()?; // Формируем сообщение SENDING_REQ и преобразуем его в поток байт
        socket.send_to(&req, BROADCAST_ADDR).await?; // Отправляем сообщение в широковещательный домен
        let mut ack = [0u8; MAX_UDP_PACKET_SIZE]; // Создаем буфер для получения ответа
        while let Ok((sz, addr)) =
            time::timeout(Duration::from_secs(5), socket.recv_from(&mut ack)).await?
        // Ожидаем ответ в течение 5 секунд
        {
            let ack = Message::from_bytes(ack[..sz].to_vec())?; // Формируем сообщение из полученных данных
            if !localaddr.eq(&addr.ip()) {
                // Проверяем, что мы не производим обмен сами с собой
                if let Message::SendingAck(h) = ack {
                    // Если сообщение имеет тип SENDING_ACK
                    if h.eq(&self.hash.clone().unwrap().get_value()) {
                        //
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
        hash: &ReedSolomonChunkHash,
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

pub trait Chunks<H> {
    fn from_file(
        path: impl AsRef<Path>,
        sharer: &Box<dyn SecretSharer<Vec<Vec<u8>>, Vec<u8>>>,
    ) -> impl Future<Output = Result<Self, Box<dyn Error>>>
    where
        Self: Sized;
    fn into_file(
        self,
        path: impl AsRef<Path>,
        sharer: &Box<dyn SecretSharer<Vec<Vec<u8>>, Vec<u8>>>,
    ) -> impl Future<Output = Result<(), Box<dyn Error>>>;
    fn encrypt(&mut self, encryptor: &Box<dyn Encryptor<Vec<u8>>>) -> Result<(), Box<dyn Error>>;
    fn decrypt(&mut self, decryptor: &Box<dyn Encryptor<Vec<u8>>>) -> Result<(), Box<dyn Error>>;
    fn update_hashes(&mut self, hasher: &Box<dyn Hasher<String>>) -> Result<(), Box<dyn Error>>;
    fn send(self) -> impl Future<Output = Result<H, Box<dyn Error>>>;
    fn recv(hashes: H) -> impl Future<Output = Result<Self, Box<dyn Error>>>
    where
        Self: Sized;
}

#[derive(Serialize, Deserialize)]
pub struct ReedSolomonChunks {
    data: Vec<ReedSolomonChunk>,
    recv: Vec<ReedSolomonChunk>,
}

impl Chunks<ReedSolomonChunksHashes> for ReedSolomonChunks {
    async fn from_file(
        path: impl AsRef<Path>,
        sharer: &Box<dyn SecretSharer<Vec<Vec<u8>>, Vec<u8>>>,
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
        sharer: &Box<dyn SecretSharer<Vec<Vec<u8>>, Vec<u8>>>,
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

    fn encrypt(&mut self, encryptor: &Box<dyn Encryptor<Vec<u8>>>) -> Result<(), Box<dyn Error>> {
        for c in &mut self.data {
            c.encrypt(encryptor)?;
        }
        for c in &mut self.recv {
            c.encrypt(encryptor)?;
        }
        Ok(())
    }

    fn decrypt(&mut self, decryptor: &Box<dyn Encryptor<Vec<u8>>>) -> Result<(), Box<dyn Error>> {
        for c in &mut self.data {
            c.decrypt(decryptor)?;
        }
        for c in &mut self.recv {
            c.decrypt(decryptor)?;
        }
        Ok(())
    }

    fn update_hashes(&mut self, hasher: &Box<dyn Hasher<String>>) -> Result<(), Box<dyn Error>> {
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

    async fn recv(hashes: ReedSolomonChunksHashes) -> Result<ReedSolomonChunks, Box<dyn Error>> {
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

pub trait ChunksHashes<H> {
    fn save_to(self, path: impl AsRef<Path>) -> impl Future<Output = Result<(), Box<dyn Error>>>;
    fn load_from(path: impl AsRef<Path>) -> impl Future<Output = Result<Self, Box<dyn Error>>>
    where
        Self: Sized;
    fn len(&self) -> usize;
    fn get_data_hash(&self, index: usize) -> H;
    fn get_recv_hash(&self, index: usize) -> H;
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ReedSolomonChunksHashes {
    data: Vec<ReedSolomonChunkHash>,
    recv: Vec<ReedSolomonChunkHash>,
}

impl ChunksHashes<ReedSolomonChunkHash> for ReedSolomonChunksHashes {
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

    fn get_data_hash(&self, index: usize) -> ReedSolomonChunkHash {
        self.data[index].clone()
    }

    fn get_recv_hash(&self, index: usize) -> ReedSolomonChunkHash {
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
