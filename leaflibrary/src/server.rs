use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::UdpSocket;

use rayon::prelude::*;

use net2::UdpBuilder;
use net2::unix::UnixUdpBuilderExt;

use atomic_refcell::AtomicRefCell;

use tokio::io::AsyncReadExt;

use consts::*;
use crate::message::Message;
use crate::server::errors::{HandlingMessageError, ServerInitError, ShutdownError};
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
    udp_socket: Arc<UdpSocket>,
    storage: AtomicRefCell<BroadcastUdpServerStorage>,
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

        let udp_socket = Arc::new(match UdpSocket::from_std(udp_socket) {
            Ok(s) => s,
            Err(e) => return Err(ServerInitError(e.to_string())),
        });

        let storage = AtomicRefCell::new(match BroadcastUdpServerStorage::new(
            chunks_folder,
        ).await {
            Ok(s) => s,
            Err(e) => return Err(ServerInitError(e.to_string())),
        });

        Ok(BroadcastUdpServer {
            udp_socket,
            storage,
        })
    }

    async fn handle_retrieving_req(&self, hash: &[u8], addr: SocketAddr) -> Result<(), HandlingMessageError> {
        let messages = match self.storage.borrow().retrieve(hash).await {
            Ok(c) => {
                let content = Message::new_with_data(hash, &c);
                content.par_iter().map(|x| x.clone().into()).collect::<Vec<Vec<_>>>()
            },
            Err(e) => return Err(HandlingMessageError(e.to_string())),
        };
        let ack: Vec<u8> = Message::RetrievingAck(hash.to_vec()).into();
        match self.udp_socket.send_to(&ack, addr).await {
            Ok(_) => {},
            Err(e) => return Err(HandlingMessageError(e.to_string())),
        };
        for message in &messages {
            match self.udp_socket.send_to(message, addr).await {
                Ok(_) => {},
                Err(e) => return Err(HandlingMessageError(e.to_string())),
            };
        }

        let ending: Vec<u8> = Message::Empty(hash.to_vec()).into();
        match self.udp_socket.send_to(&ending, addr).await {
            Ok(_) => {},
            Err(e) => return Err(HandlingMessageError(e.to_string())),
        };
        Ok(())
    }

    async fn handle_sending_req(&self, hash: &[u8], addr: SocketAddr) -> Result<(), HandlingMessageError> {
        let message: Vec<u8> = Message::SendingAck(hash.to_vec()).into();
        match self.udp_socket.send_to(&message, addr).await {
            Ok(_) => {},
            Err(e) => return Err(HandlingMessageError(e.to_string())),
        };
        Ok(())
    }

    async fn handle_content_filled(&self, hash: &[u8], data: &[u8]) -> Result<(), HandlingMessageError> {
        match self.storage.borrow_mut().add(hash, data).await {
            Ok(_) => {},
            Err(e) => return Err(HandlingMessageError(e.to_string())),
        };
        Ok(())
    }
}

impl UdpServer for BroadcastUdpServer {
    async fn listen(&self) -> Result<(), HandlingMessageError> {
        loop {
            let mut buf = [0u8; MAX_DATAGRAM_SIZE];
            let (sz, addr) = match self.udp_socket.recv_from(&mut buf).await {
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
                Message::ContentFilled(h, d) => {
                    match self.handle_content_filled(&h, &d).await {
                        Ok(_) => {},
                        Err(e) => eprintln!("{}", e.to_string()),
                    };
                }
                _ => eprintln!("Invalid message received"),
            };
        }
    }

    async fn shutdown(&self) -> Result<(), ShutdownError> {
        match self.storage.borrow_mut().shutdown().await {
            Ok(_) => Ok(()),
            Err(e) => Err(ShutdownError(e.to_string())),
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
}