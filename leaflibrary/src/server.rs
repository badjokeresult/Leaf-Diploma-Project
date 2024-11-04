use std::collections::VecDeque;
use std::io::Error;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::UdpSocket;

use rayon::prelude::*;

use net2::UdpBuilder;
use net2::unix::UnixUdpBuilderExt;

use atomic_refcell::AtomicRefCell;

use crate::message::Message;

use consts::*;
use crate::storage::BroadcastUdpServerStorage;

mod consts {
    pub const LOCAL_ADDR: &str = "0.0.0.0:62092";
    pub const BROADCAST_ADDR: &str = "255.255.255.255:62092";
    pub const MAX_DATAGRAM_SIZE: usize = 65507;
}

#[derive(Clone)]
pub struct BroadcastUdpServer {
    socket: Arc<UdpSocket>,
    storage: AtomicRefCell<BroadcastUdpServerStorage>,
    client_queue: AtomicRefCell<VecDeque<(Message, SocketAddr)>>,
    broadcast_addr: SocketAddr,
}

impl BroadcastUdpServer {
    pub async fn new(chunks_folder: &PathBuf) -> BroadcastUdpServer {
        let socket = UdpBuilder::new_v4().unwrap()
            .reuse_address(true).unwrap()
            .reuse_port(true).unwrap()
            .bind(LOCAL_ADDR).unwrap();
        socket.set_broadcast(true).unwrap();
        socket.set_write_timeout(Some(Duration::new(5, 0))).unwrap();
        socket.set_read_timeout(Some(Duration::new(5, 0))).unwrap();

        let socket = Arc::new(UdpSocket::from_std(socket).unwrap());

        let storage = AtomicRefCell::new(BroadcastUdpServerStorage::new(
            chunks_folder,
        ));

        let client_queue = AtomicRefCell::new(VecDeque::new());

        let broadcast_addr = SocketAddr::from_str(BROADCAST_ADDR).unwrap();

        BroadcastUdpServer {
            socket,
            storage,
            client_queue,
            broadcast_addr,
        }
    }

    pub async fn listen(&self) {
        let mut buf = [0u8; MAX_DATAGRAM_SIZE];
        loop {
            let (sz, addr) = self.socket.recv_from(&mut buf).await.unwrap();
            let message = Message::from(buf[..sz].to_vec());
            match message.clone() {
                Message::RetrievingReq(h) => {
                    self.handle_retrieving_req(&h, addr).await.unwrap();
                },
                Message::SendingReq(h) => {
                    println!("RECEIVED SENDING REQ!");
                    self.handle_sending_req(&h, addr).await.unwrap();
                },
                Message::ContentFilled(h, d, c) => {
                    println!("RECEIVED CONTENT FILLED!");
                    self.handle_content_filled(&h, &d, c, addr).await.unwrap();
                },
                Message::SendingAck(_) | Message::RetrievingAck(_) => {
                    self.handle_ack(message, addr).await.unwrap();
                },
                Message::Empty(h) => {
                    println!("RECEIVED EMPTY!");
                    self.handle_empty_message(&h).await.unwrap()
                },
            };
        }
    }

    async fn handle_retrieving_req(&self, hash: &[u8], addr: SocketAddr) -> Result<(), Error> {
        let messages = match self.storage.borrow().retrieve(hash).await {
            Ok(c) => {
                let content = Message::new_with_data(hash, &c, false);
                content.par_iter().map(|x| x.clone().into()).collect::<Vec<Vec<_>>>()
            },
            Err(_) => return Err(Error::last_os_error()),
        };
        for message in &messages {
            self.socket.send_to(message, addr).await.unwrap();
        }
        Ok(())
    }

    async fn handle_sending_req(&self, hash: &[u8], addr: SocketAddr) -> Result<(), Error> {
        let message: Vec<u8> = Message::SendingAck(hash.to_vec()).into();
        self.socket.send_to(&message, addr).await.unwrap();
        Ok(())
    }

    async fn handle_ack(&self, message: Message, addr: SocketAddr) -> Result<(), Error> {
        self.client_queue.borrow_mut().push_back((message, addr));
        Ok(())
    }

    async fn handle_content_filled(&self, hash: &[u8], data: &[u8], for_client: bool, addr: SocketAddr) -> Result<(), Error> {
        match for_client {
            false => self.storage.borrow_mut().add(hash, data).await?,
            true => self.client_queue.borrow_mut().push_back((Message::ContentFilled(hash.to_vec(), data.to_vec(), true), addr))
        }
        Ok(())
    }

    async fn handle_empty_message(&self, hash: &[u8]) -> Result<(), Error> {
        self.storage.borrow_mut().finalize(hash).await?;
        Ok(())
    }

    pub async fn send_chunk(&self, hash: &[u8], chunk: &[u8]) -> Result<(), Error> {
        let req: Vec<u8> = Message::SendingReq(hash.to_vec()).into();
        self.socket.send_to(&req, self.broadcast_addr).await?;

        if let Some((m, a)) = self.client_queue.borrow_mut().pop_front() {
            if let Message::SendingAck(_) = m {
                let content: Vec<Vec<u8>> = Message::new_with_data(hash, chunk, true)
                    .par_iter().map(|x| x.clone().into())
                    .collect();
                for part in content {
                    self.socket.send_to(&part, a).await?;
                };
            }
        }

        Ok(())
    }

    pub async fn recv_chunk(&self, hash: &[u8]) -> Result<Vec<u8>, Error> {
        let req: Vec<u8> = Message::RetrievingReq(hash.to_vec()).into();
        self.socket.send_to(&req, self.broadcast_addr).await?;

        if let Some((m, _)) = self.client_queue.borrow_mut().pop_front() {
            if let Message::RetrievingAck(_) = m {
                let mut result = vec![];
                if let Some((m, _)) = self.client_queue.borrow_mut().pop_front() {
                    if let Message::ContentFilled(_, mut d, c) = m {
                        if c {
                            result.append(&mut d);
                        }
                    }
                }
                return Ok(result);
            }
        }
        Err(Error::last_os_error())
    }

    pub async fn shutdown(&self) {
        self.storage.borrow_mut().shutdown().await;
    }
}