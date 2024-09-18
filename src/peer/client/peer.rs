use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::time::{Instant, Duration};

use atomic_refcell::AtomicRefCell;
use local_ip_address::{local_broadcast_ip, local_ip};
use tokio::net::UdpSocket;

use crate::messages::types::{Message, RETRIEVING_REQ_MSG_TYPE, SENDING_REQ_MSG_TYPE};
use crate::hash::hasher::{sha::Sha3_256Hasher, Hasher, gost::StreebogHasher};
use crate::codec::codec::{Codec, Base64Codec};
use super::errors::*;
use super::storage::ClientCurrentState;

const MAX_DATAGRAM_SIZE: usize = 508;

static ALLOWED_TIMEOUT: Duration = Duration::new(3, 0);

type Result<T> = std::result::Result<T, Box<dyn ClientSidePeerError>>;

pub trait ClientPeer {
    async fn send(&'static self, chunk: &[u8], gost: bool) -> Vec<u8>;
    async fn receive(&'static self, hash: &[u8], gost: bool) -> Vec<u8>;
}

pub struct BroadcastClientPeer {
    socket: UdpSocket,
    dest: SocketAddr,
    storage: AtomicRefCell<ClientCurrentState>,

}

impl BroadcastClientPeer {
    pub async fn new(port: u16) -> Result<BroadcastClientPeer> {
        let socket = UdpSocket::bind(local_ip().unwrap().to_string() + ":0").await?;
        socket.set_broadcast(true)?;

        let conn_string = String::from(local_broadcast_ip()?.to_string());
        let dest = SocketAddr::new(IpAddr::V4(Ipv4Addr::from_str(&conn_string)?), port);

        let storage = AtomicRefCell::new(ClientCurrentState::new());

        Ok(BroadcastClientPeer {
            socket,
            dest,
            storage,
        })
    }
}

impl ClientPeer for BroadcastClientPeer {
    async fn send(&'static self, chunk: &[u8], gost: bool) -> Result<Vec<u8>> {
        let mut hash: Vec<u8>;
        if gost {
            hash = StreebogHasher::calc_hash_for_chunk(chunk).unwrap();
        } else {
            hash = Sha3_256Hasher::calc_hash_for_chunk(chunk).unwrap();
        }

        let message = Message::build_message(self.socket.local_addr()?, &hash, SENDING_REQ_MSG_TYPE)?;
        let message_as_b64 = Base64Codec::encode_message(message)?;

        self.socket.send_to(&message_as_b64, self.dest).await?;
        let peer_addr = self.receive_sending_ack(&hash, gost).await;
        self.socket.send_to(chunk, peer_addr).await?;

        Ok(hash)
    }

    async fn receive(&'static self, hash: &[u8], gost: bool) -> Result<Vec<u8>> {
        let message = Message::build_message(self.socket.local_addr()?, hash, RETRIEVING_REQ_MSG_TYPE)?;
        let message_as_b64 = Base64Codec::encode_message(message)?;

        self.socket.send_to(&message_as_b64, self.dest).await?;
        let peer_addr = self.receive_retrieving_ack(hash, gost).await?;
        if let Some(a) = peer_addr.unwrap() {
            return self.receive_chunk_from_selected_peer(a).await;
        }
        Err(Box::new(RetrievingTimeoutError(String::from("No peer address was retrieved"))))
    }
}

impl BroadcastClientPeer {
    async fn receive_sending_ack(&'static self, hash: &[u8], gost: bool) -> Result<Option<SocketAddr>> {
        let mut buf = [0u8; MAX_DATAGRAM_SIZE];

        let mut peer_addr = None;
        let start = Instant::now();
        loop {
            let (_, addr) = self.socket.recv_from(&mut buf).await?;
            let message = Base64Codec::decode_message(&buf)?;
            let recv_hash = match message {
                Message::SendingAck(v) => v,
                _ => {
                    buf.fill(0u8);
                    continue
                },
            };
            if !gost {
                if <Sha3_256Hasher as Hasher>::is_hash_equal_to_model(&recv_hash, hash) {
                    peer_addr = Some(addr);
                    break;
                }
            } else {
                if <StreebogHasher as Hasher>::is_hash_equal_to_model(&recv_hash, hash) {
                    peer_addr = Some(addr);
                    break;
                }
            }
            if start.elapsed().gt(&ALLOWED_TIMEOUT) {
                break;
            }
        };

        Ok(peer_addr)
    }

    async fn receive_retrieving_ack(&'static self, hash: &[u8], gost: bool) -> Result<Option<SocketAddr>> {
        let mut buf = [0u8; MAX_DATAGRAM_SIZE];
        let mut peer_addr = None;

        let start = Instant::now();
        loop {
            let (_, addr) = self.socket.recv_from(&mut buf).await?;
            let message = Base64Codec::decode_message(&mut buf)?;
            let recv_hash = match message {
                Message::RetrievingAck(v) => v,
                _ => {
                    buf.fill(0u8);
                    continue;
                },
            };
            if !gost {
                if <Sha3_256Hasher as Hasher>::is_hash_equal_to_model(&recv_hash, hash) {
                    peer_addr = Some(addr);
                    break;
                }
            } else {
                if <StreebogHasher as Hasher>::is_hash_equal_to_model(&recv_hash, hash) {
                    peer_addr = Some(addr);
                    break;
                }
            }
            if start.elapsed().gt(&ALLOWED_TIMEOUT) {
                break;
            }
        }

        Ok(peer_addr)
    }

    async fn receive_chunk_from_selected_peer(&'static self, addr: SocketAddr) -> Result<Vec<u8>> {
        let mut buf = [0u8; MAX_DATAGRAM_SIZE];
        let start = Instant::now();
        loop {
            if start.elapsed().gt(&ALLOWED_TIMEOUT) {
                break;
            }
            let (_, addr_) = self.socket.recv_from(&mut buf).await?;
            if !addr_.eq(&addr) {
                buf.fill(0u8);
                continue;
            };
            break;
        };

        match buf.is_empty() {
            true => Err(Box::new(RetrievingTimeoutError(String::from("Timeout during retrieving data")))),
            false => {
                let buf_vec = buf.to_vec();
                Ok(buf_vec)
            }
        }
    }
}