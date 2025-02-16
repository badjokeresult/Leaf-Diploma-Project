use std::path::PathBuf;

use base64::{prelude::BASE64_STANDARD as BASE64, Engine as _};

use clap::Parser;
use clap_derive::{Parser, ValueEnum}; // Внешняя зависимость для создания интерфейса командной строки

use serde::{Deserialize, Serialize}; // Внешняя зависимость для сериализации и десериализации объектов
use tokio::{fs, net::UdpSocket}; // Внешняя зависимость для асинхронной работы с сетью и файловыми операциями

use rayon::prelude::*;

use common::{
    Encryptor, Hasher, KuznechikEncryptor, Message, ReedSolomonChunks, ReedSolomonSecretSharer,
    SecretSharer, StreebogHasher,
}; // Зависимости внутренней библиотеки

use consts::*; // Внутренний модуль с константами
use errors::*; // Внутренний модуль с составными типами ошибок

mod consts {
    pub const LOCAL_ADDR: &str = "0.0.0.0:0";
    pub const BROADCAST_ADDR: &str = "255.255.255.255:62092";
    pub const MAX_UDP_DATAGRAM_SIZE: usize = 65527;
}

#[derive(Parser, Debug)] // Автоматические реализации трейтов
#[command(version, about, long_about = None)] // Автоматическая реализация трейта для интерфейса командной строки
pub struct Args {
    // Тип структуры, отвечающий за обработку аргументов командной строки
    #[arg(value_enum, short, long)]
    action: Action, // Поле, отвечающее за выполняемое клиентом действие с файлом
    #[arg(short, long)]
    file: String, // Поле, отвечающее за получение пути к файлу
}

impl Args {
    // Реализация структуры
    pub fn get_action(&self) -> Action {
        // Метод получения типа действия
        self.action
    }

    pub fn get_file(&self) -> PathBuf {
        // Метод получения пути файла
        PathBuf::from(&self.file)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, ValueEnum)]
pub enum Action {
    // Тип перечисления, отвечающий за обработку типов действий с файлом для клиента
    Send,    // Действие - отправить файл в домен
    Receive, // Действие - получить файл из домена
}

pub fn load_args() -> Args {
    // Метод получения аргументов командной строки
    Args::parse()
}

#[derive(Serialize, Deserialize)] // Автоматическая реализация функций сериализации и десериализации
struct Metadata {
    // Тип метаданных файла, хранит хэш-суммы каждого чанка файла
    data: Vec<String>,     // Поле, хранящее хэш-суммы блоков данных
    recovery: Vec<String>, // Поле, хранящее хэш-суммы блоков восстановления
}

impl Metadata {
    // Реализация структуры
    pub fn new(data: Vec<String>, recovery: Vec<String>) -> Metadata {
        // Конструктор
        Metadata { data, recovery }
    }

    pub fn get_data(&self) -> Vec<String> {
        // Метод получения глубокой копии хэш-сумм блоков данных
        self.data.clone()
    }

    pub fn get_recv(&self) -> Vec<String> {
        // Метод получения глубокой копии хэш-сумм блоков восстановления
        self.recovery.clone()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = load_args(); // Получение аргументов командной строки

    let path = args.get_file(); // Получение пути к файлу
    match args.get_action() {
        // Выбор действия
        Action::Send => send_file(path).await, // Вызов метода отправки файла в домен
        Action::Receive => recv_file(path).await, // Вызов метода получения файла из домена
    }
}

async fn send_file(filepath: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Метод отправки файла в широковещательный домен
    let content = fs::read(&filepath).await?; // Чтение содержимого файла
    let sharer = ReedSolomonSecretSharer::new()?; // Создание экземпляра разделителя
    let chunks = sharer.split_into_chunks(&content)?; // Разделение файла на блоки
    let (data, recovery) = chunks.deconstruct(); // Разбор блоков на блоки данных и восстановительные

    let password = std::env::var("PASSWORD")?; // TODO: Написать нормальный сбор пароля
    let encryptor = KuznechikEncryptor::new(&password).await?; // Создание шифратора
    let data = data
        .par_iter()
        .map(|x| encryptor.encrypt_chunk(x))
        .collect::<Vec<_>>();
    let recovery = recovery
        .par_iter()
        .map(|x| encryptor.encrypt_chunk(x))
        .collect::<Vec<_>>();

    let hasher = StreebogHasher::new();
    let data_hash = data
        .par_iter()
        .map(|x| hasher.calc_hash_for_chunk(x))
        .collect::<Vec<_>>();
    let recv_hash = recovery
        .par_iter()
        .map(|x| hasher.calc_hash_for_chunk(x))
        .collect::<Vec<_>>();

    let metadata = Metadata::new(data_hash, recv_hash);

    let socket = UdpSocket::bind(LOCAL_ADDR).await?;
    socket.set_broadcast(true)?;
    let data_hashes = metadata.get_data();
    let recv_hashes = metadata.get_recv();
    for i in 0..data.len() {
        send_chunk(&socket, &data_hashes[i], &data[i]).await?;
    }
    for i in 0..recovery.len() {
        send_chunk(&socket, &recv_hashes[i], &recovery[i]).await?;
    }

    let json = serde_json::to_vec(&metadata)?;
    let b64 = BASE64.encode(json);
    fs::write(filepath, &b64).await?;

    Ok(())
}

async fn send_chunk(
    socket: &UdpSocket,
    hash: &str,
    data: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let req: Vec<u8> = Message::SendingReq(hash.to_string()).into_bytes()?;
    socket.send_to(&req, BROADCAST_ADDR).await?;
    let mut ack = [0u8; MAX_UDP_DATAGRAM_SIZE];
    while let Ok((sz, addr)) = socket.recv_from(&mut ack).await {
        let ack = Message::from_bytes(ack[..sz].to_vec())?;
        if let Message::SendingAck(h) = ack {
            if h.eq(hash) {
                let content: Vec<u8> =
                    Message::ContentFilled(hash.to_string(), data.to_vec()).into_bytes()?;
                socket.send_to(&content, addr).await?;
                return Ok(());
            }
        }
    }
    Err(Box::new(SendingChunkError(hash.to_string())))
}

async fn recv_chunk(socket: &UdpSocket, hash: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let req: Vec<u8> = Message::RetrievingReq(hash.to_string()).into_bytes()?;
    socket.send_to(&req, BROADCAST_ADDR).await?;
    let mut content = [0u8; MAX_UDP_DATAGRAM_SIZE];
    if let Ok((sz, _)) = socket.recv_from(&mut content).await {
        let content = Message::from_bytes(content[..sz].to_vec())?;
        if let Message::ContentFilled(h, d) = content {
            if h.eq(hash) {
                return Ok(d);
            }
        }
    }
    Err(Box::new(ReceivingChunkError(hash.to_string())))
}

async fn recv_file(filepath: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(&filepath).await.unwrap();
    let json = BASE64.decode(content).unwrap();
    let metadata: Metadata = serde_json::from_slice(&json).unwrap();

    let mut data = vec![];
    let mut recv = vec![];
    let socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();
    socket.set_broadcast(true).unwrap();
    for h in metadata.get_data().iter() {
        data.push(recv_chunk(&socket, h).await?);
    }
    for h in metadata.get_recv().iter() {
        recv.push(recv_chunk(&socket, h).await?);
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

    let chunks = ReedSolomonChunks::new(data, recv);
    let sharer = ReedSolomonSecretSharer::new().unwrap();
    let final_content = sharer.recover_from_chunks(chunks).unwrap();
    fs::write(filepath, final_content).await.unwrap();
    Ok(())
}

mod errors {
    use std::error::Error;
    use std::fmt;

    #[derive(Debug, Clone)]
    pub struct SendingChunkError(pub String);

    impl fmt::Display for SendingChunkError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Error sending chunk {}", self.0)
        }
    }

    impl Error for SendingChunkError {}

    #[derive(Debug, Clone)]
    pub struct ReceivingChunkError(pub String);

    impl fmt::Display for ReceivingChunkError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Error receiving chunk {}", self.0)
        }
    }

    impl Error for ReceivingChunkError {}
}
