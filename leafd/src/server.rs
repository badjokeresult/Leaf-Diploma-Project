use std::path::PathBuf; // Зависимость стандартной библиотеки для работы с файловыми путями
use std::sync::atomic::{AtomicBool, Ordering}; // Зависимость стандартной библиотеки для синхронизированной работы флага
use std::sync::Arc; // Зависимость стандартной библиотеки для разделяемого доступа к объектам

use tokio::fs; // Для асинхронной работы с файловой системой
use tokio::sync::mpsc::Receiver; // Асинхронный получатель канала
use tokio::task::JoinHandle; // Идентификатор асинхронного потока

use leafcommon::Message; // Тип сообщения

use crate::socket::{Packet, Socket}; // Внутренние определения сокета и пакета
use crate::stor::{ServerStorage, UdpServerStorage}; // Внутренние определения хранилища

use errors::*;

#[derive(Clone)]
pub struct Server {
    // Сервер может клонироваться для корректного перемещения в поток
    socket: Socket,
    storage: UdpServerStorage,
}

impl Server {
    pub async fn new(path: PathBuf) -> Result<Server, Box<dyn std::error::Error>> {
        // Метод конфигурации сервера
        let socket = Socket::new().await?; // Создаем объект сокета

        fs::create_dir_all(&path).await?; // Создаем все директории на пути, если они еще не созданы
        let storage = UdpServerStorage::new(path);

        Ok(Server { socket, storage })
    }

    pub fn run(
        &self,
        rx: Receiver<Packet>,
        shutdown_rx: Arc<AtomicBool>,
    ) -> Result<JoinHandle<()>, Box<dyn std::error::Error>> {
        // Метод запуска сервера
        let socket = self.socket.clone();
        let storage = self.storage.clone(); // Клонируем поля для корректного перемещения в поток
        let handle = tokio::spawn(async move {
            Self::packet_handler(rx, &storage, &socket, shutdown_rx).await;
        });
        Ok(handle) // Возвращаем идентификатор обработчика пакетов
    }

    pub fn get_socket_clone(&self) -> Socket {
        self.socket.clone() // Возвращаем глубокую копию сокета
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
                break;
            }

            // Ожидание новых данных из канала в сокете
            if let Err(e) = Self::process_packet(p, storage, socket).await {
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
