use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::sync::broadcast;
use tokio::time;

#[derive(Clone, Debug)]
pub struct Packet {
    pub data: Vec<u8>,
    pub addr: SocketAddr,
}

impl Packet {
    pub fn new(data: Vec<u8>, addr: SocketAddr) -> Packet {
        Packet { data, addr }
    }

    pub fn deconstruct(self) -> (Vec<u8>, SocketAddr) {
        (self.data, self.addr)
    }
}

#[derive(Clone)]
pub struct Socket {
    socket: Arc<UdpSocket>,
    sender: broadcast::Sender<Packet>,
}

impl Socket {
    pub async fn new() -> (Socket, broadcast::Sender<Packet>) {
        let socket = Arc::new(UdpSocket::bind("0.0.0.0:62092").await.unwrap());
        socket.set_broadcast(true).unwrap();

        let (tx, _) = broadcast::channel(100);

        (Socket {
            socket,
            sender: tx.clone(),
        }, tx)
    }

    pub async fn send(&self, packet: Packet) {
        let (data, addr) = packet.deconstruct();
        self.socket.send_to(data.as_slice(), addr).await.unwrap();
    }

    pub async fn recv(&self) {
        let mut buf = [0u8; 4096];
        loop {
            time::sleep(Duration::from_millis(100)).await;
            if let Ok((s, a)) = self.socket.recv_from(&mut buf).await {
                let packet = Packet::new(buf[..s].to_vec(), a);
                self.sender.send(packet).unwrap();
            }
        }
    }
}
