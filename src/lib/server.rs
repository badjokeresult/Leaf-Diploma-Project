use std::collections::HashMap;
use std::fs;
use std::io::Error;
use std::net::{SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use net2::UdpBuilder;
use net2::unix::UnixUdpBuilderExt;

use atomic_refcell::AtomicRefCell;

use crate::message::Message;
use crate::consts::*;

#[derive(Clone)]
pub struct BroadcastUdpServer {
    socket: Arc<Mutex<UdpSocket>>,
    storage: AtomicRefCell<HashMap<Vec<u8>, Vec<u8>>>,
    sender: Sender<(Message, SocketAddr)>,
    broadcast_addr: SocketAddr,
    stor_file_path: PathBuf,
}

impl BroadcastUdpServer {
    pub fn new(addr: &str, broadcast_addr: &str, sender: Sender<(Message, SocketAddr)>) -> BroadcastUdpServer {
        let stor_file_path = dirs::home_dir().unwrap()
            .join(WORKING_FOLDER_NAME)
            .join(DEFAULT_STOR_FILE_NAME);

        let socket = Arc::new(Mutex::new(UdpBuilder::new_v4().unwrap()
            .reuse_address(true).unwrap()
            .reuse_port(true).unwrap()
            .bind(addr).unwrap()));
        socket.lock().unwrap().set_broadcast(true).unwrap();
        socket.lock().unwrap().set_write_timeout(Some(Duration::new(5, 0))).unwrap();
        socket.lock().unwrap().set_read_timeout(Some(Duration::new(5, 0))).unwrap();

        let storage = AtomicRefCell::new(Self::restore_storage_from_file(&stor_file_path).unwrap_or_else(|_| HashMap::new()));

        let broadcast_addr = SocketAddr::from_str(broadcast_addr).unwrap();

        BroadcastUdpServer {
            socket,
            storage,
            sender,
            broadcast_addr,
            stor_file_path,
        }
    }

    fn restore_storage_from_file(stor_file_path: &PathBuf) -> Result<HashMap<Vec<u8>, Vec<u8>>, Error> {
        let content = fs::read(stor_file_path)?;
        let storage: HashMap<Vec<u8>, Vec<u8>> = serde_json::from_slice(&content)?;
        Ok(storage)
    }

    pub fn listen(&self) {
        let mut buf = [0u8; MAX_DATAGRAM_SIZE];
        loop {
            let (sz, addr) = self.socket.lock().unwrap().recv_from(&mut buf).unwrap();
            let message = Message::from(buf[..sz].to_vec());
            match message.clone() {
                Message::RetrievingReq(h) => {
                    let messages = self.handle_retrieving_req(&h).unwrap();
                    for message in messages {
                        self.socket.lock().unwrap().send_to(&message, addr).unwrap();
                    };
                },
                Message::SendingReq(h) => {
                    let message = self.handle_sending_req(&h).unwrap();
                    self.socket.lock().unwrap().send_to(&message, addr).unwrap();
                },
                Message::ContentFilled(h, d) => {
                    self.handle_content_filled(&h, &d).unwrap();
                },
                Message::SendingAck(_) | Message::RetrievingAck(_, _) => {
                    self.sender.send((message, addr)).unwrap();
                }
                _ => continue,
            };
        }
    }

    fn handle_retrieving_req(&self, hash: &[u8]) -> Result<Vec<Vec<u8>>, Error> {
        match self.storage.borrow().get(hash) {
            Some(c) => {
                let mut content = vec![Message::RetrievingAck(hash.to_vec(), None)];
                content.append(&mut Message::new_with_data(RETRIEVING_ACKNOWLEDGEMENT_TYPE, hash, c));
                Ok(content.iter().map(|x| x.clone().into()).collect())
            },
            None => Err(Error::last_os_error()),
        }
    }

    fn handle_sending_req(&self, hash: &[u8]) -> Result<Vec<u8>, Error> {
        match self.alloc_mem_for_chunk(hash) {
            Ok(_) => {
                let message = Message::SendingAck(hash.to_vec()).into();
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
            Some(d) => Ok(d.clone().append(&mut data.to_vec())),
            None => Err(Error::last_os_error()),
        }
    }

    pub fn send_chunk(&self, hash: &[u8], chunk: &[u8], receiver: &Receiver<(Message, SocketAddr)>) -> Result<(), Error> {
        let req: Vec<u8> = Message::SendingReq(hash.to_vec()).into();
        self.socket.lock().unwrap().send_to(&req, self.broadcast_addr)?;

        if let Ok((m, a)) = receiver.recv() {
            if let Message::SendingAck(_) = m {
                let content: Vec<Vec<u8>> = Message::new_with_data(CONTENT_FILLED_TYPE, hash, chunk)
                    .iter().map(|x| x.clone().into())
                    .collect();
                for part in content {
                    self.socket.lock().unwrap().send_to(&part, a)?;
                };
            }
        }

        Ok(())
    }

    pub fn recv_chunk(&self, hash: &[u8], receiver: &Receiver<(Message, SocketAddr)>) -> Result<Vec<u8>, Error> {
        let req: Vec<u8> = Message::RetrievingReq(hash.to_vec()).into();
        self.socket.lock().unwrap().send_to(&req, self.broadcast_addr)?;

        if let Ok((m, a)) = receiver.recv() {
            if let Message::RetrievingAck(_, _) = m {
                let mut result = vec![];
                while let Ok((m, _)) = receiver.recv() {
                    if let Message::ContentFilled(_, mut d) = m {
                        result.append(&mut d);
                    }
                }
                return Ok(result);
            }
        }
        Err(Error::last_os_error())
    }

    pub fn shutdown(self) {
        let content = serde_json::to_vec(&self.storage.borrow().clone()).unwrap();
        fs::write(self.stor_file_path, &content).unwrap();
    }
}