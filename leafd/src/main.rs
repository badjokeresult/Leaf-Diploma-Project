mod socket; // Объявление внутреннего модуля сокета
mod stor; // Объявление внутреннего модуля хранилища

use std::path::PathBuf;

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

    let path = PathBuf::from(APPS_DIR_ABS_PATH)
        .join(APP_DIR)
        .join(CHUNKS_DIR);
    let storage = UdpServerStorage::new(path);
    let socket_clone = socket.clone();
    let storage_clone = storage.clone(); // Клонируем поля для корректного перемещения в поток
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    tokio::spawn(async move {
        packet_handler(rx, &storage_clone, &socket_clone).await;
    });

    loop {
        socket.recv(&tx).await;
    }
}

async fn packet_handler(mut rx: Receiver<Packet>, storage: &UdpServerStorage, socket: &Socket) {
    // Функция-обработчик сообщений
    while let Some(p) = rx.recv().await {
        // Ожидание новых данных из канала в сокете
        if let Err(e) = process_packet(p, storage, socket).await {
            // Обрабатываем пакет и проверяем наличие ошибок
            eprintln!("{}", e.to_string()); // При наличии ошибок пишем их в stderr, но не прерываем поток
        };
    }
}

async fn process_packet(
    // Функция обработки отдельного пакета
    packet: Packet,
    storage: &UdpServerStorage,
    socket: &Socket,
) -> Result<(), Box<dyn std::error::Error>> {
    let (data, addr) = packet.deconstruct(); // Разбираем пакет на данные и адрес источника
    let message = Message::from_bytes(data)?; // Восстанавливаем сообщение из потока байт
    match message.clone() {
        Message::SendingReq(h) => {
            // Если сообщение является запросом на хранение
            if storage.can_save().await? {
                // Проверка доступного места на диске
                let ack = Message::SendingAck(h).into_bytes()?; // Создание сообщения подтверждения хранения и перевод его в поток байт
                let packet = Packet::new(ack, addr); // Сбор нового пакета
                socket.send(packet).await?; // Отправка пакета сокету
                return Ok(()); // Возврат
            }
            Err(Box::new(NoFreeSpaceError)) // Если места нет - возвращаем соответствующую ошибку
        }
        Message::RetrievingReq(h) => {
            // Если сообщение является запросом на получение
            if let Ok(d) = storage.get(&h).await {
                // Если в хранилище есть такой хэш
                let message = Message::ContentFilled(h.clone(), d).into_bytes()?; // Создание сообщения с данными и перевод его в поток байт
                let packet = Packet::new(message, addr); // Сбор нового пакета
                socket.send(packet).await?; // Отправка пакета в сокет
                return Ok(()); // Возврат
            }
            Err(Box::new(NoHashError(h))) // Если в хранилище нет такого хэша - возвращаем соответствующую ошибку
        }
        Message::ContentFilled(h, d) => {
            // Если сообщение содержит данные
            storage.save(&h, &d).await?; // Сохраняем данные на диске
            Ok(()) // Возврат
        }
        _ => Err(Box::new(InvalidMessageError)), // Если пришли любые другие данные - возвращаем ошибку
    }
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
}
