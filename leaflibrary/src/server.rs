use std::io::Error;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::UdpSocket   ;

use rayon::prelude::*;

use net2::UdpBuilder;
use net2::unix::UnixUdpBuilderExt;

use atomic_refcell::AtomicRefCell;

use tokio::io::AsyncReadExt;

use consts::*;
use crate::message::Message;
use crate::storage::BroadcastUdpServerStorage;

mod consts {
    pub const LOCAL_ADDR: &str = "0.0.0.0:62092";
    pub const MAX_DATAGRAM_SIZE: usize = 65507;
}

#[derive(Clone)]
pub struct BroadcastUdpServer {
    udp_socket: Arc<UdpSocket>,
    storage: AtomicRefCell<BroadcastUdpServerStorage>,
}

impl BroadcastUdpServer {
    pub async fn new(chunks_folder: &PathBuf) -> BroadcastUdpServer {
        let udp_socket = UdpBuilder::new_v4().unwrap()
            .reuse_address(true).unwrap()
            .reuse_port(true).unwrap()
            .bind(LOCAL_ADDR).unwrap();
        udp_socket.set_broadcast(true).unwrap();
        udp_socket.set_write_timeout(Some(Duration::new(5, 0))).unwrap();
        udp_socket.set_read_timeout(Some(Duration::new(5, 0))).unwrap();

        let udp_socket = Arc::new(UdpSocket::from_std(udp_socket).unwrap());

        let storage = AtomicRefCell::new(BroadcastUdpServerStorage::new(
            chunks_folder,
        ).await);

        BroadcastUdpServer {
            udp_socket,
            storage,
        }
    }

    pub async fn listen(&self) {
        loop {
            let mut buf = [0u8; MAX_DATAGRAM_SIZE];
            let (sz, addr) = self.udp_socket.recv_from(&mut buf).await.unwrap();
            let message = Message::from(buf[..sz].to_vec());
            match message.clone() {
                Message::RetrievingReq(h) => {
                    self.handle_retrieving_req(&h, addr).await.unwrap();
                },
                Message::SendingReq(h) => {
                    println!("RECEIVED SENDING REQ FROM {}", addr);
                    self.handle_sending_req(&h, addr).await.unwrap();
                },
                Message::ContentFilled(h, d) => {
                    println!("RECEIVED CONTENT FILLED FROM {}", addr);
                    self.handle_content_filled(&h, &d).await.unwrap();
                }
                _ => eprintln!("Invalid message received"),
            };
        }
    }

    async fn handle_retrieving_req(&self, hash: &[u8], addr: SocketAddr) -> Result<(), Error> {
        let messages = match self.storage.borrow().retrieve(hash).await {
            Ok(c) => {
                let content = Message::new_with_data(hash, &c);
                content.par_iter().map(|x| x.clone().into()).collect::<Vec<Vec<_>>>()
            },
            Err(_) => return Err(Error::last_os_error()),
        };
        let ack: Vec<u8> = Message::RetrievingAck(hash.to_vec()).into();
        self.udp_socket.send_to(&ack, addr).await?;
        for message in &messages {
            self.udp_socket.send_to(message, addr).await?;
        }
        Ok(())
    }

    async fn handle_sending_req(&self, hash: &[u8], addr: SocketAddr) -> Result<(), Error> {
        let message: Vec<u8> = Message::SendingAck(hash.to_vec()).into();
        self.udp_socket.send_to(&message, addr).await?;
        println!("SENT SENDING ACK TO {}", addr);
        Ok(())
    }

    async fn handle_content_filled(&self, hash: &[u8], data: &[u8]) -> Result<(), Error> {
        self.storage.borrow_mut().add(hash, data).await?;
        Ok(())
    }

    pub async fn shutdown(&self) {
        self.storage.borrow_mut().shutdown().await;
    }
}