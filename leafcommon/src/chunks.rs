#![allow(refining_impl_trait)] // Разрешение на уточнение типов в реализациях трейтов

use std::error::Error; // Трейт ошибок стандартной библиотеки
use std::future::Future; // Трейт асинхронных операций стандартной библиотеки
use std::net::IpAddr; // Перечисление с типами IP-адресов
use std::path::Path; // Структура "сырого" файлового пути
use std::time::Duration; // Структура с длительностью ожидания

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _}; // BASE64-кодек
use rayon::prelude::*;
use serde::{Deserialize, Serialize}; // Трейты (де)сериализации
use tokio::fs; // Асинхронные операции с файловой системой
use tokio::net::UdpSocket; // Асинхронный UDP-сокет
use tokio::time; // Асинхронное ожидание

use crate::crypto::{hash::streebog, Encryptor}; // Трейты шифровальщика и хэш-вычислителя
use crate::message::Message; // Перечисление сообщений
use crate::shards::reed_solomon; // Трейт разделителя секрета

use consts::*; // Внутренние константы
use errors::*; // Внутренние ошибки

mod consts {
    pub const CLIENT_ADDR: &str = "0.0.0.0:0";
    pub const BROADCAST_ADDR: &str = "255.255.255.255:62092"; // Широковещательный адрес локальной сети с портом
    pub const MAX_UDP_PACKET_SIZE: usize = 65535; // Максимальный размер данных по UDP
}

pub trait ChunkHash<V, S> {
    // Трейт хэша одного чанка
    fn from_chunk(chunk: &[u8]) -> Self
    where
        Self: Sized; // Метод получения хэша из чанка
    fn get_value(&self) -> V; // Получение значения хэша
    fn get_size(&self) -> S; // Получение размера чанка
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq, Debug)]
pub struct ReedSolomonChunkHash {
    // Структура хэша чанка, полученного по Риду-Соломону
    value: String, // Значение хэша
    size: usize,   // Размер изначального чанка
}

impl ChunkHash<String, usize> for ReedSolomonChunkHash {
    fn from_chunk(chunk: &[u8]) -> Self {
        let value = streebog::calc_hash(chunk); // Вычисление хэша
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

pub trait Chunk<V, S, H> {
    // Трейт чанка
    fn encrypt(&mut self, encryptor: &Box<dyn Encryptor>) -> Result<(), Box<dyn Error>>; // Метод шифрования чанка
    fn decrypt(&mut self, decryptor: &Box<dyn Encryptor>) -> Result<(), Box<dyn Error>>; // Метод дешифрования чанка
    fn update_hash(&mut self) -> Result<(), Box<dyn Error>>; // Метод обновления хэш-суммы чанка
    fn send(
        self,
        socket: &UdpSocket,
        localaddr: IpAddr,
    ) -> impl Future<Output = Result<impl ChunkHash<V, S>, Box<dyn Error>>>; // Метод отправки чанка в сеть
    fn recv(
        socket: &UdpSocket,
        hash: impl ChunkHash<V, S>,
    ) -> impl Future<Output = Result<Self, Box<dyn Error>>>
    where
        Self: Sized; // Метод получения чанка из сети
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq, Debug)]
pub struct ReedSolomonChunk {
    // Структура чанка по Риду-Соломону
    value: Vec<u8>,                     // Данные
    hash: Option<ReedSolomonChunkHash>, // Хэш чанка (при создании равен None)
}

impl Chunk<String, usize, String> for ReedSolomonChunk {
    fn encrypt(&mut self, encryptor: &Box<dyn Encryptor>) -> Result<(), Box<dyn Error>> {
        self.value = encryptor.encrypt_chunk(&self.value); // Переписываем значение на созданное шифровальщиком
        Ok(())
    }

    fn decrypt(&mut self, decryptor: &Box<dyn Encryptor>) -> Result<(), Box<dyn Error>> {
        self.value = decryptor.decrypt_chunk(&self.value)?; // Переписываем значение на созданное дешифровальщиком
        Ok(())
    }

    fn update_hash(&mut self) -> Result<(), Box<dyn Error>> {
        self.hash = Some(ReedSolomonChunkHash::from_chunk(&self.value)); // Получаем значение хэша в Some
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
            time::timeout(Duration::from_secs(10), socket.recv_from(&mut ack)).await?
        // Ожидаем ответ в течение 5 секунд
        {
            let ack = Message::from_bytes(ack[..sz].to_vec())?; // Формируем сообщение из полученных данных
            if !localaddr.eq(&addr.ip()) {
                // Проверяем, что мы не производим обмен сами с собой
                if let Message::SendingAck(h) = ack {
                    // Если сообщение имеет тип SENDING_ACK
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
        hash: impl ChunkHash<String, usize>,
    ) -> Result<ReedSolomonChunk, Box<dyn Error>> {
        let req: Vec<u8> = Message::RetrievingReq(hash.get_value()).into_bytes()?; // Создание запроса на получение
        socket.send_to(&req, BROADCAST_ADDR).await?; // Отправка сообщения на широковещательный адрес
        let mut content = [0u8; MAX_UDP_PACKET_SIZE]; // Буфер для приема сообщения
        if let Ok((sz, _)) =
            time::timeout(Duration::from_secs(10), socket.recv_from(&mut content)).await?
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
                    )))); // Ошибка несоответствия размеров блока
                }
            }
            return Err(Box::new(ReceivingChunkError(String::from(
                "Hash is incorrect",
            )))); // Ошибка несоответствия хэш-сумм
        }
        Err(Box::new(ReceivingChunkError(String::from("Timeout")))) // Ошибка таймаута
    }
}

pub trait Chunks<H> {
    // Трейт для набора чанков
    fn from_file(path: impl AsRef<Path>) -> impl Future<Output = Result<Self, Box<dyn Error>>>
    where
        Self: Sized; // Получение чанков из файла
    fn into_file(self, path: impl AsRef<Path>) -> impl Future<Output = Result<(), Box<dyn Error>>>; // Восстановление файла
    fn encrypt(&mut self, encryptor: &Box<dyn Encryptor>) -> Result<(), Box<dyn Error>>; // Шифрование
    fn decrypt(&mut self, decryptor: &Box<dyn Encryptor>) -> Result<(), Box<dyn Error>>; // Дешифрование
    fn update_hashes(&mut self) -> Result<(), Box<dyn Error>>; // Обновление хэш-сумм
    fn send(self) -> impl Future<Output = Result<H, Box<dyn Error>>>; // Отправка в домен
    fn recv(hashes: H) -> impl Future<Output = Result<Self, Box<dyn Error>>>
    where
        Self: Sized; // Получение из домена
}

#[derive(Serialize, Deserialize)]
pub struct ReedSolomonChunks {
    // Чанки Рида-Соломона
    data: Vec<ReedSolomonChunk>,
    recv: Vec<ReedSolomonChunk>,
}

impl Chunks<ReedSolomonChunksHashes> for ReedSolomonChunks {
    async fn from_file(path: impl AsRef<Path>) -> Result<ReedSolomonChunks, Box<dyn Error>> {
        let content = fs::read(path).await?; // Чтение файла
        let (data, recv) = reed_solomon::split(content)?; // Формирование чанков
        Ok(ReedSolomonChunks {
            data: data
                .par_iter()
                .map(|x| ReedSolomonChunk {
                    value: x.clone(),
                    hash: None,
                })
                .collect::<Vec<_>>(),
            recv: recv
                .par_iter()
                .map(|x| ReedSolomonChunk {
                    value: x.clone(),
                    hash: None,
                })
                .collect::<Vec<_>>(),
        })
    }

    async fn into_file(self, path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
        let data = self
            .data
            .par_iter()
            .map(|x| x.value.clone())
            .collect::<Vec<_>>();
        let recv = self
            .recv
            .par_iter()
            .map(|x| x.value.clone())
            .collect::<Vec<_>>(); // Получение чанков

        let content = reed_solomon::recover(data, recv)?; // Восстановление данных
        fs::write(path, content).await?; // Запись в файл

        Ok(())
    }

    fn encrypt(&mut self, encryptor: &Box<dyn Encryptor>) -> Result<(), Box<dyn Error>> {
        self.data
            .iter_mut()
            .chain(self.recv.iter_mut())
            .try_for_each(|c| c.encrypt(encryptor)) // Шифрование
    }

    fn decrypt(&mut self, decryptor: &Box<dyn Encryptor>) -> Result<(), Box<dyn Error>> {
        self.data
            .iter_mut()
            .chain(self.recv.iter_mut())
            .try_for_each(|c| c.decrypt(decryptor)) // Дешифрование
    }

    fn update_hashes(&mut self) -> Result<(), Box<dyn Error>> {
        self.data
            .iter_mut()
            .chain(self.recv.iter_mut())
            .try_for_each(|c| c.update_hash()) // Обновление хэшей
    }

    async fn send(self) -> Result<ReedSolomonChunksHashes, Box<dyn Error>> {
        let localaddr = pnet::datalink::interfaces()
            .par_iter()
            .find_first(|i| !i.is_loopback() && !i.ips.is_empty())
            .map_or(
                Err(SendingChunkError(String::from("No interface found"))),
                |x| Ok(x),
            )?
            .ips
            .first()
            .map_or(Err(SendingChunkError(String::from("No IP found"))), |x| {
                Ok(x)
            })?
            .ip(); // IP-адрес машины

        let socket = UdpSocket::bind(CLIENT_ADDR).await?;
        socket.set_broadcast(true)?; // Создание сокета

        let (mut data_hashes, mut recv_hashes): (
            Vec<ReedSolomonChunkHash>,
            Vec<ReedSolomonChunkHash>,
        ) = (
            Vec::with_capacity(self.data.len()),
            Vec::with_capacity(self.recv.len()),
        );

        for c in self.data {
            data_hashes.push(c.send(&socket, localaddr).await?);
        }
        for c in self.recv {
            recv_hashes.push(c.send(&socket, localaddr).await?);
        }

        Ok(ReedSolomonChunksHashes {
            data: data_hashes,
            recv: recv_hashes,
        })
    }

    async fn recv(hashes: ReedSolomonChunksHashes) -> Result<ReedSolomonChunks, Box<dyn Error>> {
        let socket = UdpSocket::bind(CLIENT_ADDR).await?;
        socket.set_broadcast(true)?; // Создание сокета
        let mut data = Vec::with_capacity(hashes.len());
        let mut non_received_data_indexes = Vec::with_capacity(hashes.len());
        for i in 0..hashes.len() {
            data.push(match ReedSolomonChunk::recv(&socket, hashes.get_data_hash(i)).await {
                Ok(d) => d, // Получение чанка
                Err(e) => {
                    eprintln!("Error receiving data chunk ({}), trying to receive a recovering one...", e.to_string());
                    non_received_data_indexes.push(i);
                    ReedSolomonChunk {
                        value: vec![0u8; hashes.get_data_hash(i).get_size()],
                        hash: None,
                    }
                }, // Запись пустого чанка и запись его индекса
            });
        }
        let mut recv = Vec::with_capacity(hashes.len());
        let mut is_all_recovery_received = true;
        for i in 0..hashes.len() {
            if !is_all_recovery_received {
                // Если блок не получен - выходим и завершаем
                break;
            }
            if !non_received_data_indexes.contains(&i) {
                recv.push(ReedSolomonChunk {
                    value: vec![0u8; hashes.get_recv_hash(i).get_size()],
                    hash: None,
                });
            }
            recv.push(
                match ReedSolomonChunk::recv(&socket, hashes.get_recv_hash(i)).await {
                    Ok(d) => d, // Получение данных
                    Err(_) => {
                        is_all_recovery_received = false; // Устанавливаем флаг
                        ReedSolomonChunk {
                            value: vec![0u8; hashes.get_recv_hash(i).get_size()],
                            hash: None,
                        } // Запись пустого чанка
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
    fn save_to(self, path: impl AsRef<Path>) -> impl Future<Output = Result<(), Box<dyn Error>>>; // Сохранение метаданных
    fn load_from(path: impl AsRef<Path>) -> impl Future<Output = Result<Self, Box<dyn Error>>>
    where
        Self: Sized; // Чтение метаданных
    fn len(&self) -> usize; // Количество чанков
    fn get_data_hash(&self, index: usize) -> H; // Получение данных
    fn get_recv_hash(&self, index: usize) -> H; // Получение восстановительных данных
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ReedSolomonChunksHashes {
    data: Vec<ReedSolomonChunkHash>,
    recv: Vec<ReedSolomonChunkHash>,
}

impl ChunksHashes<ReedSolomonChunkHash> for ReedSolomonChunksHashes {
    async fn save_to(self, path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
        let data = BASE64.encode(serde_json::to_vec(&self)?); // Сериализация
        fs::write(path, &data).await?; // Запись в файл
        Ok(())
    }

    async fn load_from(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        let content = fs::read(path).await?; // Чтение из файла
        let obj = serde_json::from_slice(&BASE64.decode(&content)?)?; // Десериализация
        Ok(obj)
    }

    fn len(&self) -> usize {
        self.data.len() // Количество чанков одинаково
    }

    fn get_data_hash(&self, index: usize) -> ReedSolomonChunkHash {
        self.data[index].clone() // Получение хэша по индексу
    }

    fn get_recv_hash(&self, index: usize) -> ReedSolomonChunkHash {
        self.recv[index].clone() // Получение хэша по индексу
    }
}

mod errors {
    // Модуль с составными ошибками
    use std::error::Error;
    use std::fmt;
    use std::fmt::{Display, Formatter};

    #[derive(Debug, Clone)]
    pub struct SendingChunkError(pub String); // Ошибка отправки данных

    impl Display for SendingChunkError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error sending chunk: {}", self.0)
        }
    }

    impl Error for SendingChunkError {}

    #[derive(Debug, Clone)]
    pub struct ReceivingChunkError(pub String); // Ошибка получения данных

    impl Display for ReceivingChunkError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error receiving chunk: {}", self.0)
        }
    }

    impl Error for ReceivingChunkError {}
}
