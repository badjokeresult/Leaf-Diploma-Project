use std::cell::RefCell;
use std::net::SocketAddr;

use local_ip_address::local_ip;
use tokio::net::UdpSocket;

use leaf_common::message::{message_builder, consts::*, Message};
use leaf_common::{Codec, DeflateCodec};

use crate::storage::{BroadcastServerStorage, ServerStorage};

use consts::*;
use errors::*;

pub mod consts {
    pub const DEFAULT_SERVER_PORT: u16 = 62092;
    pub const MAX_DATAGRAM_SIZE: usize = 508;
}

type Result<T> = std::result::Result<T, Box<dyn ServerPeerError>>;

pub trait ServerPeer {
    async fn listen<'a>(&'a self);
}

pub struct BroadcastServerPeer {
    socket: UdpSocket,
    storage: RefCell<BroadcastServerStorage>,
    codec: Box<dyn Codec>,
}

impl BroadcastServerPeer {
    pub async fn new() -> Result<BroadcastServerPeer> {
        let addr = SocketAddr::new(match local_ip() {
            Ok(i) => i,
            Err(e) => return Err(Box::new(LocalIpResolvingError(e.to_string()))),
        }, DEFAULT_SERVER_PORT);
        let socket = match UdpSocket::bind(addr).await {
            Ok(s) => s,
            Err(e) => return Err(Box::new(SocketBindingError(e.to_string()))),
        };

        let storage = RefCell::new(match BroadcastServerStorage::new().await {
            Ok(s) => s,
            Err(_) => return Err(Box::new(StorageInitError)),
        });
        let codec = Box::new(DeflateCodec::new());

        Ok(BroadcastServerPeer {
            socket,
            storage,
            codec,
        })
    }
}

impl ServerPeer for BroadcastServerPeer {
    async fn listen<'a>(&'a self) {
        let mut buf = [0u8; MAX_DATAGRAM_SIZE];
        loop {
            let (_, addr) = match self.socket.recv_from(&mut buf).await {
                Ok((s, a)) => (s, a),
                Err(_) => {
                    eprintln!("Error during receiving a datagram");
                    continue;
                },
            };
            eprintln!("RECEIVED UDP MESSAGE!!!!");
            match message_builder::get_decode_message(&self.codec, &buf) {
                Ok(m) => match m.get_type() {
                    SENDING_REQ_MSG_TYPE => match self.handle_sending_req(&m, addr).await {
                        Some(_) => {
                            eprintln!("Error handling SENDING_REQ message");
                            continue;
                        },
                        None => {},
                    },
                    RETRIEVING_REQ_MSG_TYPE => match self.handle_retrieving_req(&m, addr).await {
                        Some(_) => {
                            eprintln!("Error handling RETRIEVING_REQ message");
                            continue;
                        },
                        None => {},
                    },
                    CONTENT_FILLED_MSG_TYPE => match self.handle_content_filled(&m).await {
                        Some(_) => {
                            eprintln!("Error handling CONTENT_FILLED message");
                            continue;
                        },
                        None => {},
                    },
                    _ => {
                        eprintln!("Invalid message type was found: {}", m);
                        continue;
                    },
                },
                Err(_) => {
                    eprintln!("Invalid message content was found");
                    continue;
                },
            };
        }
    }
}

impl BroadcastServerPeer {
    async fn handle_sending_req(&self, message: &Message, addr: SocketAddr) -> Option<Box<dyn ServerPeerError>> {
        let new_message = match message_builder::build_encoded_message(&self.codec, SENDING_ACK_MSG_TYPE, &message.get_hash(), Some(message.get_data().unwrap())) {
            Ok(m) => m,
            Err(_) => return Some(Box::new(MessageBuildingError))
        };
        match self.socket.send_to(&new_message, addr).await {
            Ok(_) => None,
            Err(e) => Some(Box::new(SendingDatagramError(e.to_string()))),
        }
    }

    async fn handle_retrieving_req(&self, message: &Message, addr: SocketAddr) -> Option<Box<dyn ServerPeerError>> {
        let content = match self.storage.borrow_mut().get(&message.get_hash()).await {
            Ok(c) => c,
            Err(_) => return Some(Box::new(ReceivingFromStorageError)),
        };
        let msg = match message_builder::build_encoded_message(&self.codec, RETRIEVING_ACK_MSG_TYPE, &message.get_hash(), Some(content)) {
            Ok(m) => m,
            Err(_) => return Some(Box::new(MessageBuildingError)),
        };
        match self.socket.send_to(&msg, addr).await {
            Ok(_) => None,
            Err(e) => Some(Box::new(SendingDatagramError(e.to_string()))),
        }
    }

    async fn handle_content_filled(&self, message: &Message) -> Option<Box<dyn ServerPeerError>> {
        match self.storage.borrow_mut().add(&message.get_hash(), &message.get_data().unwrap()).await {
            Ok(_) => None,
            Err(_) => Some(Box::new(SendingIntoStorage)),
        }
    }
}

mod errors {
    use std::fmt;
    use std::fmt::{Debug, Formatter};

    pub trait ServerPeerError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result;
    }

    #[derive(Debug, Clone)]
    pub struct LocalIpResolvingError(pub String);

    impl ServerPeerError for LocalIpResolvingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error resolving local IP-address: {}", self.0)
        }
    }

    impl fmt::Display for LocalIpResolvingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            ServerPeerError::fmt(self, f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct SocketBindingError(pub String);

    impl ServerPeerError for SocketBindingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error binding UDP socket: {}", self.0)
        }
    }

    impl fmt::Display for SocketBindingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            ServerPeerError::fmt(self, f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct StorageInitError;

    impl ServerPeerError for StorageInitError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error during initialization of a storage")
        }
    }

    impl fmt::Display for StorageInitError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            ServerPeerError::fmt(self, f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct ReceivingDatagramError(pub String);

    impl ServerPeerError for ReceivingDatagramError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error receiving datagram: {}", self.0)
        }
    }

    impl fmt::Display for ReceivingDatagramError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            ServerPeerError::fmt(self, f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct SendingDatagramError(pub String);

    impl ServerPeerError for SendingDatagramError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error sending datagram: {}", self.0)
        }
    }

    impl fmt::Display for SendingDatagramError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            ServerPeerError::fmt(self, f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct MessageBuildingError;

    impl ServerPeerError for MessageBuildingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error building message")
        }
    }

    impl fmt::Display for MessageBuildingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            ServerPeerError::fmt(self, f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct MessageDeconstructionError;

    impl ServerPeerError for MessageDeconstructionError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error deconstructing message")
        }
    }

    impl fmt::Display for MessageDeconstructionError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            ServerPeerError::fmt(self, f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct ReceivingFromStorageError;

    impl ServerPeerError for ReceivingFromStorageError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error receiving data from storage")
        }
    }

    impl fmt::Display for ReceivingFromStorageError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            ServerPeerError::fmt(self, f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct SendingIntoStorage;

    impl ServerPeerError for SendingIntoStorage {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error sending into storage")
        }
    }

    impl fmt::Display for SendingIntoStorage {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            ServerPeerError::fmt(self, f)
        }
    }
}