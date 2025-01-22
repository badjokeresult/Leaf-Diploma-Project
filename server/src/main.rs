use std::path::PathBuf;
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::time;

use common::Message;

use stor::{ServerStorage, UdpServerStorage};
use socket::{Packet, Socket};

mod stor;
mod socket;

async fn process_packet(packet: Packet, storage: &UdpServerStorage, socket: &Socket) {
    time::sleep(Duration::from_millis(100)).await;
    let addr = packet.addr;
    let message = Message::from(packet.data);
    match message.clone() {
        Message::SendingReq(h) => {
            let curr_size = storage.get_occupied_space().await.unwrap();
            if curr_size < 10 * 1024 * 1024 * 1024 {
                let ack: Vec<u8> = Message::SendingAck(h).into();
                let packet = Packet::new(ack, addr);
                socket.send(packet).await;
            }
        },
        Message::RetrievingReq(h) => {
            if let Ok(d) = storage.get(&h).await {
                let mut messages: Vec<Vec<u8>> = vec![];
                messages.push(Message::RetrievingAck(h.clone()).into());
                let content_messages = Message::new_with_data(&h, &d);
                for msg in content_messages {
                    messages.push(msg.into());
                }
                for message in messages {
                    let packet = Packet::new(message, addr);
                    socket.send(packet).await;
                }
            }
        },
        Message::ContentFilled(h, d) => {
            storage.save(&h, &d, false).await.unwrap();
        },
        Message::Empty(h) => {
            storage.save(&h, &[0u8; 0], true).await.unwrap();
        },
        _ => {},
    }
}

async fn packet_handler(mut rx: broadcast::Receiver<Packet>, storage: &UdpServerStorage, socket: &Socket) {
    loop {
        if let Ok(p) = rx.recv().await {
            process_packet(p, storage, socket).await;
        }
    }
}

#[tokio::main]
async fn main() {
    let (socket, tx) = Socket::new().await;
    let storage = UdpServerStorage::new(PathBuf::from(std::env::var("APPDATA").unwrap().as_str()));

    for _ in 0..4 {
        let rx = tx.subscribe();
        let socket_clone = socket.clone();
        let storage_clone = storage.clone();
        tokio::spawn(async move {
            packet_handler(rx, &storage_clone, &socket_clone).await;
        });
    }

    tokio::spawn(async move {
        socket.recv().await;
    });

    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}