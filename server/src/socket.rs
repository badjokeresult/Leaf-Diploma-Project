use std::net::SocketAddr; // Зависимость стандартной библиотеки для работы с сетевыми адресами
use std::sync::Arc; // Зависимость стандартной библиотеки для работы с объектами в многопоточном режиме
use std::time::Duration; // Зависимость стандартной библиотеки для обеспечения простоя потока

use tokio::net::UdpSocket; // Внешняя зависимость для работы с асинхронным UDP-сокетом
use tokio::sync::broadcast; // Внешняя зависимость для работы с асинхронными широковещательными каналами
use tokio::time; // Внешняя зависимость для работы со временем в асинхронном исполнении

use consts::*; // Внутренняя зависимость для использования констант

mod consts {
    // Модуль с константами
    pub const LOCAL_ADDR: &str = "0.0.0.0:62092"; // Строка адреса для открытия сокета
    pub const CHAN_QUEUE_SIZE: usize = 100; // Размер очереди для канала
    pub const UDP_SOCKET_BUF_SIZE: usize = 1024; // Размер буфера для приема данных из сети
    pub const TIMEOUT_MILLIS: u64 = 100; // Значение таймаута для переключения потока
}

#[derive(Clone, Debug)]
pub struct Packet {
    // Структура пакета
    pub data: Vec<u8>,    // Данные пакета (закодированное сообщение)
    pub addr: SocketAddr, // Адрес источника
}

impl Packet {
    pub fn new(data: Vec<u8>, addr: SocketAddr) -> Packet {
        Packet { data, addr }
    } // Метод создания нового пакета

    pub fn deconstruct(self) -> (Vec<u8>, SocketAddr) {
        (self.data, self.addr)
    } // Удаление пакета и получение его полей
}

#[derive(Clone)]
pub struct Socket {
    // Структура сокета
    socket: Arc<UdpSocket>, // Сокет с возможностью работы в нескольких потоках
    sender: broadcast::Sender<Packet>, // Отправитель широковещательного канала
}

impl Socket {
    pub async fn new() -> (Socket, broadcast::Sender<Packet>) {
        // Метод создания нового сокета, возвращает сам сокет, а также отправителя канала
        let socket = Arc::new(UdpSocket::bind(LOCAL_ADDR).await.unwrap()); // Создаем UDP-сокет
        socket.set_broadcast(true).unwrap(); // Устанавливаем сокет как способный работать с широковещательными запросами

        let (tx, _) = broadcast::channel(CHAN_QUEUE_SIZE); // Создаем канал

        (
            Socket {
                socket,
                sender: tx.clone(),
            },
            tx,
        ) // Возращаем сокет и отправителя
    }

    pub async fn send(&self, packet: Packet) {
        // Метод отправки данных в сеть
        let (data, addr) = packet.deconstruct(); // Разбор пакета на части
        self.socket.send_to(data.as_slice(), addr).await.unwrap(); // Отправка данных пакета по указанному адресу
    }

    pub async fn recv(&self) {
        // Метод получения данных из сети
        let mut buf = [0u8; UDP_SOCKET_BUF_SIZE]; // Создаем буфер
        loop {
            time::sleep(Duration::from_millis(TIMEOUT_MILLIS)).await; // Задержка для переключения потока
            if let Ok((s, a)) = self.socket.recv_from(&mut buf).await {
                // Если в сокете есть данные
                let packet = Packet::new(buf[..s].to_vec(), a); // Собираем из данных пакет
                self.sender.send(packet).unwrap(); // Отправляем пакет по каналу получателям для дальнейшей обработки
            }
        }
    }
}
