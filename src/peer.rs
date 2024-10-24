use std::net::UdpSocket;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Mutex;
use crate::message::Message;

pub struct Peer {
    socket: Arc<Mutex<UdpSocket>>,
    from_client_receiver: Receiver<Message>,
    to_client_sender: Sender<Message>,
}

impl Peer {
    pub fn new(socket: Arc<Mutex<UdpSocket>>, receiver: Receiver<Message>, sender: Sender<Message>) -> Peer {
        Peer { socket, receiver, sender }
    }

    pub fn run(&self, num_threads: usize) {
        for i in 0..num_threads {
            let socket = Arc::clone(&self.socket);
            let sender = self.sender.clone();
        }
    }
}