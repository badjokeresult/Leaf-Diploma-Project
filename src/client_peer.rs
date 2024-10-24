use std::net::UdpSocket;
use std::io::Error;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::time::{Duration, Instant};
use crate::hash::{Hasher, StreebogHasher};
use crate::message::{MessageBuilder, MessageType};

pub trait ClientPeer {
    fn send(&self, chunk: &[u8]) -> Result<Vec<u8>, Error>;
    fn recv(&self, hash: &[u8]) -> Result<Vec<u8>, Error>;
}

pub struct BroadcastClientPeer {
    socket: UdpSocket,
    hasher: StreebogHasher,
    message_builder: MessageBuilder,
}

impl BroadcastClientPeer {
    pub fn new() -> BroadcastClientPeer {
        let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
        socket.set_broadcast(true).unwrap();
        let hasher = StreebogHasher::new();
        let message_builder = MessageBuilder::new();

        BroadcastClientPeer {
            socket,
            hasher,
            message_builder,
        }
    }
}

impl ClientPeer for BroadcastClientPeer {
    fn send(&self, chunk: &[u8]) -> Result<Vec<u8>, Error> {
        let hash = self.hasher.calc_hash_for_chunk(chunk);
        self.send_req(&hash, MessageType::SendingReq).unwrap();
        let addr = self.recv_ack(&hash, MessageType::SendingAck).unwrap();
        self.send_content(&hash, chunk, addr).unwrap();
        Ok(hash)
    }

    fn recv(&self, hash: &[u8]) -> Result<Vec<u8>, Error> {
        self.send_req(hash, MessageType::RetrievingReq).unwrap();
        let addr = self.recv_ack(hash, MessageType::RetrievingAck).unwrap();
        let data = self.recv_content(hash, addr).unwrap();
        Ok(data)
    }
}

impl BroadcastClientPeer {
    fn send_req(&self, hash: &[u8], req_type: MessageType) -> Result<(), Error> {
        let message = self.message_builder.build_encoded_message(
            req_type.into(),
            hash,
            None,
        ).unwrap();
        let broadcast_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::from_str("192.168.124.255").unwrap()), 62092);
        self.socket.send_to(&message, broadcast_addr).unwrap();
        Ok(())
    }

    fn recv_ack(&self, hash: &[u8], ack_type: MessageType) -> Result<SocketAddr, Error> {
        let start = Instant::now();
        loop {
            let mut buf = [0u8; 65536];
            let (sz, addr) = self.socket.recv_from(&mut buf).unwrap();
            let message = self.message_builder.deconstruct_encoded_message(&buf[..sz]).unwrap();
            if message.get_type() == ack_type && message.get_hash().eq(hash) {
                return Ok(addr);
            }
            if start.elapsed().eq(&Duration::from_secs(3)) {
                return Err(Error::last_os_error());
            }
        }
    }

    fn send_content(&self, hash: &[u8], chunk: &[u8], addr: SocketAddr) -> Result<(), Error> {
        let message = self.message_builder.build_encoded_message(
            MessageType::ContentFilled.into(),
            hash,
            Some(chunk.to_vec()),
        ).unwrap();
        self.socket.send_to(&message, addr).unwrap();
        Ok(())
    }

    fn recv_content(&self, hash: &[u8], addr: SocketAddr) -> Result<Vec<u8>, Error> {
        let start = Instant::now();
        loop {
            let mut buf = [0u8; 65536];
            let (sz, new_addr) = self.socket.recv_from(&mut buf).unwrap();
            let message = self.message_builder.deconstruct_encoded_message(&buf[..sz]).unwrap();
            if new_addr.eq(&addr) && message.get_type() == MessageType::ContentFilled && message.get_hash().iter().eq(hash) {
                return Ok(message.get_data().unwrap());
            }
            if start.elapsed().eq(&Duration::from_secs(3)) {
                return Err(Error::last_os_error());
            }
        }
    }
}