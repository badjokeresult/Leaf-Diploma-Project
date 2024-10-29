use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;

use net2::{UdpBuilder, UdpSocketExt};
use net2::unix::UnixUdpBuilderExt;

const DEFAULT_BINDING_ADDRESS: &str = "0.0.0.0:62092";

pub struct BroadcastUdpPeer {
    socket: Arc<UdpSocket>,
}

impl BroadcastUdpPeer {
    pub fn new() -> BroadcastUdpPeer {
        let socket = UdpBuilder::new_v4().unwrap()
            .reuse_address(true).unwrap()
            .reuse_port(true).unwrap()
            .bind(DEFAULT_BINDING_ADDRESS.parse::<SocketAddr>().unwrap()).unwrap();
        socket.set_broadcast(true).unwrap();
        socket.set_read_timeout(Some(Duration::new(5, 0))).unwrap();
        socket.set_write_timeout(Some(Duration::new(5, 0))).unwrap();

        let socket = Arc::new(UdpSocket::from_std(socket).unwrap());

        BroadcastUdpPeer {
            socket,
        }
    }

    pub async fn start(&self, sender: Sender<(Vec<u8>, SocketAddr)>, mut receiver: Receiver<(Vec<u8>, SocketAddr)>, num_threads: usize) -> Vec<JoinHandle<()>> {
        let mut handles = Vec::with_capacity(num_threads - 1);

        for _ in 0..num_threads - 2 {
            let socket = self.socket.clone();
            let sender = sender.clone();
            let mut buf = [0u8; 65507];
            handles.push(tokio::spawn(async move {
                loop {
                    let (sz, addr) = socket.recv_from(&mut buf).await.unwrap();
                    sender.send((buf[..sz].to_vec(), addr)).await.unwrap();
                }
            }));
        }

        let socket = self.socket.clone();
        handles.push(tokio::spawn(async move {
            loop {
                if let Some((d, a)) = receiver.recv().await {
                    socket.send_to(&d, a).await.unwrap();
                };
            };
        }));

        handles
    }
}