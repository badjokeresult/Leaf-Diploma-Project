use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Error;
use std::net::UdpSocket;
use std::sync::Arc;
use std::time::Duration;

use net2::{UdpBuilder, UdpSocketExt};
use net2::unix::UnixUdpBuilderExt;

use crate::message::Message;
use crate::message::consts::*;

const MAX_DATAGRAM_SIZE: usize = 65507;

#[derive(Clone)]
pub struct BroadcastUdpServer {
    socket: Arc<UdpSocket>,
    storage: RefCell<HashMap<Vec<u8>, Vec<u8>>>,
}

impl BroadcastUdpServer {
    pub fn new(addr: &str) -> BroadcastUdpServer {
        let socket = Arc::new(UdpBuilder::new_v4().unwrap()
            .reuse_address(true).unwrap()
            .reuse_port(true).unwrap()
            .bind(addr).unwrap());
        socket.set_broadcast(true).unwrap();
        socket.set_write_timeout(Some(Duration::new(5, 0))).unwrap();
        socket.set_read_timeout(Some(Duration::new(5, 0))).unwrap();

        let storage = RefCell::new(HashMap::new());

        BroadcastUdpServer {
            socket,
            storage,
        }
    }

    pub fn listen(&self) {
        let mut buf = [0u8; MAX_DATAGRAM_SIZE];
        loop {
            let (sz, addr) = self.socket.recv_from(&mut buf).unwrap();
            let message = Message::from(buf[..sz]);
            match message.clone() {
                Message::RetrievingReq(h) => {
                    let messages = self.handle_retrieving_req(&h).unwrap();
                    for message in messages {
                        self.socket.send_to(&message, addr).unwrap();
                    };
                },
                Message::SendingReq(h) => {
                    let message = self.handle_sending_req(&h).unwrap();
                    self.socket.send_to(&message, addr).unwrap();
                },
                Message::ContentFilled(h, d) => {
                    self.handle_content_filled(&h, &d).unwrap();
                },
                _ => continue,
            };
        }
    }

    fn handle_retrieving_req(&self, hash: &[u8]) -> Result<Vec<Vec<u8>>, Error> {
        match self.storage.borrow().get(hash) {
            Some(c) => {
                let mut content = vec![Message::new(RETRIEVING_ACKNOWLEDGEMENT_TYPE, hash)];
                content.append(&mut Message::new_with_data(CONTENT_FILLED_TYPE, hash, c.clone()));
                Ok(content.iter().map(|x| x.clone().into()).collect())
            },
            None => Err(Error::last_os_error()),
        }
    }

    fn handle_sending_req(&self, hash: &[u8]) -> Result<Vec<u8>, Error> {
        match self.alloc_mem_for_chunk(hash) {
            Ok(_) => {
                let message = Message::new(SENDING_ACKNOWLEDGEMENT_TYPE, hash).into();
                Ok(message)
            },
            Err(_) => Err(Error::last_os_error()),
        }
    }

    fn alloc_mem_for_chunk(&self, hash: &[u8]) -> Result<(), Error> {
        match self.storage.borrow_mut().insert(hash.to_vec(), vec![]) {
            Some(_) => Ok(()),
            None => Err(Error::last_os_error()),
        }
    }

    fn handle_content_filled(&self, hash: &[u8], data: &[u8]) -> Result<(), Error> {
        match self.storage.borrow_mut().get(hash) {
            Some(mut d) => Ok(d.append(&mut data.to_vec())),
            None => Err(Error::last_os_error()),
        }
    }
}