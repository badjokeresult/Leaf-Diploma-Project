use std::cell::RefCell;
use std::net::{SocketAddr, UdpSocket};

use local_ip_address::local_ip;

use leaf_common::message::{builder, consts::*, Message};
use leaf_common::{Codec, DeflateCodec, MessageType};

use crate::storage::{BroadcastServerStorage, ServerStorage};

use consts::*;
use errors::*;

pub mod consts {
    pub const DEFAULT_SERVER_PORT: u16 = 62092;
    pub const MAX_DATAGRAM_SIZE: usize = 65536;
}

pub trait ServerPeer {
    fn listen(&self);
}

pub struct BroadcastServerPeer {
    socket: UdpSocket,
    storage: RefCell<BroadcastServerStorage>,
    codec: DeflateCodec,
}

impl BroadcastServerPeer {
    pub fn new() -> Result<BroadcastServerPeer, ServerPeerInitializationError> {
        let addr = SocketAddr::new(match local_ip() {
            Ok(i) => i,
            Err(e) => return Err(ServerPeerInitializationError(e.to_string())),
        }, DEFAULT_SERVER_PORT);
        let socket = match UdpSocket::bind(addr) {
            Ok(s) => s,
            Err(e) => return Err(ServerPeerInitializationError(e.to_string())),
        };

        match socket.set_broadcast(true) {
            Ok(_) => {},
            Err(e) => return Err(ServerPeerInitializationError(e.to_string())),
        }

        let storage = RefCell::new(match BroadcastServerStorage::new() {
            Ok(s) => s,
            Err(e) => return Err(ServerPeerInitializationError(e.to_string())),
        });
        let codec = DeflateCodec::new();

        Ok(BroadcastServerPeer {
            socket,
            storage,
            codec,
        })
    }
}

impl ServerPeer for BroadcastServerPeer {
    fn listen(&self) {
        let mut buf = [0u8; MAX_DATAGRAM_SIZE];
        loop {
            let (_, addr) = match self.socket.recv_from(&mut buf) {
                Ok((s, a)) => (s, a),
                Err(_) => {
                    eprintln!("Error during receiving a datagram");
                    continue;
                },
            };
            println!("MESSAGE WAS RECEIVED FROM {}", addr);
            match builder::get_decode_message(&self.codec, &buf) {
                Ok(m) => match m.get_type() {
                    MessageType::SendingReq => match self.handle_sending_req(&m, addr) {
                        Some(_) => {
                            eprintln!("Error handling SENDING_REQ message");
                            continue;
                        },
                        None => {},
                    },
                    MessageType::RetrievingReq => match self.handle_retrieving_req(&m, addr) {
                        Some(_) => {
                            eprintln!("Error handling RETRIEVING_REQ message");
                            continue;
                        },
                        None => {},
                    },
                    MessageType::ContentFilled => match self.handle_content_filled(&m) {
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
                Err(e) => eprintln!("{}", e.to_string()),
            };
            println!("MESSAGE WAS HANDLED!");
        }
    }
}

impl BroadcastServerPeer {
    fn handle_sending_req(&self, message: &Message, addr: SocketAddr) -> Option<MessageHandlingError> {
        eprintln!("SENDING_REQ RECEIVED!");
        let new_message = match builder::build_encoded_message(&self.codec, SENDING_ACK_MSG_TYPE, &message.get_hash(), Some(message.get_data().unwrap())) {
            Ok(m) => m,
            Err(e) => return Some(MessageHandlingError(message.get_type(), e.to_string()))
        };
        match self.socket.send_to(&new_message, addr) {
            Ok(_) => None,
            Err(e) => Some(MessageHandlingError(MessageType::SendingAck, e.to_string())),
        }
    }

    fn handle_retrieving_req(&self, message: &Message, addr: SocketAddr) -> Option<MessageHandlingError> {
        eprintln!("RERTRIEVING_REQ RECEIVED!");
        let content = match self.storage.borrow_mut().get(&message.get_hash()) {
            Ok(c) => c,
            Err(e) => return Some(MessageHandlingError(message.get_type(), e.to_string())),
        };
        let msg = match builder::build_encoded_message(&self.codec, RETRIEVING_ACK_MSG_TYPE, &message.get_hash(), Some(content)) {
            Ok(m) => m,
            Err(e) => return Some(MessageHandlingError(MessageType::RetrievingAck, e.to_string())),
        };
        match self.socket.send_to(&msg, addr) {
            Ok(_) => None,
            Err(e) => Some(MessageHandlingError(MessageType::RetrievingAck, e.to_string())),
        }
    }

    fn handle_content_filled(&self, message: &Message) -> Option<MessageHandlingError> {
        eprintln!("CONTENT_FILLED RECEIVED!");
        self.storage.borrow_mut().add(&message.get_hash(), &message.get_data().unwrap());
        None
    }
}

mod errors {
    use std::fmt;
    use std::fmt::Formatter;
    use leaf_common::MessageType;

    #[derive(Debug, Clone)]
    pub struct ServerPeerInitializationError(pub String);

    impl fmt::Display for ServerPeerInitializationError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error initializing server peer: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct MessageHandlingError(pub MessageType, pub String);

    impl fmt::Display for MessageHandlingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error handling {:?} message: {}", self.0, self.1)
        }
    }
}