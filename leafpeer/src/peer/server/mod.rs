use std::cell::RefCell;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use local_ip_address::local_ip;
use tokio::sync::Mutex;
use tokio::task;
use tokio::net::UdpSocket;

use crate::messages::{MessageType, RETRIEVING_ACK_MSG_TYPE, SENDING_ACK_MSG_TYPE};
use crate::storage::{ChunksStorage, LocalChunksStorage};
use crate::codec;

pub trait ServerPeer {
    async fn listen(&'static self);
    async fn proper_shutdown(&self);
}

pub struct BroadcastServerPeer {
    socket: UdpSocket,
    port: u16,
    storage: Arc<Mutex<RefCell<LocalChunksStorage>>>,
}

impl BroadcastServerPeer {
    pub async fn new(port: u16) -> BroadcastServerPeer {
        let local_ip_addr = SocketAddr::new(local_ip().unwrap(), port);
        let socket = UdpSocket::bind(local_ip_addr).await.unwrap();

        let storage = Arc::new(Mutex::new(RefCell::new(LocalChunksStorage::new().await)));

        BroadcastServerPeer {
            socket,
            port,
            storage,
        }
    }
}

impl ServerPeer for BroadcastServerPeer {
    async fn listen(&'static self) {
        loop {
            let mut buf = [0u8; 4096];
            let _ = self.socket.recv_from(buf.as_mut()).await.unwrap();
            let message = codec::decode_message_from_b64_bytes(buf.as_slice());
            match message {
                MessageType::SendingReq(i, h) => self.handle_sending_req(i, &h).await,
                MessageType::RetrievingReq(i, h) => self.handle_retrieving_req(i, &h).await,
                _ => continue,
            };
        }
    }

    async fn proper_shutdown(&self) {
        self.storage.lock().await.borrow().save_meta_before_shutdown();
    }
}

impl BroadcastServerPeer {
    async fn handle_sending_req(&'static self, ip: IpAddr, hash: &[u8]) {
        let sending_ack = MessageType::build_message(self.socket.local_addr().unwrap(), hash, SENDING_ACK_MSG_TYPE).unwrap();
        let ack_as_b64 = codec::encode_message_as_b64_bytes(sending_ack);
        let socket = SocketAddr::new(ip, self.port);
        self.socket.send_to(&ack_as_b64, socket).await.unwrap();

        let buf = task::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                let (_, addr) = self.socket.recv_from(&mut buf).await.unwrap();
                if !addr.ip().eq(&ip) {
                    buf.fill(0u8);
                    continue;
                }
                break;
            }
            buf
        }).await.unwrap();

        self.storage.lock().await.borrow_mut().save(&buf, hash).await;
    }

    async fn handle_retrieving_req(&self, ip: IpAddr, hash: &[u8]) {
        let chunk = self.storage.lock().await.borrow_mut().retrieve(hash).await.unwrap();
        let retrieving_ack = MessageType::build_message(self.socket.local_addr().unwrap(), &chunk, RETRIEVING_ACK_MSG_TYPE).unwrap();
        let ack_as_b64 = codec::encode_message_as_b64_bytes(retrieving_ack);
        let socket = SocketAddr::new(ip, self.port);
        self.socket.send_to(&ack_as_b64, socket).await.unwrap();
    }
}
