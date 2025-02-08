use std::collections::HashMap; // Зависимость стандартной библиотеки для работы с хэш-таблицами
use std::path::PathBuf; // Зависимость стандартной библиотеки для работы с файловыми путями
use std::time::Duration; // Зависимость стандартной библиотеки для с простоем потоков

use tokio::fs; // Внешняя зависимость для работы с дисковыми операциями ввода-вывода в асинхронном исполнении
use tokio::sync::broadcast; // Внешняя зависимость для работы с широковещательными асинхронными каналами
use tokio::time; // Внешняя зависимость для асинхронной работы с временем

use common::Message; // Зависимость внутренней библиотеки для работы с сообщениями

use socket::{Packet, Socket};
use stor::{ServerStorage, UdpServerStorage}; // Внутренняя зависимость для работы с хранилищем принятых данных // Внутренняя зависимость для работы с UDP-сокетом

mod socket; // Объявление модуля UDP-сокета
mod stor; // Объявление модуля хранилища

async fn process_packet(
    // Функция для обработки пришедшего сообщения
    packet: Packet,
    storage: &UdpServerStorage,
    socket: &Socket,
    buf: &mut HashMap<Vec<u8>, Vec<u8>>,
) {
    time::sleep(Duration::from_millis(100)).await; // Простой для переключения потока
    let addr = packet.addr; // Получение адреса источника
    let message = Message::from(packet.data); // Формирование сообщения из двоичных данных
    match message.clone() {
        Message::SendingReq(h) => {
            // Если пришел запрос на отправку
            if storage.can_save().await {
                // Если можно сохранить данные
                let ack: Vec<u8> = Message::SendingAck(h).into(); // Если текущий размер меньше, формируется подтверждение отправки
                let packet = Packet::new(ack, addr); // Формирование объекта датаграммы для отправки
                socket.send(packet).await; // Отправка датаграммы
            }
        }
        Message::RetrievingReq(h) => {
            // Если пришел запрос на получение
            if let Ok(d) = storage.get(&h).await {
                // Если данные с такой хэш-суммой представлены в хранилище
                let mut messages: Vec<Vec<u8>> = vec![]; // Буфер для потока сообщений
                messages.push(Message::RetrievingAck(h.clone()).into()); // Первое сообщение - подтверждение получения
                let content_messages = Message::generate_stream_for_chunk(&h, &d).unwrap(); // Формирование потока сообщений с данными
                for msg in content_messages {
                    messages.push(msg.into()); // Добавление сообщений в буфер с предварительным переводом их в двоичный формат
                }
                for message in messages {
                    let packet = Packet::new(message, addr); // Формирование объекта датаграммы для каждого сообщения
                    socket.send(packet).await; // Отправка датаграмм
                }
            }
        }
        Message::ContentFilled(h, d) => {
            // Если сообщение содержит данные
            if let Some(data) = buf.get(&h) {
                let mut dt = data.clone();
                dt.append(&mut d.to_vec()); // Если данные с такой хэш-суммой уже есть, то пишем в конец массива новые данные
            } else {
                buf.insert(h, d); // Иначе создаем новый массив
            }
        }
        Message::Empty(h) => {
            // Если сообщение-заглушка
            storage.save(&h, buf.get(&h).unwrap()).await.unwrap(); // Сохраняем данные в буфере на диск и очищаем буфер
            buf.remove(&h).unwrap(); // Очищаем буфер
        }
        _ => {} // Если тип сообщения неизвестен, ничего не делаем
    }
}

async fn packet_handler(
    mut rx: broadcast::Receiver<Packet>,
    storage: &UdpServerStorage,
    socket: &Socket,
) {
    // Метод обработчика пакетов
    let mut buf = HashMap::new();
    loop {
        if let Ok(p) = rx.recv().await {
            process_packet(p, storage, socket, &mut buf).await;
        }
    }
}

#[tokio::main]
async fn main() {
    let (socket, tx) = Socket::new().await; // Создание объекта сокета

    #[cfg(windows)]
    let base_path = PathBuf::from(env::var("APPDATA").unwrap()); // Базовый путь для Windows

    #[cfg(not(windows))]
    let base_path = PathBuf::from("/var/local"); // Базовый путь для Linux

    let path = base_path.join("leaf").join("chunks");
    fs::create_dir_all(&path).await.unwrap(); // Создание директории для хранения частей файлов

    let storage = UdpServerStorage::new(path); // Создание объекта хранилища

    for _ in 0..4 {
        let rx = tx.subscribe(); // Создание нового потребителя канала
        let socket_clone = socket.clone(); // Клонирование сокета для отправки его в поток
        let storage_clone = storage.clone(); // Клонирование хранилища для отправки его в поток
        tokio::spawn(async move {
            // Создание асинхронного потока
            packet_handler(rx, &storage_clone, &socket_clone).await; // Запуск обработчика пакетов в отдельном потоке
        });
    }

    tokio::spawn(async move {
        // Запуск асинхронного потока для приема пакетов от сокета
        socket.recv().await;
    });

    loop {
        tokio::time::sleep(Duration::from_millis(100)).await; // Запуск бесконечного цикла для продолжения работы основного потока с ожиданием для переключения на другие работающие потоки
    }
}
