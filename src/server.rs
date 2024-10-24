use std::collections::HashMap;
use std::io::{Error, ErrorKind};
use std::sync::mpsc::{channel, Receiver, Sender};
use tools::{Message, MessageType};

pub struct Server {
    to_peer_sender: Sender<Message>,
    storage: Storage,
}

impl Server {
    pub fn new() -> (Server, Receiver<Message>) {
        let (to_peer_sender, from_peer_receiver) = channel::<Message>();
        let storage = Storage::new();

        (Server {
            to_peer_sender,
            storage,
        }, from_peer_receiver)
    }
}

impl Server {
    pub fn handle_sending_req(&mut self, message: Message) {
        if self.storage.is_enough_mem() {
            self.storage.alloc_mem_for_chunk(&message.get_hash()).unwrap();
            let message = Message::new(MessageType::SendingAck.into(), &message.get_hash(), None);
            self.to_peer_sender.send(message).unwrap()
        }
    }
}

pub struct Storage {
    database: HashMap<Vec<u8>, Vec<u8>>,
}

impl Storage {
    pub fn new() -> Storage {
        Storage {
            database: HashMap::new(),
        }
    }
}

impl Storage {
    pub fn is_enough_mem(&self) -> bool {
        true
    }

    pub fn alloc_mem_for_chunk(&mut self, hash: &[u8]) -> Result<(), Error> {
        match self.database.get(hash) {
            Some(data) => match data.len() {
                0 => Ok(()),
                _ => Err(Error::new(ErrorKind::AlreadyExists, "Already allocated")),
            },
            None => {
                self.database.insert(hash.to_vec(), Vec::new());
                Ok(())
            },
        }
    }
}