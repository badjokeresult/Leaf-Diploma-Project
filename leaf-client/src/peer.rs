use std::hash::Hash;
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
        let hash = self.hasher.calc_hash_for_chunk(chunk).unwrap();
        let message = self.codec.encode_message(&Message::new(
            MessageType::SendingReq,
            &hash,
            None,
        ).as_json().unwrap()).unwrap();
        let broadcast_addr = SocketAddr::new(local_broadcast_ip().unwrap(), DEFAULT_SERVER_PORT);
        self.socket.send_to(&message, broadcast_addr).await.unwrap();

        let mut peer_addr = None;
        let mut sending_ack_buf = [0u8; MAX_DATAGRAM_SIZE];
        loop {
            let (_, addr) = self.socket.recv_from(&mut sending_ack_buf).await.unwrap();
            let message = Message::from_encoded_json(&sending_ack_buf).unwrap();
            if message.hash.eq(&hash) && message.msg_type == MessageType::SendingAck {
                peer_addr = Some(addr);
                break;
            }
        };

        let message = self.codec.encode_message(&Message::new(
            MessageType::ContentFilled,
            &hash,
            Some(chunk.to_vec()),
        ).as_json().unwrap()).unwrap();
        self.socket.send_to(&message, peer_addr.unwrap()).await.unwrap();

        Ok(hash)
    }

    async fn recv(&self, hash: &[u8]) -> Result<Vec<u8>> {
        let message = self.codec.encode_message(&Message::new(
            MessageType::RetrievingReq,
            hash,
            None,
        ).as_json().unwrap()).unwrap();
        let broadcast_addr = SocketAddr::new(local_broadcast_ip().unwrap(), DEFAULT_SERVER_PORT);
        self.socket.send_to(&message, broadcast_addr).await.unwrap();

        let mut retrieving_ack_buf = [0u8; MAX_DATAGRAM_SIZE];
        let mut data = None;
        loop {
            let _ = self.socket.recv_from(&mut retrieving_ack_buf).await.unwrap();
            let message = Message::from_encoded_json(&retrieving_ack_buf).unwrap();
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
}