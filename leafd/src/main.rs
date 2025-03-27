mod socket; // Объявление внутреннего модуля сокета
mod stor; // Объявление внутреннего модуля хранилища

use std::{net::SocketAddr, path::PathBuf};

use socket::{Packet, Socket};
use stor::{ServerStorage, UdpServerStorage};
use tokio::sync::mpsc::Receiver;

use leafcommon::Message;

use consts::*; // Внутренний модуль с константами
use errors::*;

mod consts {
    // Константы для Linux компилируются для вызывающего кода
    #[cfg(target_os = "linux")]
    pub const APPS_DIR_ABS_PATH: &str = "/var/local"; // Абсолютный путь к корню директории хранилища
    #[cfg(target_os = "linux")]
    pub const APP_DIR: &str = "leaf"; // Директория приложения
    #[cfg(target_os = "linux")]
    pub const CHUNKS_DIR: &str = "chunks"; // Директория чанков
    #[cfg(target_os = "linux")]
    pub const STATE_FILE: &str = "last_state.bin";

    #[cfg(target_os = "windows")]
    pub const APPS_DIR_ABS_PATH: &str = "C:\\Program Files"; // Корень директории с приложениями
    #[cfg(target_os = "windows")]
    pub const APP_DIR: &str = "Leaf"; // Корень приложения
    #[cfg(target_os = "windows")]
    pub const CHUNKS_DIR: &str = "Chunks"; // Директория хранилища
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket = Socket::new().await?; // Создаем объект сокета

    let base_path = PathBuf::from(APPS_DIR_ABS_PATH).join(APP_DIR);
    let stor_path = base_path.join(CHUNKS_DIR);
    let state_path = base_path.join(STATE_FILE);
    let storage = UdpServerStorage::new(stor_path, state_path).await?;
    let socket_clone = socket.clone();
    let mut storage_clone = storage.clone(); // Клонируем поля для корректного перемещения в поток
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    tokio::spawn(async move {
        packet_handler(rx, &mut storage_clone, &socket_clone).await;
    });

    loop {
        socket.recv(&tx).await;
    }
}

async fn packet_handler(mut rx: Receiver<Packet>, storage: &mut UdpServerStorage, socket: &Socket) {
    // Функция-обработчик сообщений
    while let Some(p) = rx.recv().await {
        // Ожидание новых данных из канала в сокете
        process_packet(p, storage, &socket).await;
    }
}

async fn process_packet(
    // Функция обработки отдельного пакета
    packet: Packet,
    storage: &mut UdpServerStorage,
    socket: &Socket,
) {
    let (data, addr) = packet.deconstruct(); // Разбираем пакет на данные и адрес источника
    let message = Message::from_bytes(data).unwrap(); // Восстанавливаем сообщение из потока байт
    match message.clone() {
        Message::SendingReq(h) => {
            if let Err(e) = send_sending_ack(h.clone(), addr, socket, storage).await {
                eprintln!("{}", e.to_string());
            }
        }
        Message::RetrievingReq(h) => {
            if let Err(e) = send_content_filled(h.clone(), addr, socket, storage).await {
                eprintln!("{}", e.to_string());
            }
        }
        Message::ContentFilled(h, d) => {
            // Если сообщение содержит данные
            if let Err(e) = storage.save(&h, &d).await {
                eprintln!("{}", e.to_string());
            };
        }
        _ => eprintln!(
            "{:?}",
            Err::<(), Box<InvalidMessageError>>(Box::new(InvalidMessageError))
        ), // Если пришли любые другие данные - возвращаем ошибку
    }
}

async fn send_sending_ack(
    hash: String,
    addr: SocketAddr,
    socket: &Socket,
    storage: &UdpServerStorage,
) -> Result<(), SendingAckError> {
    if storage.can_save() {
        // Проверка доступного места на диске
        let ack = Message::SendingAck(hash)
            .into_bytes()
            .map_err(|e| SendingAckError(e.to_string()))?; // Создание сообщения подтверждения хранения и перевод его в поток байт
        let packet = Packet::new(ack, addr); // Сбор нового пакета
        socket
            .send(packet)
            .await
            .map_err(|e| SendingAckError(e.to_string()))? // Отправка пакета сокету
    }
    Err(SendingAckError(String::from(
        "Not enough free space to store",
    ))) // Если места нет - возвращаем соответствующую ошибку
}

async fn send_content_filled(
    hash: String,
    addr: SocketAddr,
    socket: &Socket,
    storage: &mut UdpServerStorage,
) -> Result<(), SendingContentFilled> {
    if let Ok(d) = storage.get(&hash).await {
        // Если в хранилище есть такой хэш
        let message = Message::ContentFilled(hash, d)
            .into_bytes()
            .map_err(|e| SendingContentFilled(e.to_string()))?; // Создание сообщения с данными и перевод его в поток байт
        let packet = Packet::new(message, addr); // Сбор нового пакета
        socket
            .send(packet)
            .await
            .map_err(|e| SendingContentFilled(e.to_string()))?;
    }
    Err(SendingContentFilled(String::from("No hash was found")))
}

mod errors {
    // Модуль с составными типами ошибок
    use std::error::Error; // Зависимость стандартной библиотеки для работы с трейтом ошибок
    use std::fmt; // Зависимость стандартной библиотеки для работы с форматированием

    #[derive(Debug, Clone)]
    pub struct NoFreeSpaceError; // Тип ошибки отсутствия свободного места на диске

    impl fmt::Display for NoFreeSpaceError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "No free space left for keeping data")
        }
    }

    impl Error for NoFreeSpaceError {}

    #[derive(Debug, Clone)]
    pub struct NoHashError(pub String); // Тип ошибки отсутствия представленного хэша в хранилище

    impl fmt::Display for NoHashError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "No hash {} was found", self.0)
        }
    }

    impl Error for NoHashError {}

    #[derive(Debug, Clone)]
    pub struct InvalidMessageError; // Тип ошибки нераспознанного сообщения

    impl fmt::Display for InvalidMessageError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Got invalid message")
        }
    }

    impl Error for InvalidMessageError {}

    #[derive(Debug, Clone)]
    pub struct ServerInitError(pub String); // Тип ошибки инициализации сервера

    impl fmt::Display for ServerInitError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Error starting server: {}", self.0)
        }
    }

    impl Error for ServerInitError {}

    #[derive(Debug, Clone)]
    pub struct SendingAckError(pub String);

    impl fmt::Display for SendingAckError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Error sending SENDING_ACK: {}", self.0)
        }
    }

    impl Error for SendingAckError {}

    #[derive(Debug, Clone)]
    pub struct SendingContentFilled(pub String);

    impl fmt::Display for SendingContentFilled {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Error sending CONTENT_FILLED: {}", self.0)
        }
    }

    impl Error for SendingContentFilled {}
}
