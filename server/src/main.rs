use common::Message;
use socket::{Packet, Socket};
use std::path::PathBuf;
use std::time::Duration;
use stor::{ServerStorage, UdpServerStorage};
use tokio::fs;
use tokio::sync::broadcast;
use tokio::time;

mod socket;
mod stor;

async fn process_packet(packet: Packet, storage: &UdpServerStorage, socket: &Socket) {
    time::sleep(Duration::from_millis(100)).await;
    let addr = packet.addr;
    let message = Message::from(packet.data);
    match message.clone() {
        Message::SendingReq(h) => {
            if storage.can_save().await {
                let ack: Vec<u8> = Message::SendingAck(h).into();
                let packet = Packet::new(ack, addr);
                socket.send(packet).await;
            }
        }
        Message::RetrievingReq(h) => {
            if let Ok(d) = storage.get(&h).await {
                let message: Vec<u8> = Message::ContentFilled(h, d).into();
                let packet = Packet::new(message, addr);
                socket.send(packet).await;
            }
        }
        Message::ContentFilled(h, d) => {
            println!("Received data with hash: {}", h); // Логирование хэш-суммы
            storage.save(&h, &d).await.unwrap();
        }
        _ => {}
    }
}

async fn packet_handler(
    mut rx: broadcast::Receiver<Packet>,
    storage: &UdpServerStorage,
    socket: &Socket,
) {
    while let Ok(p) = rx.recv().await {
        process_packet(p, storage, socket).await;
    }
}

#[tokio::main]
async fn main() {
    let (socket, tx) = Socket::new().await;

    #[cfg(windows)]
    let base_path = PathBuf::from(std::env::var("APPDATA").unwrap());

    #[cfg(not(windows))]
    let base_path = PathBuf::from("/var/local");

    let path = base_path.join("leaf").join("chunks");
    fs::create_dir_all(&path).await.unwrap();

    let storage = UdpServerStorage::new(path);

    for _ in 0..4 {
        let rx = tx.subscribe();
        let socket_clone = socket.clone();
        let storage_clone = storage.clone();
        tokio::spawn(async move {
            packet_handler(rx, &storage_clone, &socket_clone).await;
        });
    }

    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
        socket.recv().await;
    }
}
