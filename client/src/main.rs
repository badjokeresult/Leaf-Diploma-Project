use std::path::PathBuf;

use base64::{prelude::BASE64_STANDARD as BASE64, Engine as _};

use clap::Parser;
use clap_derive::{Parser, ValueEnum}; // Внешняя зависимость для создания интерфейса командной строки

use dialoguer::{theme::ColorfulTheme, Password};

use pnet::datalink;

use serde::{Deserialize, Serialize}; // Внешняя зависимость для сериализации и десериализации объектов
use tokio::{
    fs,
    fs::File,
    io::{AsyncReadExt, BufReader},
    net::UdpSocket,
    time::{self, Duration},
}; // Внешняя зависимость для асинхронной работы с сетью и файловыми операциями

use common::{
    Encryptor, Hasher, KuznechikEncryptor, Message, ReedSolomonChunks, ReedSolomonSecretSharer,
    SecretSharer, StreebogHasher,
}; // Зависимости внутренней библиотеки

use consts::*; // Внутренний модуль с константами
use errors::*; // Внутренний модуль с составными типами ошибок

mod consts {
    pub const LOCAL_ADDR: &str = "0.0.0.0:0";
    pub const BROADCAST_ADDR: &str = "255.255.255.255:62092";
    pub const MAX_UDP_DATAGRAM_SIZE: usize = 1400;
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
    block_size: usize,     // Поле, хранящее размер каждого блока данных
}

impl Metadata {
    // Реализация структуры
    pub fn new(data: Vec<String>, recovery: Vec<String>, block_size: usize) -> Metadata {
        // Конструктор
        Metadata {
            data,
            recovery,
            block_size,
        }
    }

    pub fn get_data(&self) -> Vec<String> {
        // Метод получения глубокой копии хэш-сумм блоков данных
        self.data.clone()
    }

    pub fn get_recv(&self) -> Vec<String> {
        // Метод получения глубокой копии хэш-сумм блоков восстановления
        self.recovery.clone()
    }

    pub fn get_block_size(&self) -> usize {
        self.block_size
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = load_args(); // Получение аргументов командной строки

    let password = Password::with_theme(&ColorfulTheme::default())
        .with_prompt("Enter the password")
        .interact()?;

    let sharer: Box<dyn SecretSharer> = Box::new(ReedSolomonSecretSharer::new()?);
    let encryptor: Box<dyn Encryptor> = Box::new(KuznechikEncryptor::new(&password).await?);

    let path = args.get_file(); // Получение пути к файлу
    match args.get_action() {
        // Выбор действия
        Action::Send => {
            let hasher: Box<dyn Hasher> = Box::new(StreebogHasher::new());
            send_file(path, sharer, encryptor, hasher).await
        } // Вызов метода отправки файла в домен
        Action::Receive => recv_file(path, sharer, encryptor).await, // Вызов метода получения файла из домена
    }
}

async fn send_file(
    filepath: PathBuf,
    sharer: Box<dyn SecretSharer>,
    encryptor: Box<dyn Encryptor>,
    hasher: Box<dyn Hasher>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Метод отправки файла в широковещательный домен
    let file = File::open(&filepath).await?;
    let mut reader = BufReader::new(file);
    let mut buffer = vec![];
    reader.read_to_end(&mut buffer).await?;
    let chunks = sharer.split_into_chunks(&buffer)?; // Разделение файла на блоки
    let (data, recovery) = chunks.deconstruct(); // Разбор блоков на блоки данных и восстановительные
    let block_size = data.first().map_or(
        Err(SendingChunkError(String::from("No data was found"))),
        |v| Ok(v.len()),
    )?; // Получение размера каждого блока для последующей записи в метаданные

    let data = data
        .iter()
        .map(|x| encryptor.encrypt_chunk(x))
        .collect::<Vec<_>>(); // Шифрование каждого блока данных
    let recovery = recovery
        .iter()
        .map(|x| encryptor.encrypt_chunk(x))
        .collect::<Vec<_>>(); // Шифрование каждого блока восстановления

    let data_hash = data
        .iter()
        .map(|x| hasher.calc_hash_for_chunk(x))
        .collect::<Vec<_>>(); // Вычисление хэш-суммы для каждого блока данных
    let recv_hash = recovery
        .iter()
        .map(|x| hasher.calc_hash_for_chunk(x))
        .collect::<Vec<_>>(); // Вычисление хэш-суммы для каждого блока восстановления

    // println!("{:?}", data_hash);
    // println!("{:?}", recv_hash);

    let metadata = Metadata::new(data_hash, recv_hash, block_size); // Создание объекта метаданных

    let socket = UdpSocket::bind(LOCAL_ADDR).await?; // Создание UDP-сокета
    socket.set_broadcast(true)?; // Разрешение сокету на отправку широковещательных запросов
    let data_hashes = metadata.get_data(); // Получение списка хэшей данных
    let recv_hashes = metadata.get_recv(); // Получение списка хэшей восстановления
    for i in 0..data.len() {
        send_chunk(&socket, &data_hashes[i], &data[i]).await?; // Отправляем блоки данных
    }
    for i in 0..recovery.len() {
        send_chunk(&socket, &recv_hashes[i], &recovery[i]).await?; // Отправляем блоки восстановления
    }

    let json = serde_json::to_vec(&metadata)?; // Сериализация в JSON метаданных
    let b64 = BASE64.encode(json); // Кодирование в base64
    fs::write(filepath, &b64).await?; // Запись в исходный файл вместо данных

    Ok(())
}

async fn send_chunk(
    socket: &UdpSocket,
    hash: &str,
    data: &[u8],
    // Метод отправки блока в домен
) -> Result<(), Box<dyn std::error::Error>> {
    let req: Vec<u8> = Message::SendingReq(hash.to_string()).into_bytes()?; // Создание запроса на отправку
    socket.send_to(&req, BROADCAST_ADDR).await?; // Отправка запроса на широковещательный адрес
    println!("Sent {} bytes in REQ", req.len());
    let localaddr = datalink::interfaces()
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
    let mut ack = [0u8; MAX_UDP_DATAGRAM_SIZE]; // Буфер для записи пришедших данных
    while let Ok((sz, addr)) =
        tokio::time::timeout(Duration::from_secs(5), socket.recv_from(&mut ack)).await?
    {
        println!("Received {} bytes IN ACK", sz);
        let ack = Message::from_bytes(ack[..sz].to_vec())?; // Проверка валидности сообщения
        if !localaddr.eq(&addr.ip()) {
            if let Message::SendingAck(h) = ack {
                if h.eq(hash) {
                    let content: Vec<u8> =
                        Message::ContentFilled(hash.to_string(), data.to_vec()).into_bytes()?; // Сборка сообщения с данными
                    socket.send_to(&content, addr).await?; // Отправка сообщения с данными
                    println!("Sent {} bytes in CONTENT", content.len());
                    return Ok(());
                }
            }
            // return Err(Box::new(SendingChunkError(format!(
            //     "Hashes mismatch: orig = {}, recv = {}",
            //     hash, &h
            // ))));
        }
        return Err(Box::new(SendingChunkError(String::from(
            "Invalid message type",
        ))));
    }
    Err(Box::new(SendingChunkError(String::from("No ACK received"))))
}

async fn recv_chunk(
    socket: &UdpSocket,
    hash: &str,
    block_size: usize,
    // Функция получения данных из домена
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let req: Vec<u8> = Message::RetrievingReq(hash.to_string()).into_bytes()?; // Создание запроса на получение
    socket.send_to(&req, BROADCAST_ADDR).await?; // Отправка сообщения на широковещательный адрес
    let mut content = [0u8; MAX_UDP_DATAGRAM_SIZE]; // Буфер для приема сообщения
    if let Ok((sz, _)) =
        tokio::time::timeout(Duration::from_secs(5), socket.recv_from(&mut content)).await?
    {
        let content = Message::from_bytes(content[..sz].to_vec())?; // Проверка корректности сообщения
        if let Message::ContentFilled(h, d) = content {
            // Проверка типа сообщения
            if h.eq(hash) {
                // Проверка равенства хэш-сумм
                if d.len() == block_size {
                    // Проверка равенства размеров блока данных
                    return Ok(d); // Возврат данных
                }
            }
        }
        return Err(Box::new(ReceivingChunkError(hash.to_string())));
    }
    Err(Box::new(ReceivingChunkError(hash.to_string())))
}

async fn recv_file(
    filepath: PathBuf,
    sharer: Box<dyn SecretSharer>,
    encryptor: Box<dyn Encryptor>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Функция восстановления файла
    let content = fs::read_to_string(&filepath).await.unwrap(); // Чтение метаданных файла
    let json = BASE64.decode(content).unwrap(); // Декодирование из base64
    let metadata: Metadata = serde_json::from_slice(&json).unwrap(); // Десериализация из JSON

    let mut data = vec![]; // Вектор блоков данных
    let mut recv = vec![]; // Вектор блоков восстановления
    let socket = UdpSocket::bind("0.0.0.0:0").await.unwrap(); // Создание UDP-сокета
    socket.set_broadcast(true).unwrap(); // Разрешение сокету выполнять широковещательные запросы
    let block_size = metadata.get_block_size(); // Получение размера блока из метаданных
    let data_hashes = metadata.get_data();
    for h in data_hashes.iter() {
        // Для каждой хэш-суммы блоков восстановления
        data.push(
            recv_chunk(&socket, h, block_size)
                .await
                .map_or(vec![], |v| v), // Пытаемся получить данные из сети
                                        // Если данные были получены - добавляем их в вектор
                                        // В противном случае добавляем пустой вектор для индикации
        );
    }
    let recv_hashes = metadata.get_recv(); // Получаем хэш-суммы блоков восстановления
    for i in 0..data.len() {
        if data[i].len() == 0 {
            // Если данный блок данных не был получен
            println!(
                "Data chunk {} was not received, trying to receive recovering one...", // Предупреждаем пользователя
                &data_hashes[i],
            );
            recv.push(
                recv_chunk(&socket, &recv_hashes[i], block_size)
                    .await
                    .expect("Cannot receive both data and recovery chunk, please try again later"),
            ); // Пытаемся получить восстанавливающий блок и в случае неудачи останавливаем программу с предупреждением
        } else {
            recv.push(vec![]); // Если текущий блок данных есть - возвращаем пустой вектор для индикации
        }
    }

    let (mut new_data, mut new_recv) = (vec![], vec![]); // Новые векторы для хранения дешифрованных данных
    for c in data.iter_mut() {
        new_data.push(encryptor.decrypt_chunk(c)?); // Расшифровка блоков данных
    }
    for c in recv.iter_mut() {
        new_recv.push(encryptor.decrypt_chunk(c)?); // Расшифровка блоков восстановления
    }

    let chunks = ReedSolomonChunks::new(new_data, new_recv); // Создание объекта блоков для схемы Рида-Соломона
    let final_content = sharer.recover_from_chunks(chunks).unwrap(); // Восстановление файла из блоков
    fs::write(filepath, final_content).await.unwrap(); // Запись восстановленных данных в исходный файл
    Ok(())
}

mod errors {
    // Модуль с составными типами ошибок
    use std::error::Error; // Зависимость стандартной библиотеки для работы с трейтом ошибок
    use std::fmt; // Зависимость стандартной библиотеки для работы с форматированием

    #[derive(Debug, Clone)]
    pub struct SendingChunkError(pub String); // Тип ошибки отправки блока в домен

    impl fmt::Display for SendingChunkError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Error sending chunk {}", self.0)
        }
    }

    impl Error for SendingChunkError {}

    #[derive(Debug, Clone)]
    pub struct ReceivingChunkError(pub String); // Тип ошибки получения блока из домена

    impl fmt::Display for ReceivingChunkError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Error receiving chunk {}", self.0)
        }
    }

    impl Error for ReceivingChunkError {}
}
