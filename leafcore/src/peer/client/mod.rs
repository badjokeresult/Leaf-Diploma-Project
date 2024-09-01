use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;

use local_ip_address::{local_broadcast_ip, local_ip};
use tokio::net::UdpSocket;
use tokio::task;

use crate::messages::{MessageType, RETRIEVING_REQ_MSG_TYPE, SENDING_REQ_MSG_TYPE};
use crate::hash;
use crate::hash::is_hash_equal_to_model;
use crate::codec;

pub trait ClientPeer {
    async fn send(&'static self, chunk: &[u8]) -> Vec<u8>;
    async fn receive(&'static self, hash: &[u8]) -> Vec<u8>;
}

pub struct BroadcastClientPeer {
    socket: UdpSocket,
    dest: SocketAddr
}

impl BroadcastClientPeer {
    pub async fn new(port: u16) -> BroadcastClientPeer {
        let socket = UdpSocket::bind(local_ip().unwrap().to_string() + ":0").await.unwrap();
        socket.set_broadcast(true).unwrap();

        let conn_string = String::from(local_broadcast_ip().unwrap().to_string());
        let dest = SocketAddr::new(IpAddr::V4(Ipv4Addr::from_str(&conn_string).unwrap()), port);

        BroadcastClientPeer {
            socket,
            dest,
        }
    }
}

impl ClientPeer for BroadcastClientPeer {
    async fn send(&'static self, chunk: &[u8]) -> Vec<u8> {
        let hash = hash::calc_hash_for_chunk(chunk);
        let message = MessageType::build_message(self.socket.local_addr().unwrap(), &hash, SENDING_REQ_MSG_TYPE).unwrap();
        let message_as_b64 = codec::encode_message_as_b64_bytes(message);

        self.socket.send_to(&message_as_b64, self.dest).await.unwrap();
        let peer_addr = self.receive_sending_ack(&hash).await;
        self.socket.send_to(chunk, peer_addr).await.unwrap();

        hash
    }

    async fn receive(&'static self, hash: &[u8]) -> Vec<u8> {
        let message = MessageType::build_message(self.socket.local_addr().unwrap(), hash, RETRIEVING_REQ_MSG_TYPE).unwrap();
        let message_as_b64 = codec::encode_message_as_b64_bytes(message);

        self.socket.send_to(&message_as_b64, self.dest).await.unwrap();
        let peer_addr = self.receive_retrieving_ack(hash).await;
        let chunk = self.receive_chunk_from_selected_peer(peer_addr).await;
        chunk
    }
}

impl BroadcastClientPeer {
    async fn receive_sending_ack(&'static self, hash: &[u8]) -> SocketAddr {
        let mut buf = [0u8; 4096];

        let mut peer_addr = None;
        loop {
            let (_, addr) = self.socket.recv_from(&mut buf).await.unwrap();
            let message = codec::decode_message_from_b64_bytes(&mut buf);
            let recv_hash = match message {
                MessageType::SendingAck(v) => v,
                _ => {
                    buf.fill(0u8);
                    continue
                },
            };
            if is_hash_equal_to_model(&recv_hash, hash) {
                peer_addr = Some(addr);
                break;
            }
        };

        peer_addr.unwrap()
    }

    async fn receive_retrieving_ack(&'static self, hash: &[u8]) -> SocketAddr {
        let mut buf = [0u8; 4096];

        let peer_addr = task::spawn(async move {
            loop {
                let (_, addr) = self.socket.recv_from(&mut buf).await.unwrap();
                let message = codec::decode_message_from_b64_bytes(&mut buf);
                let recv_hash = match message {
                    MessageType::RetrievingAck(v) => v,
                    _ => {
                        buf.fill(0u8);
                        continue;
                    },
                };
                if is_hash_equal_to_model(&recv_hash, hash) {
                    return addr;
                }
            }
        }).await.unwrap();

        peer_addr
    }

    async fn receive_chunk_from_selected_peer(&'static self, addr: SocketAddr) -> Vec<u8> {
        let buf = task::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                let (_, addr_) = self.socket.recv_from(&mut buf).await.unwrap();
                if !addr_.eq(&addr) {
                    buf.fill(0u8);
                    continue;
                };
                break;
            };
            let x = buf.to_vec();
            x
        }).await.unwrap();

        buf
    }
}
