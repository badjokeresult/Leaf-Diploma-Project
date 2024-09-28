use std::net::SocketAddr;
use local_ip_address::local_ip;
use tokio::net::UdpSocket;
use leaf_common::consts::{DEFAULT_SERVER_PORT, MAX_DATAGRAM_SIZE};
use leaf_common::message::{Message, MessageType};

pub trait ServerPeer {
    async fn listen(&'static self);
}

pub struct BroadcastServerPeer {
    socket: UdpSocket,
}

impl BroadcastServerPeer {
    pub async fn new() -> BroadcastServerPeer {
        let addr = SocketAddr::new(local_ip().unwrap(), DEFAULT_SERVER_PORT);
        let socket = UdpSocket::bind(addr).await.unwrap();

        BroadcastServerPeer {
            socket,
        }
    }
}

impl ServerPeer for BroadcastServerPeer {
    async fn listen(&'static self) {
        let mut buf = [0u8; MAX_DATAGRAM_SIZE];
        loop {
            let (_, addr) = self.socket.recv_from(&mut buf).await.unwrap();
            let message = Message::from_encoded_json(&buf).unwrap();
            match message.msg_type {
                MessageType::SendingReq =>
            }
        }
    }
}