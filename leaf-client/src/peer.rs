use std::net::SocketAddr;

use local_ip_address::{local_broadcast_ip, local_ip};
use tokio::net::UdpSocket;

use leaf_common::codec::{Codec, DeflateCodec};
use leaf_common::consts::{DEFAULT_SERVER_PORT, MAX_DATAGRAM_SIZE};
use leaf_common::hash::{Hasher, StreebogHasher};
use leaf_common::message::{Message, MessageType};

use errors::*;

type Result<T> = std::result::Result<T, Box<dyn ClientPeerError>>;

pub trait ClientPeer {
    async fn send(&self, chunk: &[u8]) -> Result<Vec<u8>>;
    async fn recv(&self, hash: &[u8]) -> Result<Vec<u8>>;
}

pub struct BroadcastClientPeer {
    socket: UdpSocket,
    hasher: Box<dyn Hasher>,
    codec: Box<dyn Codec>,
}

impl BroadcastClientPeer {
    pub async fn new() -> Result<BroadcastClientPeer> {
        let addr = local_ip().unwrap().to_string() + ":0";
        let socket = UdpSocket::bind(addr).await.unwrap();
        socket.set_broadcast(true).unwrap();

        let codec = Box::new(DeflateCodec::new());

        Ok(BroadcastClientPeer {
            socket,
            hasher: Box::new(StreebogHasher::new()),
            codec,
        })
    }
}

impl ClientPeer for BroadcastClientPeer {
    async fn send(&self, chunk: &[u8]) -> Result<Vec<u8>> {
        let hash = match self.hasher.calc_hash_for_chunk(chunk) {
            Ok(h) => h,
            Err(_) => return Err(Box::new(HashCalculationError)),
        };
        let message = match self.codec.encode_message(match &Message::new(
            MessageType::SendingReq,
            &hash,
            None,
        ).as_json() {
            Ok(s) => s,
            Err(_) => return Err(Box::new(BuildingMessageError)),
        }) {
            Ok(d) => d,
            Err(_) => return Err(Box::new(BuildingMessageError)),
        };
        let broadcast_addr = SocketAddr::new(local_broadcast_ip().unwrap(), DEFAULT_SERVER_PORT);
        self.socket.send_to(&message, broadcast_addr).await.unwrap();

        let mut peer_addr = None;
        let mut sending_ack_buf = [0u8; MAX_DATAGRAM_SIZE];
        loop {
            let (_, addr) = self.socket.recv_from(&mut sending_ack_buf).await.unwrap();
            let message = match Message::from_json(
                match &self.codec.decode_message(
                    sending_ack_buf.as_slice(),
                ) {
                    Ok(m) => m,
                    Err(_) => return Err(Box::new(CollectingMessageError)),
                }) {
                Ok(m) => m,
                Err(_) => return Err(Box::new(CollectingMessageError)),
            };
            if message.hash.eq(&hash) && message.msg_type == MessageType::SendingAck {
                peer_addr = Some(addr);
                break;
            }
        };

        let message = match self.codec.encode_message(match &Message::new(
            MessageType::ContentFilled,
            &hash,
            Some(chunk.to_vec()),
        ).as_json() {
            Ok(s) => s,
            Err(_) => return Err(Box::new(BuildingMessageError)),
        }) {
            Ok(d) => d,
            Err(_) => return Err(Box::new(BuildingMessageError)),
        };
        self.socket.send_to(&message, peer_addr.unwrap()).await.unwrap();

        Ok(hash)
    }

    async fn recv(&self, hash: &[u8]) -> Result<Vec<u8>> {
        let message = match self.codec.encode_message(match &Message::new(
            MessageType::RetrievingReq,
            &hash,
            None,
        ).as_json() {
            Ok(s) => s,
            Err(_) => return Err(Box::new(BuildingMessageError)),
        }) {
            Ok(d) => d,
            Err(_) => return Err(Box::new(BuildingMessageError)),
        };
        let broadcast_addr = SocketAddr::new(local_broadcast_ip().unwrap(), DEFAULT_SERVER_PORT);
        self.socket.send_to(&message, broadcast_addr).await.unwrap();

        let mut retrieving_ack_buf = [0u8; MAX_DATAGRAM_SIZE];
        let mut data = None;
        loop {
            let _ = self.socket.recv_from(&mut retrieving_ack_buf).await.unwrap();
            let message = match Message::from_json(
                match &self.codec.decode_message(
                    retrieving_ack_buf.as_slice(),
                ) {
                    Ok(m) => m,
                    Err(_) => return Err(Box::new(CollectingMessageError)),
                }) {
                Ok(m) => m,
                Err(_) => return Err(Box::new(CollectingMessageError)),
            };
            if message.msg_type == MessageType::ContentFilled && message.hash.eq(hash) {
                data = Some(message.data.unwrap());
                break;
            }
        }

        Ok(data.unwrap())
    }
}

mod errors {
    use std::fmt;
    use std::fmt::Formatter;

    pub trait ClientPeerError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result;
    }

    #[derive(Debug, Clone)]
    pub struct HashCalculationError;

    impl ClientPeerError for HashCalculationError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error calculation hash for chunk")
        }
    }

    impl fmt::Display for HashCalculationError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            ClientPeerError::fmt(self, f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct BuildingMessageError;

    impl ClientPeerError for BuildingMessageError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error building message")
        }
    }

    impl fmt::Display for BuildingMessageError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            ClientPeerError::fmt(self, f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct CollectingMessageError;

    impl ClientPeerError for CollectingMessageError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error building message")
        }
    }

    impl fmt::Display for CollectingMessageError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            ClientPeerError::fmt(self, f)
        }
    }
}
