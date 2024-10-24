use std::cell::RefCell;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::net::UdpSocket;

use crate::storage::BroadcastServerStorage;

pub trait ServerPeer {
    fn listen(&self);
}

pub struct BroadcastServerPeer {
    socket: UdpSocket,
    storage: RefCell<BroadcastServerStorage>,
    message_builder: MessageBuilder,
}

impl BroadcastServerPeer {
    pub fn new(filepath: PathBuf) -> BroadcastServerPeer {
        let socket = UdpSocket::bind("0.0.0.0:62092").unwrap();
        socket.set_broadcast(true).unwrap();
        let storage = RefCell::new(BroadcastServerStorage::new(filepath));
        let message_builder = MessageBuilder::new();

        BroadcastServerPeer {
            socket,
            storage,
            message_builder,
        }
    }
}

impl ServerPeer for BroadcastServerPeer {
    fn listen(&self) {
        loop {
            let mut buf = [0u8; 65536];
            let (sz, addr) = self.socket.recv_from(&mut buf).unwrap();
            let message = self.message_builder.deconstruct_encoded_message(&buf[..sz]).unwrap();
            match message.get_type() {
                MessageType::SendingReq => self.handle_sending_req(&message.get_hash(), addr),
                MessageType::RetrievingReq => self.handle_retrieving_req(&message.get_hash(), addr),
                MessageType::ContentFilled => self.handle_content(&message.get_hash(), &message.get_data().unwrap()),
                _ => eprintln!("INVALID MESSAGE TYPE"),
            }
        }
    }
}

impl BroadcastServerPeer {
    fn handle_sending_req(&self, hash: &[u8], addr: SocketAddr) {
        let message = self.message_builder.build_encoded_message(
            MessageType::SendingAck.into(),
            hash,
            None,
        ).unwrap();
        self.socket.send_to(&message, addr).unwrap();
    }

    fn handle_retrieving_req(&self, hash: &[u8], addr: SocketAddr) {
        let data = match self.storage.borrow().database.get(hash) {
            Some(d) => d.to_vec(),
            None => return,
        };
        let message = self.message_builder.build_encoded_message(
            MessageType::RetrievingAck.into(),
            hash,
            Some(data),
        ).unwrap();
        self.socket.send_to(&message, addr).unwrap();
    }

    fn handle_content(&self, hash: &[u8], data: &[u8]) {
        self.storage.borrow_mut().database.insert(hash.to_vec(), data.to_vec());
    }
}