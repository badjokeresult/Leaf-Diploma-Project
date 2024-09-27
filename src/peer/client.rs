use std::hash::Hash;
use std::net::SocketAddr;
use local_ip_address::{local_broadcast_ip, local_ip};
use tokio::net::UdpSocket;
use errors::*;
use crate::consts::{DEFAULT_SERVER_PORT, MAX_DATAGRAM_SIZE};
use crate::hash::{Hasher, StreebogHasher};
use crate::message::{Message, MessageType};

type Result<T> = std::result::Result<T, Box<dyn ClientPeerError>>;

pub trait ClientPeer {
    async fn send(&self, chunk: &[u8]) -> Result<Vec<u8>>;
    async fn recv(&self, hash: &[u8]) -> Result<Vec<u8>>;
}

pub struct BroadcastClientPeer {
    socket: UdpSocket,
    hasher: Box<dyn Hasher>,
}

impl BroadcastClientPeer {
    pub async fn new() -> Result<BroadcastClientPeer> {
        let addr = local_ip().unwrap().to_string() + ":0";
        let socket = UdpSocket::bind(addr).await.unwrap();
        socket.set_broadcast(true).unwrap();

        Ok(BroadcastClientPeer {
            socket,
            hasher: Box::new(StreebogHasher::new()),
        })
    }
}

impl ClientPeer for BroadcastClientPeer {
    async fn send(&self, chunk: &[u8]) -> Result<Vec<u8>> {
        let hash = self.hasher.calc_hash_for_chunk(chunk).unwrap();
        let message = Message::new(
            MessageType::SendingReq,
            &hash,
            None,
        ).as_encoded_json().unwrap();
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

        let message = Message::new(
            MessageType::ContentFilled,
            &hash,
            Some(chunk.to_vec()),
        ).as_encoded_json().unwrap();
        self.socket.send_to(&message, peer_addr.unwrap()).await.unwrap();

        Ok(hash)
    }

    async fn recv(&self, hash: &[u8]) -> Result<Vec<u8>> {
        todo!()
    }
}

mod errors {
    use std::fmt;
    use std::fmt::Formatter;

    pub trait ClientPeerError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result;
    }
}