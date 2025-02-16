use std::net::SocketAddr; // Зависимость стандартной библиотеки для работы с сетевыми адресами
use std::sync::Arc; // Зависимость стандартной библиотеки для работы с объектами в многопоточном режиме

use tokio::net::UdpSocket; // Внешняя зависимость для работы с асинхронным UDP-сокетом
use tokio::sync::mpsc::Sender; // Внешняя зависимость для работы с асинхронными каналами

use consts::*; // Зависимость внутреннего модуля для работы с константами
use errors::*; // Зависимость внутреннего модуля для работы с составными типами ошибок

mod consts {
    // Модуль с константами
    pub const LOCAL_ADDR: &str = "0.0.0.0:62092"; // Строка адреса для открытия сокета
    pub const UDP_SOCKET_BUF_SIZE: usize = 65527; // Размер буфера для приема данных из сети (максимальный размер поля полезной нагрузки датаграммы)
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
    } // Конструктор нового пакета

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
    pub async fn new() -> Result<Socket, SocketInitError> {
        // Конструктор нового сокета
        let socket = Arc::new(
            UdpSocket::bind(LOCAL_ADDR)
                .await
                .map_err(|e| SocketInitError(e.to_string()))?,
        ); // Создаем UDP-сокет
        socket
            .set_broadcast(true)
            .map_err(|e| SocketInitError(e.to_string()))?; // Устанавливаем сокет как способный работать с широковещательными запросами

        Ok(Socket { socket }) // Возращаем сокет
    }

    pub async fn send(&self, packet: Packet) -> Result<(), SendingPacketError> {
        // Метод отправки данных в сеть
        let (data, addr) = packet.deconstruct(); // Разбор пакета на части
        self.socket
            .send_to(data.as_slice(), addr)
            .await
            .map_err(|e| SendingPacketError(e.to_string()))?; // Отправка данных пакета по указанному адресу
        Ok(())
    }

    pub async fn recv(&self, tx: &Sender<Packet>) {
        // Метод получения данных из сети
        let mut buf = [0u8; UDP_SOCKET_BUF_SIZE]; // Создаем буфер
        while let Ok((s, a)) = self.socket.recv_from(&mut buf).await {
            // Если в сокете есть данные
            let packet = Packet::new(buf[..s].to_vec(), a); // Собираем из данных пакет
            if let Err(e) = tx.send(packet).await {
                eprintln!("{}", e.to_string());
            } // Отправляем пакет по каналу получателям для дальнейшей обработки
        }
    }
}

mod errors {
    // Внутренний модуль с составными типами ошибок
    // Зависимости внутренней библиотеки для создания составных типов ошибок
    use std::error::Error;
    use std::fmt;
    use std::fmt::Formatter;

    #[derive(Debug, Clone)]
    pub struct SocketInitError(pub String); // Тип ошибки инициализации сокета

    impl fmt::Display for SocketInitError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error during socket initialization: {}", self.0)
        }
    }

    impl Error for SocketInitError {}

    #[derive(Debug, Clone)]
    pub struct SendingPacketError(pub String); // Тип ошибки инициализации сокета

    impl fmt::Display for SendingPacketError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error sending packet: {}", self.0)
        }
    }

    impl Error for SendingPacketError {}
}
