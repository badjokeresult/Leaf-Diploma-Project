use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use net2::UdpBuilder;
use net2::unix::UnixUdpBuilderExt;
use tokio::net::UdpSocket;
use crate::{Hasher, Message, StreebogHasher};

pub struct BroadcastUdpClient {
    socket: UdpSocket,
    hasher: StreebogHasher,
    broadcast_addr: SocketAddr,
}

impl BroadcastUdpClient {
    pub async fn new(local_addr: &str, broadcast_addr: &str) -> BroadcastUdpClient {
        let socket = UdpBuilder::new_v4().unwrap()
            .reuse_address(true).unwrap()
            .reuse_port(true).unwrap()
            .bind(local_addr).unwrap();
        socket.set_broadcast(true).unwrap();
        socket.set_write_timeout(Some(Duration::new(5, 0))).unwrap();
        socket.set_read_timeout(Some(Duration::new(5, 0))).unwrap();

        let socket = UdpSocket::from_std(socket).unwrap();

        let hasher = StreebogHasher::new();

        let broadcast_addr = SocketAddr::from_str(broadcast_addr).unwrap();

        BroadcastUdpClient {
            socket,
            hasher,
            broadcast_addr,
        }
    }

    pub async fn send_data(&self, data: &[u8]) -> Result<Vec<u8>, tokio::io::Error> {
        let hash = self.hasher.calc_hash_for_chunk(data);

        let req: Vec<u8> = Message::SendingReq(hash.clone()).into();
        self.socket.send_to(&req, self.broadcast_addr).await?;
        println!("SENT SENDING REQ TO {}", self.broadcast_addr);

        let mut buf = [0u8; 65507];
        if let Ok((sz, addr)) = self.socket.recv_from(&mut buf).await {
            let ack = Message::from(buf[..sz].to_vec());
            if let Message::SendingAck(h) = ack {
                if h.eq(&hash) {
                    println!("RECEIVED SENDING ACK FROM {}", addr);
                    let content: Vec<Vec<u8>> = Message::new_with_data(&hash, data)
                        .iter().map(|x| x.clone().into()).collect::<Vec<Vec<_>>>();
                    println!("LEN OF CONTENT MESSAGES : {}", content.len());
                    for msg in &content {
                        self.socket.send_to(&msg, addr).await?;
                    }
                    println!("SENDING CONTENT FINISHED");
                };
            };
        };

        Err(tokio::io::Error::last_os_error())
    }

    pub async fn recv_data(&self, hash: &[u8]) -> Result<Vec<u8>, tokio::io::Error> {
        let req: Vec<u8> = Message::RetrievingReq(hash.to_vec()).into();
        self.socket.send_to(&req, self.broadcast_addr).await?;

        let mut result = vec![];
        let mut buf = [0u8; 65507];
        if let Ok((sz, addr)) = self.socket.recv_from(&mut buf).await {
            let ack = Message::from(buf[..sz].to_vec());
            if let Message::RetrievingAck(h) = ack {
                if h.eq(&hash) {
                    buf.fill(0u8);
                    let (peer_sz, peer_addr) = self.socket.recv_from(&mut buf).await?;
                    let content = Message::from(buf[..peer_sz].to_vec());
                    if peer_addr.eq(&addr) {
                        if let Message::ContentFilled(h, mut d) = content {
                            if h.eq(&hash) {
                                result.append(&mut d);
                            }
                        }
                    }
                };
            };
        };

        Ok(result)
    }
}