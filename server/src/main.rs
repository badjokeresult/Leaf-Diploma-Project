mod config;
mod stor;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};

use common::Message;

use config::*;
use stor::ServerStorage;
use stor::UdpServerStorage;

#[tokio::main]
async fn main() {
    let socket = Arc::new(UdpSocket::bind((LOCAL_ADDR, LOCAL_PORT)).await.unwrap());
    let (sock_recv_tx, mut sock_recv_rx) = mpsc::channel(CHAN_SIZE);
    let (worker_recv_tx, mut worker_recv_rx): (
        Sender<(SocketAddr, Vec<u8>)>,
        Receiver<(SocketAddr, Vec<u8>)>,
    ) = mpsc::channel(CHAN_SIZE);

    let socket_clone = socket.clone();
    let mut buf = [0u8; BUF_SIZE];
    tokio::spawn(async move {
        loop {
            if let Ok((sz, addr)) = socket_clone.recv_from(&mut buf).await {
                sock_recv_tx.send((addr, buf[..sz].to_vec())).await.unwrap();
            } else {
                eprintln!("Error receiving message");
            }
            tokio::time::sleep(Duration::from_millis(MILLIS_TIMEOUT)).await;
        }
    });

    let socket_clone1 = socket.clone();
    tokio::spawn(async move {
        loop {
            if let Some((addr, msg)) = worker_recv_rx.recv().await {
                socket_clone1.send_to(&msg, &addr).await.unwrap();
            }
            tokio::time::sleep(Duration::from_millis(MILLIS_TIMEOUT)).await;
        }
    });

    let storage = UdpServerStorage::new(PathBuf::from(STOR_PATH));
    loop {
        if let Some((addr, data)) = sock_recv_rx.recv().await {
            let message = Message::from(data);
            match message.clone() {
                Message::SendingReq(h) => {
                    let data = parse_sending_req(&h);
                    worker_recv_tx.send((addr, data)).await.unwrap();
                }
                Message::RetrievingReq(h) => {
                    let data = parse_retrieving_req(&storage, &h).await;
                    for msg in data {
                        worker_recv_tx.send((addr, msg)).await.unwrap();
                        tokio::time::sleep(Duration::from_millis(MILLIS_TIMEOUT)).await;
                    }
                }
                Message::ContentFilled(h, d) => {
                    parse_content_filled(&storage, &h, &d).await;
                }
                _ => eprintln!("{:?}", message),
            }
        }
        tokio::time::sleep(Duration::from_millis(MILLIS_TIMEOUT)).await;
    }
}

fn parse_sending_req(hash: &[u8]) -> Vec<u8> {
    let answer: Vec<u8> = Message::SendingAck(hash.to_vec()).into();
    answer
}

async fn parse_retrieving_req(storage: &UdpServerStorage, hash: &[u8]) -> Vec<Vec<u8>> {
    let mut messages: Vec<Vec<u8>> = vec![];
    if let Ok(x) = storage.get(hash).await {
        let mut answer = vec![];
        answer.push(Message::RetrievingAck(hash.to_vec()));
        answer.append(&mut Message::new_with_data(hash, &x));
        for msg in answer {
            messages.push(msg.into());
        }
    }
    messages
}

async fn parse_content_filled(storage: &UdpServerStorage, hash: &[u8], data: &[u8]) {
    storage.save(hash, data).await.unwrap();
}
