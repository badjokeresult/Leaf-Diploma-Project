use std::cell::RefCell;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::net::UdpSocket;

use common::{MessageBuilder, MessageType};
use crate::storage::BroadcastServerStorage;

pub trait ServerPeer {
    async fn listen(&self);
}

pub struct BroadcastServerPeer {
    socket: UdpSocket,
    storage: RefCell<BroadcastServerStorage>,
    message_builder: MessageBuilder,
}

impl BroadcastServerPeer {
    pub async fn new(filepath: PathBuf) -> BroadcastServerPeer {
        let socket = UdpSocket::bind("0.0.0.0:62092").await.unwrap();
        socket.set_broadcast(true).unwrap();
        let storage = RefCell::new(BroadcastServerStorage::new(filepath).await);
        let message_builder = MessageBuilder::new();

        BroadcastServerPeer {
            socket,
            storage,
            message_builder,
        }
    }
}

impl ServerPeer for BroadcastServerPeer {
    async fn listen(&self) {
        loop {
            let mut buf = [0u8; 65536];
            let (sz, addr) = self.socket.recv_from(&mut buf).await.unwrap();
            let message = self.message_builder.deconstruct_encoded_message(&buf[..sz]).unwrap();
            match message.get_type() {
                MessageType::SendingReq => self.handle_sending_req(&message.get_hash(), addr).await,
                MessageType::RetrievingReq => self.handle_retrieving_req(&message.get_hash(), addr).await,
                MessageType::ContentFilled => self.handle_content(&message.get_hash(), &message.get_data().unwrap()).await,
                _ => eprintln!("INVALID MESSAGE TYPE"),
            }
        }
    }
}

impl BroadcastServerPeer {
    async fn handle_sending_req(&self, hash: &[u8], addr: SocketAddr) {
        let message = self.message_builder.build_encoded_message(
            MessageType::SendingAck.into(),
            hash,
            None,
        ).unwrap();
        self.socket.send_to(&message, addr).await.unwrap();
    }

    async fn handle_retrieving_req(&self, hash: &[u8], addr: SocketAddr) {
        let data = match self.storage.borrow().database.get(hash) {
            Some(d) => d.to_vec(),
            None => return,
        };
        let message = self.message_builder.build_encoded_message(
            MessageType::RetrievingAck.into(),
            hash,
            Some(data),
        ).unwrap();
        self.socket.send_to(&message, addr).await.unwrap();
    }

    async fn handle_content(&self, hash: &[u8], data: &[u8]) {
        self.storage.borrow_mut().database.insert(hash.to_vec(), data.to_vec());
    }
}