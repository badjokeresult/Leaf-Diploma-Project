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
use crate::lib::storage::BroadcastUdpServerStorage;

#[derive(Clone)]
pub struct BroadcastUdpServer {
    socket: Arc<UdpSocket>,
    storage: AtomicRefCell<BroadcastUdpServerStorage>,
    sender: Sender<(Message, SocketAddr)>,
    broadcast_addr: SocketAddr,
}

impl BroadcastUdpServer {
    pub async fn new(addr: &str, broadcast_addr: &str, sender: Sender<(Message, SocketAddr)>) -> BroadcastUdpServer {
        let socket = UdpBuilder::new_v4().unwrap()
            .reuse_address(true).unwrap()
            .reuse_port(true).unwrap()
            .bind(addr).unwrap();
        socket.set_broadcast(true).unwrap();
        socket.set_write_timeout(Some(Duration::new(5, 0))).unwrap();
        socket.set_read_timeout(Some(Duration::new(5, 0))).unwrap();

        let socket = Arc::new(UdpSocket::from_std(socket).unwrap());

        let storage = AtomicRefCell::new(BroadcastUdpServerStorage::new(
            PathBuf::new().join(WORKING_FOLDER_NAME).join(DEFAULT_CHUNKS_STOR_FOLDER),
        ));

        let broadcast_addr = SocketAddr::from_str(broadcast_addr).unwrap();

        BroadcastUdpServer {
            socket,
            storage,
            sender,
            broadcast_addr,
        }
    }

    async fn restore_storage_from_file(stor_file_path: &PathBuf) -> Result<HashMap<Vec<u8>, Vec<u8>>, Error> {
        let content = fs::read(stor_file_path).await?;
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
        match self.storage.borrow().retrieve(hash) {
            Ok(c) => {
                let mut content = vec![Message::RetrievingAck(hash.to_vec(), None)];
                content.append(&mut Message::new_with_data(RETRIEVING_ACKNOWLEDGEMENT_TYPE, hash, c));
                Ok(content.par_iter().map(|x| x.clone().into()).collect())
            },
            Err(_) => Err(Error::last_os_error()),
        }
    }

    fn handle_sending_req(&self, hash: &[u8]) -> Result<Vec<u8>, Error> {
        Ok(Message::SendingAck(hash.to_vec()).into())
    }

    fn handle_content_filled(&self, hash: &[u8], data: &[u8]) -> Result<(), Error> {
        match self.storage.borrow_mut().retrieve(hash) {
            Some(d) => Ok(d.clone().append(&mut data.to_vec())),
            None => Err(Error::last_os_error()),
        }
    }

    pub async fn send_chunk(&self, hash: &[u8], chunk: &[u8], receiver: &mut Receiver<(Message, SocketAddr)>) -> Result<(), Error> {
        let req: Vec<u8> = Message::SendingReq(hash.to_vec()).into();
        self.socket.send_to(&req, self.broadcast_addr).await?;

        if let Some((m, a)) = receiver.recv().await {
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

        if let Some((m, _)) = receiver.recv().await {
            if let Message::RetrievingAck(_, _) = m {
                let mut result = vec![];
                while let Some((m, _)) = receiver.recv().await {
                    if let Message::ContentFilled(_, mut d) = m {
                        result.append(&mut d);
                    }
                }
                return Ok(result);
            }
        }
        Err(Error::last_os_error())
    }
}