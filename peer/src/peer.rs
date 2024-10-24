use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use atomic_refcell::AtomicRefCell;
use tools::{Message, MessageType};

const MAX_DATAGRAM_SIZE: usize = 65507;

pub struct Peer {
    socket: Arc<Mutex<UdpSocket>>,
    storage: AtomicRefCell<HashMap<Vec<u8>, Vec<u8>>>,
}

impl Peer {
    pub fn new(addr: SocketAddr) -> Peer {
        let socket = Arc::new(Mutex::new(UdpSocket::bind(addr).unwrap()));
        let storage = AtomicRefCell::new(HashMap::new());

        Peer { socket, storage }
    }

    pub fn run(&self, num_threads: usize) -> Vec<JoinHandle<()>> {
        for _ in 0..num_threads {
            let socket = Arc::clone(&self.socket);

            let mut buf = [0u8; MAX_DATAGRAM_SIZE];
            thread::spawn(move || {
                loop {
                    match socket.lock().unwrap().recv_from(&mut buf) {
                        Ok((s, a)) => {
                            let message = Message::from(&buf[..s]);
                            match message.get_type() {
                                MessageType::SendingReq => {
                                    self.handle_sending_req(&message.get_hash());
                                    let answer = Message::new(MessageType::SendingAck.into(), &message.get_hash(), None);
                                    socket.lock().unwrap().send_to(&message.into(), a).unwrap();
                                },
                                MessageType::RetrievingReq => {
                                    let data = self.handle_retrieving_req(&message.get_hash());
                                    let answer = Message::new(MessageType::RetrievingAck.into(), &message.get_hash(), Some(data));
                                    socket.lock().unwrap().send_to(&message.into(), a).unwrap();
                                },
                                MessageType::RetrievingAck => {

                                }
                            }
                        }
                    }
                }
            })
        }
    }

    fn handle_sending_req(&self, hash: &[u8]) {
        if self.is_enough_mem() {
            self.alloc_mem_for_chunk(hash);
        }
    }

    fn handle_retrieving_req(&self, hash: &[u8]) -> Vec<u8> {

    }

    fn is_enough_mem(&self) -> bool {
        true
    }

    fn alloc_mem_for_chunk(&self, hash: &[u8]) {
        match self.storage.borrow().get(hash) {
            Some(d) => panic!(),
            None => {
                self.storage.borrow_mut().insert(hash.to_vec(), vec![]);
                return;
            }
        };
    }
}