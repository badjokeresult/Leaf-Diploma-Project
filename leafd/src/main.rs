mod socket; // Объявление внутреннего модуля сокета
mod stor; // Объявление внутреннего модуля хранилища

use consts::*;
use errors::*;
use leafcommon::Message;
use socket::{Packet, Socket};
use std::{net::SocketAddr, path::PathBuf, time::Duration};
use stor::{ServerStorage, UdpServerStorage};
use tokio::sync::mpsc::Receiver;

// Добавляем зависимости для служб
#[cfg(target_os = "linux")]
use systemd::daemon; // Для уведомлений systemd
#[cfg(target_os = "windows")]
use windows_service::{
    service::{ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceType},
    service_dispatcher,
    service_manager::{ServiceManager, ServiceManagerAccess},
};

// Константы для Linux и Windows
mod consts {
    #[cfg(target_os = "linux")]
    pub const APPS_DIR_ABS_PATH: &str = "/var/local";
    #[cfg(target_os = "linux")]
    pub const APP_DIR: &str = "leaf";
    #[cfg(target_os = "linux")]
    pub const CHUNKS_DIR: &str = "chunks";
    #[cfg(target_os = "linux")]
    pub const STATE_FILE: &str = "last_state.bin";

    #[cfg(target_os = "windows")]
    pub const APPS_DIR_ABS_PATH: &str = "C:\\Program Files";
    #[cfg(target_os = "windows")]
    pub const APP_DIR: &str = "Leaf";
    #[cfg(target_os = "windows")]
    pub const CHUNKS_DIR: &str = "Chunks";
    #[cfg(target_os = "windows")]
    pub const STATE_FILE: &str = "last_state.bin";
}

// Определяем основную логику сервера
async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    let socket = Socket::new().await?;

    let base_path = PathBuf::from(APPS_DIR_ABS_PATH).join(APP_DIR);
    let stor_path = base_path.join(CHUNKS_DIR);
    let state_path = base_path.join(STATE_FILE);
    let storage = UdpServerStorage::new(stor_path, state_path).await?;
    let socket_clone = socket.clone();
    let mut storage_clone = storage.clone();
    let (tx, rx) = tokio::sync::mpsc::channel(100);

    // Уведомляем systemd о готовности (только для Linux)
    #[cfg(target_os = "linux")]
    {
        daemon::notify(false, vec![&("READY", "1")].into_iter())
            .map_err(|e| ServerInitError(e.to_string()))?;
        println!("Notified systemd: READY=1");
    }

    tokio::spawn(async move {
        packet_handler(rx, &mut storage_clone, &socket_clone).await;
    });

    loop {
        socket.recv(&tx).await;
    }
}

// Обработчик пакетов (без изменений)
async fn packet_handler(mut rx: Receiver<Packet>, storage: &mut UdpServerStorage, socket: &Socket) {
    while let Some(p) = rx.recv().await {
        process_packet(p, storage, &socket).await;
    }
}

// Обработка пакета (без изменений)
async fn process_packet(packet: Packet, storage: &mut UdpServerStorage, socket: &Socket) {
    let (data, addr) = packet.deconstruct();
    let message = Message::from_bytes(data).unwrap();
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
            if let Err(e) = storage.save(&h, &d).await {
                eprintln!("{}", e.to_string());
            }
        }
        _ => eprintln!(
            "{:?}",
            Err::<(), Box<InvalidMessageError>>(Box::new(InvalidMessageError))
        ),
    }
}

// Отправка подтверждения (без изменений)
async fn send_sending_ack(
    hash: String,
    addr: SocketAddr,
    socket: &Socket,
    storage: &UdpServerStorage,
) -> Result<(), SendingAckError> {
    if storage.can_save() {
        let ack = Message::SendingAck(hash)
            .into_bytes()
            .map_err(|e| SendingAckError(e.to_string()))?;
        let packet = Packet::new(ack, addr);
        socket
            .send(packet)
            .await
            .map_err(|e| SendingAckError(e.to_string()))?;
        Ok(())
    } else {
        Err(SendingAckError(String::from(
            "Not enough free space to store",
        )))
    }
}

// Отправка данных (без изменений)
async fn send_content_filled(
    hash: String,
    addr: SocketAddr,
    socket: &Socket,
    storage: &mut UdpServerStorage,
) -> Result<(), SendingContentFilled> {
    if let Ok(d) = storage.get(&hash).await {
        let message = Message::ContentFilled(hash, d)
            .into_bytes()
            .map_err(|e| SendingContentFilled(e.to_string()))?;
        let packet = Packet::new(message, addr);
        socket
            .send(packet)
            .await
            .map_err(|e| SendingContentFilled(e.to_string()))?;
        Ok(())
    } else {
        Err(SendingContentFilled(String::from("No hash was found")))
    }
}

// Реализация службы для Windows
#[cfg(target_os = "windows")]
mod windows_service_impl {
    use super::*;
    use windows_service::service::ServiceControl;
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};

    const SERVICE_NAME: &str = "LeafServer";
    const SERVICE_DISPLAY_NAME: &str = "Leaf UDP Server";
    const SERVICE_DESCRIPTION: &str = "UDP-based server for Leaf application";

    pub fn run() -> Result<(), windows_service::Error> {
        service_dispatcher::start(SERVICE_NAME, service_main)?;
        Ok(())
    }

    fn service_main(_arguments: Vec<String>) {
        if let Err(e) = run_service() {
            eprintln!("Service failed: {}", e);
        }
    }

    fn run_service() -> Result<(), Box<dyn std::error::Error>> {
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(async {
            run_server().await?;
            Ok(())
        })
    }

    pub fn install_service() -> Result<(), windows_service::Error> {
        let manager_access = ServiceManagerAccess::CREATE_SERVICE;
        let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;

        let service_info = ServiceInfo {
            name: SERVICE_NAME.into(),
            display_name: SERVICE_DISPLAY_NAME.into(),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path: std::env::current_exe()?,
            launch_arguments: vec![],
            dependencies: vec![],
            account_name: None, // Локальная система
            account_password: None,
        };

        let service =
            service_manager.create_service(&service_info, ServiceAccess::CHANGE_CONFIG)?;
        service.set_description(SERVICE_DESCRIPTION)?;
        println!("Service installed successfully");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Проверяем, запущен ли процесс как служба
    #[cfg(target_os = "windows")]
    {
        if std::env::args().any(|arg| arg == "--service") {
            return windows_service_impl::run().map_err(Into::into);
        } else if std::env::args().any(|arg| arg == "--install") {
            return windows_service_impl::install_service().map_err(Into::into);
        }
    }

    // Обычный запуск (консольный режим или systemd)
    run_server().await?;
    Ok(())
}

// Модуль ошибок (без изменений)
mod errors {
    use std::error::Error;
    use std::fmt;

    #[derive(Debug, Clone)]
    pub struct NoFreeSpaceError;
    impl fmt::Display for NoFreeSpaceError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "No free space left for keeping data")
        }
    }
    impl Error for NoFreeSpaceError {}

    #[derive(Debug, Clone)]
    pub struct NoHashError(pub String);
    impl fmt::Display for NoHashError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "No hash {} was found", self.0)
        }
    }
    impl Error for NoHashError {}

    #[derive(Debug, Clone)]
    pub struct InvalidMessageError;
    impl fmt::Display for InvalidMessageError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Got invalid message")
        }
    }
    impl Error for InvalidMessageError {}

    #[derive(Debug, Clone)]
    pub struct ServerInitError(pub String);
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
