use std::net::SocketAddr; // Зависимость стандартной библиотеки для работы с сетевыми адресами
use std::sync::Arc; // Зависимость стандартной библиотеки для работы с объектами в многопоточном режиме

use tokio::net::UdpSocket; // Внешняя зависимость для работы с асинхронным UDP-сокетом

use consts::*;
use tokio::sync::mpsc::Sender; // Внутренняя зависимость для использования констант

mod consts {
    // Модуль с константами
    pub const LOCAL_ADDR: &str = "0.0.0.0:62092"; // Строка адреса для открытия сокета
    pub const UDP_SOCKET_BUF_SIZE: usize = 65535; // Размер буфера для приема данных из сети
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
}

impl Socket {
    pub async fn new() -> Socket {
        // Метод создания нового сокета, возвращает сам сокет, а также отправителя канала
        let socket = Arc::new(UdpSocket::bind(LOCAL_ADDR).await.unwrap()); // Создаем UDP-сокет
        socket.set_broadcast(true).unwrap(); // Устанавливаем сокет как способный работать с широковещательными запросами

        Socket { socket } // Возращаем сокет и отправителя
    }

    pub async fn send(&self, packet: Packet) {
        // Метод отправки данных в сеть
        let (data, addr) = packet.deconstruct(); // Разбор пакета на части
        self.socket.send_to(data.as_slice(), addr).await.unwrap(); // Отправка данных пакета по указанному адресу
    }

    pub async fn recv(&self, tx: &Sender<Packet>) {
        // Метод получения данных из сети
        let mut buf = [0u8; UDP_SOCKET_BUF_SIZE]; // Создаем буфер
        while let Ok((s, a)) = self.socket.recv_from(&mut buf).await {
            // Если в сокете есть данные
            let packet = Packet::new(buf[..s].to_vec(), a); // Собираем из данных пакет
            tx.send(packet).await.unwrap(); // Отправляем пакет по каналу получателям для дальнейшей обработки
        }
    }
}
