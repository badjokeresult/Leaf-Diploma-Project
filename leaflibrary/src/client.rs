use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use net2::UdpBuilder;
use net2::unix::UnixUdpBuilderExt;

use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use errors::*;
use crate::hash::StreebogHasher;
use crate::{Hasher, Message};

pub trait UdpClient {
    fn send_chunk(&self, data: &[u8]) -> impl std::future::Future<Output = Result<Vec<u8>, SendingChunkError>> + Send;
    fn recv_chunk(&self, hash: &[u8]) -> impl std::future::Future<Output = Result<Vec<u8>, ReceivingChunkError>> + Send;
}

pub struct BroadcastUdpClient {
    socket: UdpSocket,
    hasher: Mutex<StreebogHasher>,
    broadcast_addr: SocketAddr,
}

impl BroadcastUdpClient {
    pub async fn new(local_addr: &str, broadcast_addr: &str) -> Result<BroadcastUdpClient, ClientInitError> {
        let udp_socket = match UdpBuilder::new_v4() {
            Ok(b) => match b.reuse_address(true) {
                Ok(b) => match b.reuse_port(true) {
                    Ok(b) => match b.bind(local_addr) {
                        Ok(s) => s,
                        Err(e) => return Err(ClientInitError(e.to_string())),
                    },
                    Err(e) => return Err(ClientInitError(e.to_string())),
                },
                Err(e) => return Err(ClientInitError(e.to_string())),
            },
            Err(e) => return Err(ClientInitError(e.to_string())),
        };

        match udp_socket.set_broadcast(true) {
            Ok(_) => {},
            Err(e) => return Err(ClientInitError(e.to_string())),
        };
        match udp_socket.set_write_timeout(Some(Duration::new(5, 0))) {
            Ok(_) => {},
            Err(e) => return Err(ClientInitError(e.to_string())),
        };
        match udp_socket.set_read_timeout(Some(Duration::new(5, 0))) {
            Ok(_) => {},
            Err(e) => return Err(ClientInitError(e.to_string())),
        };

        let socket = match UdpSocket::from_std(udp_socket) {
            Ok(s) => s,
            Err(e) => return Err(ClientInitError(e.to_string())),
        };

        let hasher = Mutex::new(StreebogHasher::new());

        let broadcast_addr = match SocketAddr::from_str(broadcast_addr) {
            Ok(a) => a,
            Err(e) => return Err(ClientInitError(e.to_string())),
        };

        Ok(BroadcastUdpClient {
            socket,
            hasher,
            broadcast_addr,
        })
    }
}

impl UdpClient for BroadcastUdpClient {
    async fn send_chunk(&self, data: &[u8]) -> Result<Vec<u8>, SendingChunkError> {
        let hash = self.hasher.lock().await.calc_hash_for_chunk(data);

        let req: Vec<u8> = Message::SendingReq(hash.clone()).into();
        match self.socket.send_to(&req, self.broadcast_addr).await {
            Ok(_) => {},
            Err(e) => return Err(SendingChunkError(e.to_string())),
        };

        let mut buf = [0u8; 65507];
        if let Ok((sz, addr)) = self.socket.recv_from(&mut buf).await {
            let ack = Message::from(buf[..sz].to_vec());
            if let Message::SendingAck(h) = ack {
                if h.eq(&hash) {
                    let content: Vec<Vec<u8>> = Message::new_with_data(&hash, data)
                        .iter().map(|x| x.clone().into()).collect::<Vec<Vec<_>>>();
                    for msg in &content {
                        match self.socket.send_to(&msg, addr).await {
                            Ok(_) => {},
                            Err(e) => return Err(SendingChunkError(e.to_string())),
                        };
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    return Ok(hash);
                };
            };
        };
        Err(SendingChunkError(String::from("No acknowledgement received")))
    }

    async fn recv_chunk(&self, hash: &[u8]) -> Result<Vec<u8>, ReceivingChunkError> {
        let req: Vec<u8> = Message::RetrievingReq(hash.to_vec()).into();
        match self.socket.send_to(&req, self.broadcast_addr).await {
            Ok(_) => {},
            Err(e) => return Err(ReceivingChunkError(e.to_string())),
        };

        let mut result = vec![];
        let mut buf = [0u8; 65507];
        if let Ok((sz, addr)) = self.socket.recv_from(&mut buf).await {
            let ack = Message::from(buf[..sz].to_vec());
            if let Message::RetrievingAck(h) = ack {
                if h.eq(&hash) {
                    while let Ok((peer_sz, peer_addr)) = self.socket.recv_from(&mut buf).await {
                        let content = Message::from(buf[..peer_sz].to_vec());
                        if peer_addr.eq(&addr) {
                            if let Message::ContentFilled(h, mut d) = content {
                                if h.eq(&hash) {
                                    result.append(&mut d);
                                };
                            } else if let Message::Empty(h) = content {
                                if h.eq(&hash) {
                                    return Ok(result);
                                };
                            } else {
                                return Err(ReceivingChunkError(String::from("Unexpected message type")));
                            }
                        };
                    };
                };
            } else {
                return Err(ReceivingChunkError(String::from("No acknowledgement received")));
            }
        };

        if result.len() == 0 {
            return Err(ReceivingChunkError(String::from("Data cannot be received")));
        }
        Ok(result)
    }
}

mod errors {
    use std::fmt;
    use std::fmt::Formatter;

    #[derive(Debug, Clone)]
    pub struct SendingChunkError(pub String);

    impl fmt::Display for SendingChunkError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error sending chunk into domain: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct ReceivingChunkError(pub String);

    impl fmt::Display for ReceivingChunkError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error receiving from domain: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct ClientInitError(pub String);

    impl fmt::Display for ClientInitError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error init client: {}", self.0)
        }
    }
}