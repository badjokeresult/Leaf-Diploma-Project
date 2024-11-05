use std::collections::VecDeque;
use std::io::Error;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::{TcpListener, UdpSocket};

use rayon::prelude::*;

use net2::{TcpListenerExt, UdpBuilder};
use net2::unix::{UnixTcpBuilderExt, UnixUdpBuilderExt};

use atomic_refcell::AtomicRefCell;
use tokio::io::AsyncReadExt;

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
    udp_socket: Arc<UdpSocket>,
    tcp_listener: Arc<TcpListener>,
    storage: AtomicRefCell<BroadcastUdpServerStorage>,
    client_queue: AtomicRefCell<VecDeque<(Message, SocketAddr)>>,
    broadcast_addr: SocketAddr,
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

        let tcp_listener = Arc::new(TcpListener::bind(LOCAL_ADDR).await.unwrap());

        let storage = AtomicRefCell::new(BroadcastUdpServerStorage::new(
            chunks_folder,
        ));

        let client_queue = AtomicRefCell::new(VecDeque::new());

        let broadcast_addr = SocketAddr::from_str(BROADCAST_ADDR).unwrap();

        BroadcastUdpServer {
            udp_socket,
            tcp_listener,
            storage,
            client_queue,
            broadcast_addr,
        }
    }

    pub async fn listen_udp(&self) {
        let mut buf = [0u8; MAX_DATAGRAM_SIZE];
        loop {
            let (sz, addr) = self.udp_socket.recv_from(&mut buf).await.unwrap();
            let message = Message::from(buf[..sz].to_vec());
            match message.clone() {
                Message::RetrievingReq(h) => {
                    self.handle_retrieving_req(&h, addr).await.unwrap();
                },
                Message::SendingReq(h) => {
                    self.handle_sending_req(&h, addr).await.unwrap();
                },
                _ => eprintln!("Invalid message received"),
            };
        }
    }

    pub async fn listen_tcp(&self) {
        let mut buf = [0u8; MAX_DATAGRAM_SIZE];
        loop {
            let (mut socket, _) = self.tcp_listener.accept().await.unwrap();
            let sz = socket.read(&mut buf).await.unwrap();
            let message = Message::from(buf[..sz].to_vec());
            if let Message::ContentFilled(h, d) = message {
                self.storage.borrow_mut().add(&h, &d).await.unwrap();
            }
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
        for message in &messages {
            self.udp_socket.send_to(message, addr).await?;
        }
        Ok(())
    }

    async fn handle_sending_req(&self, hash: &[u8], addr: SocketAddr) -> Result<(), Error> {
        let message: Vec<u8> = Message::SendingAck(hash.to_vec()).into();
        self.udp_socket.send_to(&message, addr).await?;
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