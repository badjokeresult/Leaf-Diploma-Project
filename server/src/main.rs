use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time;

use common::Message;

use config::Storage;
use stor::ServerStorage;
use socket::{Packet, Socket};

mod config;
mod stor;
mod socket;

async fn process_packet(packet: Packet, storage: &Storage, socket: &Socket) {
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
            storage.save(&h, &d, false).unwrap();
        },
        Message::Empty(h) => {
            storage.save(&h, vec![], true).unwrap();
        },
        _ => {},
    }
}

async fn process_handler(mut rx: mpsc::Receiver<Packet>, storage: &Storage, socket: &Socket) {
    loop {
        if let Some(p) = rx.recv().await {
            process_packet(p, storage, socket).await;
        }
    }
}

#[tokio::main]
async fn main() {
    println!("Hello, world!");
}