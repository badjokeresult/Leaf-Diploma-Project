use std::collections::HashMap;
use std::io::Error;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::fs;
use rayon::prelude::*;

use net2::UdpBuilder;
use net2::unix::UnixUdpBuilderExt;

use atomic_refcell::AtomicRefCell;

use crate::message::Message;
use crate::consts::*;

#[derive(Clone)]
pub struct BroadcastUdpServer {
    socket: Arc<UdpSocket>,
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

        let socket = UdpBuilder::new_v4().unwrap()
            .reuse_address(true).unwrap()
            .reuse_port(true).unwrap()
            .bind(addr).unwrap();
        socket.set_broadcast(true).unwrap();
        socket.set_write_timeout(Some(Duration::new(5, 0))).unwrap();
        socket.set_read_timeout(Some(Duration::new(5, 0))).unwrap();

        let socket = Arc::new(UdpSocket::from_std(socket).unwrap());

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

    pub async fn listen(&self) {
        let mut buf = [0u8; MAX_DATAGRAM_SIZE];
        loop {
            let (sz, addr) = self.socket.recv_from(&mut buf).await.unwrap();
            let message = Message::from(buf[..sz].to_vec());
            match message.clone() {
                Message::RetrievingReq(h) => {
                    let messages = self.handle_retrieving_req(&h).unwrap();
                    for message in messages {
                        self.socket.send_to(&message, addr).await.unwrap();
                    };
                },
                Message::SendingReq(h) => {
                    let message = self.handle_sending_req(&h).unwrap();
                    self.socket.send_to(&message, addr).await.unwrap();
                },
                Message::ContentFilled(h, d) => {
                    self.handle_content_filled(&h, &d).unwrap();
                },
                Message::SendingAck(_) | Message::RetrievingAck(_, _) => {
                    self.sender.send((message, addr)).await.unwrap();
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
                Ok(content.par_iter().map(|x| x.clone().into()).collect())
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

    pub async fn send_chunk(&self, hash: &[u8], chunk: &[u8], receiver: &mut Receiver<(Message, SocketAddr)>) -> Result<(), Error> {
        let req: Vec<u8> = Message::SendingReq(hash.to_vec()).into();
        self.socket.send_to(&req, self.broadcast_addr).await?;

        if let Ok((m, a)) = receiver.recv().await {
            if let Message::SendingAck(_) = m {
                let content: Vec<Vec<u8>> = Message::new_with_data(CONTENT_FILLED_TYPE, hash, chunk)
                    .par_iter().map(|x| x.clone().into())
                    .collect();
                for part in content {
                    self.socket.send_to(&part, a).await?;
                };
            }
        }

        Ok(())
    }

    pub async fn recv_chunk(&self, hash: &[u8], receiver: &mut Receiver<(Message, SocketAddr)>) -> Result<Vec<u8>, Error> {
        let req: Vec<u8> = Message::RetrievingReq(hash.to_vec()).into();
        self.socket.send_to(&req, self.broadcast_addr).await?;

        if let Ok((m, a)) = receiver.recv().await {
            if let Message::RetrievingAck(_, _) = m {
                let mut result = vec![];
                while let Ok((m, _)) = receiver.recv().await {
                    if let Message::ContentFilled(_, mut d) = m {
                        result.append(&mut d);
                    }
                }
                return Ok(result);
            }
        }
        Err(Error::last_os_error())
    }

    pub async fn shutdown(self) {
        let content = serde_json::to_vec(&self.storage.borrow().clone()).unwrap();
        fs::write(self.stor_file_path, &content).await.unwrap();
    }
}