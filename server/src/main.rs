mod socket; // Объявление внутреннего модуля сокета
mod stor; // Объявление внутреннего модуля хранилища

use std::os::unix::net::UnixDatagram;
use std::path::PathBuf; // Зависимость стандартной библиотеки для работы с файловыми путями
use std::sync::atomic::{AtomicBool, Ordering}; // Для флага завершения
use std::sync::Arc; // Для разделяемого владения

use tokio::fs; // Внешняя зависимость для асинхронной работы с файловой системой
use tokio::select; // Для одновременного ожидания нескольких событий
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc::{channel, Receiver}; // Внешняя зависимость для использования асинхронных каналов // Для обработки сигналов Unix

use common::Message; // Зависимость библиотеки проекта для работы с сообщениями

use consts::*; // Внутренний модуль с константами
use errors::*; // Внутренний модуль с ошибками
use socket::*; // Внутренний модуль сокета
use stor::*;

mod consts {
    // Модуль с константами
    #[cfg(windows)]
    pub const APPS_DIR_ABS_PATH: &str = "C:\\Program Files"; // Имя переменной среды, хранящей корень директории приложений (Windows)

    #[cfg(not(windows))]
    pub const APPS_DIR_ABS_PATH: &str = "/var/local"; // Абсолютный путь к корню директории хранилища

    pub const APP_DIR: &str = "leaf"; // Директория приложения
    pub const CHUNKS_DIR: &str = "chunks"; // Директория чанков
}

// Функция для отправки нотификации systemd
fn notify_systemd(state: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(socket_path) = std::env::var("NOTIFY_SOCKET") {
        if socket_path.is_empty() {
            return Ok(());
        }

        // Удаляем префикс '@' для абстрактных сокетов
        let socket_addr = if socket_path.starts_with('@') {
            format!("\0{}", &socket_path[1..])
        } else {
            socket_path
        };

        // Создаем датаграмный сокет
        let socket = UnixDatagram::unbound()?;

        // Отправляем сообщение в systemd
        socket.send_to(state.as_bytes(), socket_addr)?;

        println!("Sent systemd notification: {}", state);
        Ok(())
    } else {
        // Не запущено под systemd
        Ok(())
    }
}

// Уведомляем systemd о готовности
fn notify_systemd_ready() -> Result<(), Box<dyn std::error::Error>> {
    notify_systemd("READY=1")
}

// Уведомляем systemd о завершении
fn notify_systemd_stopping() -> Result<(), Box<dyn std::error::Error>> {
    notify_systemd("STOPPING=1")
}

// Отправляем сигнал watchdog
fn notify_systemd_watchdog() -> Result<(), Box<dyn std::error::Error>> {
    notify_systemd("WATCHDOG=1")
}

// Функция для настройки watchdog таймера
async fn setup_watchdog(shutdown: Arc<AtomicBool>) {
    let watchdog_usec = match std::env::var("WATCHDOG_USEC") {
        Ok(value) => match value.parse::<u64>() {
            Ok(usec) => usec,
            Err(_) => return,
        },
        Err(_) => return,
    };

    // Преобразуем микросекунды в миллисекунды и делим на 2 для безопасности
    let interval_ms = watchdog_usec / 1000 / 2;
    println!("Watchdog timer set to {} ms", interval_ms);

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(interval_ms));

        while !shutdown.load(Ordering::Relaxed) {
            interval.tick().await;
            if let Err(e) = notify_systemd_watchdog() {
                eprintln!("Failed to send watchdog notification: {}", e);
            }
        }
    });
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting UDP server...");

    // Флаг для изящного завершения работы
    let shutdown = Arc::new(AtomicBool::new(false));

    // Настройка обработчика сигналов
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sighup = signal(SignalKind::hangup())?;

    let socket = Socket::new().await?; // Создаем объект сокета
    let (tx, rx) = channel(100); // Создаем канал с размером очереди 100

    let base_path = PathBuf::from(APPS_DIR_ABS_PATH); // Получаем корень директории хранилища из пути

    let path = base_path.join(APP_DIR).join(CHUNKS_DIR); // Получаем путь до хранилища
    fs::create_dir_all(&path).await?; // Создаем все директории на пути, если они еще не созданы

    let storage = UdpServerStorage::new(path); // Создаем объект хранилища

    // Уведомляем systemd, что сервер готов к работе
    notify_systemd_ready()?;

    // Настройка watchdog
    setup_watchdog(Arc::clone(&shutdown)).await;

    // Клонирование сокета и флага для использования в асинхронном потоке
    let socket_clone = socket.clone();
    let shutdown_clone = Arc::clone(&shutdown);

    // Запуск обработчика пакетов
    let handler = tokio::spawn(async move {
        packet_handler(rx, &storage, &socket_clone, shutdown_clone).await;
    });

    // Основной цикл, который также проверяет сигналы для корректного завершения
    let receiver_task = tokio::spawn(async move {
        while !shutdown.load(Ordering::Relaxed) {
            select! {
                _ = socket.recv(&tx) => {
                    // Обработка полученных данных продолжается
                }
                _ = sigterm.recv() => {
                    println!("Received SIGTERM, shutting down gracefully...");
                    shutdown.store(true, Ordering::Relaxed);
                    break;
                }
                _ = sigint.recv() => {
                    println!("Received SIGINT, shutting down gracefully...");
                    shutdown.store(true, Ordering::Relaxed);
                    break;
                }
                _ = sighup.recv() => {
                    println!("Received SIGHUP, reloading configuration...");
                    // Здесь могла бы быть логика перезагрузки конфигурации
                }
            }
        }
    });

    // Ожидаем завершения любой из задач
    select! {
        _ = handler => {
            println!("Packet handler finished");
        }
        _ = receiver_task => {
            println!("Receiver task finished");
        }
    }

    // Уведомляем systemd о завершении работы
    if let Err(e) = notify_systemd_stopping() {
        eprintln!("Failed to notify systemd about stopping: {}", e);
    }

    println!("Server shut down gracefully");
    Ok(())
}

async fn packet_handler(
    mut rx: Receiver<Packet>,
    storage: &UdpServerStorage,
    socket: &Socket,
    shutdown: Arc<AtomicBool>,
) {
    // Функция-обработчик сообщений
    while let Some(p) = rx.recv().await {
        // Выходим из цикла, если получен сигнал завершения
        if shutdown.load(Ordering::Relaxed) {
            println!("Packet handler shutting down...");
            break;
        }

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
    println!("Received {} bytes", data.len());
    let message = Message::from_bytes(data)?; // Восстанавливаем сообщение из потока байт
    match message.clone() {
        Message::SendingReq(h) => {
            // Если сообщение является запросом на хранение
            if storage.can_save().await? {
                // Проверка доступного места на диске
                let ack = Message::SendingAck(h).into_bytes()?; // Создание сообщения подтверждения хранения и перевод его в поток байт
                println!("Sent {} bytes", ack.len());
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
                println!("Sent {} bytes", message.len());
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
