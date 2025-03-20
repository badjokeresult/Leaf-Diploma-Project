use crate::socket::*;
use crate::stor::*;
use common::Message;
use errors::*;
use log::{error, info};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::mpsc::{channel, Receiver};

// Константы для путей
#[cfg(target_os = "windows")]
const APPS_DIR_ABS_PATH: &str = "C:\\Program Files";
#[cfg(target_os = "linux")]
const APPS_DIR_ABS_PATH: &str = "/var/local";

const APP_DIR: &str = "leaf";
const CHUNKS_DIR: &str = "chunks";

// Структура AsyncServer, которая управляет жизненным циклом сервера
#[derive(Clone)]
pub struct AsyncServer {
    shutdown_signal: Arc<AtomicBool>,
}

impl AsyncServer {
    pub fn new() -> Self {
        AsyncServer {
            shutdown_signal: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn get_shutdown_signal(&self) -> Arc<AtomicBool> {
        self.shutdown_signal.clone()
    }

    pub async fn run(&self) {
        // Запускаем основной код сервера внутри нашей структуры
        match run_server(self.shutdown_signal.clone()).await {
            Ok(_) => info!("Сервер завершил работу успешно"),
            Err(e) => error!("Ошибка сервера: {}", e),
        }
    }
}

// Основная асинхронная функция сервера, адаптированная для работы с сигналом завершения
async fn run_server(shutdown_signal: Arc<AtomicBool>) -> Result<(), Box<dyn std::error::Error>> {
    let socket = Socket::new().await?;
    let (tx, rx) = channel(100);

    let base_path = PathBuf::from(APPS_DIR_ABS_PATH);

    let path = base_path.join(APP_DIR).join(CHUNKS_DIR);
    fs::create_dir_all(&path).await?;

    let storage = UdpServerStorage::new(path);
    let socket_clone = socket.clone();

    // Запускаем обработчик пакетов в отдельной задаче
    let handler_handle = tokio::spawn(async move {
        packet_handler(rx, &storage, &socket_clone).await;
    });

    // Запускаем основной цикл приема данных с проверкой сигнала завершения

    loop {
        // Проверяем сигнал завершения
        if shutdown_signal.load(Ordering::SeqCst) {
            break;
        }
        // Принимаем данные (с таймаутом для проверки сигнала завершения)
        socket.recv(&tx).await;
    }

    handler_handle.await?;
    Ok(())
}

// Функция-обработчик сообщений (без изменений)
async fn packet_handler(mut rx: Receiver<Packet>, storage: &UdpServerStorage, socket: &Socket) {
    // Функция-обработчик сообщений
    while let Some(p) = rx.recv().await {
        // Ожидание новых данных из канала в сокете
        if let Err(e) = process_packet(p, storage, socket).await {
            // Обрабатываем пакет и проверяем наличие ошибок
            error!("{}", e.to_string()); // Используем error! из log вместо eprintln!
        };
    }
}

// Функция обработки отдельного пакета (без изменений)
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
