use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::sync::Mutex;

use net2::UdpBuilder;
use net2::unix::UnixUdpBuilderExt;

use consts::*;
use crate::message::Message;
use crate::server::errors::{HandlingMessageError, ServerInitError, ServingMessageError, ShutdownError};
use crate::storage::{BroadcastUdpServerStorage, UdpStorage};

mod consts {
    pub const LOCAL_ADDR: &str = "0.0.0.0:62092";
    pub const MAX_DATAGRAM_SIZE: usize = 65507;
}

pub trait UdpServer {
    fn listen(&self) -> impl std::future::Future<Output = Result<(), HandlingMessageError>> + Send;
    fn shutdown(&self) -> impl std::future::Future<Output = Result<(), ShutdownError>> + Send;
}

#[derive(Clone)]
pub struct BroadcastUdpServer {
    udp_socket: Arc<Mutex<UdpSocket>>,
    storage: BroadcastUdpServerStorage,
}

impl BroadcastUdpServer {
    pub async fn new(chunks_folder: &PathBuf) -> Result<BroadcastUdpServer, ServerInitError> {
        let udp_socket = match UdpBuilder::new_v4() {
            Ok(b) => match b.reuse_address(true) {
                Ok(b) => match b.reuse_port(true) {
                    Ok(b) => match b.bind(LOCAL_ADDR) {
                        Ok(s) => s,
                        Err(e) => return Err(ServerInitError(e.to_string())),
                    },
                    Err(e) => return Err(ServerInitError(e.to_string())),
                },
                Err(e) => return Err(ServerInitError(e.to_string())),
            },
            Err(e) => return Err(ServerInitError(e.to_string())),
        };

        match udp_socket.set_broadcast(true) {
            Ok(_) => {},
            Err(e) => return Err(ServerInitError(e.to_string())),
        };
        match udp_socket.set_write_timeout(Some(Duration::new(5, 0))) {
            Ok(_) => {},
            Err(e) => return Err(ServerInitError(e.to_string())),
        };
        match udp_socket.set_read_timeout(Some(Duration::new(5, 0))) {
            Ok(_) => {},
            Err(e) => return Err(ServerInitError(e.to_string())),
        };

        let udp_socket = Arc::new(Mutex::new(match UdpSocket::from_std(udp_socket) {
            Ok(s) => s,
            Err(e) => return Err(ServerInitError(e.to_string())),
        }));

        let storage = match BroadcastUdpServerStorage::new(
            chunks_folder,
        ).await {
            Ok(s) => s,
            Err(e) => return Err(ServerInitError(e.to_string())),
        };

        Ok(BroadcastUdpServer {
            udp_socket,
            storage,
        })
    }
}

impl UdpServer for BroadcastUdpServer {
    async fn listen(&self) -> Result<(), HandlingMessageError> {
        loop {
            let mut buf = [0u8; MAX_DATAGRAM_SIZE];
            let (sz, addr) = match self.udp_socket.lock().await.recv_from(&mut buf).await {
                Ok((s, a)) => (s, a),
                Err(e) => return Err(HandlingMessageError(e.to_string())),
            };
            let message = Message::from(buf[..sz].to_vec());
            match message.clone() {
                Message::RetrievingReq(h) => {
                    match self.handle_retrieving_req(&h, addr).await {
                        Ok(_) => {},
                        Err(e) => eprintln!("{}", e.to_string()),
                    };
                },
                Message::SendingReq(h) => {
                    match self.handle_sending_req(&h, addr).await {
                        Ok(_) => {},
                        Err(e) => eprintln!("{}", e.to_string()),
                    };
                },
                Message::ContentFilled(h, c) => {
                    match self.handle_content_filled(&h, &c).await {
                        Ok(_) => {},
                        Err(e) => eprintln!("{}", e.to_string()),
                    }
                },
                _ => eprintln!("Unknown message type"),
            }
        }
    }

    async fn shutdown(&self) -> Result<(), ShutdownError> {
        match self.storage.shutdown().await {
            Ok(_) => Ok(()),
            Err(e) => Err(ShutdownError(e.to_string())),
        }
    }
}

impl BroadcastUdpServer {
    async fn handle_retrieving_req(&self, hash: &[u8], addr: SocketAddr) -> Result<(), ServingMessageError> {
        let content = match self.storage.retrieve(hash).await {
            Ok(c) => c,
            Err(e) => return Err(ServingMessageError(e.to_string())),
        };

        let retrieving_ack: Vec<u8> = Message::RetrievingAck(hash.to_vec()).into();
        match self.udp_socket.lock().await.send_to(&retrieving_ack, addr).await {
            Ok(_) => {},
            Err(e) => return Err(ServingMessageError(e.to_string())),
        };

        let content_filled_messages = Message::new_with_data(hash, &content);
        for msg in content_filled_messages.into_iter() {
            let d: Vec<u8> = msg.into();
            match self.udp_socket.lock().await.send_to(&d, addr).await {
                Ok(_) => {},
                Err(e) => return Err(ServingMessageError(e.to_string())),
            };
            tokio::time::sleep(Duration::from_millis(100)).await;
        };

        let ending: Vec<u8> = Message::Empty(hash.to_vec()).into();
        match self.udp_socket.lock().await.send_to(&ending, addr).await {
            Ok(_) => Ok(()),
            Err(e) => Err(ServingMessageError(e.to_string())),
        }
    }

    async fn handle_sending_req(&self, hash: &[u8], addr: SocketAddr) -> Result<(), ServingMessageError> {
        let sending_ack: Vec<u8> = Message::SendingAck(hash.to_vec()).into();
        match self.udp_socket.lock().await.send_to(&sending_ack, addr).await {
            Ok(_) => Ok(()),
            Err(e) => Err(ServingMessageError(e.to_string())),
        }
    }

    async fn handle_content_filled(&self, hash: &[u8], content: &[u8]) -> Result<(), ServingMessageError> {
        match self.storage.add(hash, content).await {
            Ok(_) => Ok(()),
            Err(e) => Err(ServingMessageError(e.to_string())),
        }
    }
}

mod errors {
    use std::fmt;
    use std::fmt::Formatter;

    #[derive(Debug, Clone)]
    pub struct ServerInitError(pub String);

    impl fmt::Display for ServerInitError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error init server: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct HandlingMessageError(pub String);

    impl fmt::Display for HandlingMessageError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error handling message: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct ShutdownError(pub String);

    impl fmt::Display for ShutdownError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error during shutdown: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct ServingMessageError(pub String);

    impl fmt::Display for ServingMessageError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error serving message: {}", self.0)
        }
    }
}