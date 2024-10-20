use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::task;
use client;
use server;
use server::{BroadcastServerPeer, ServerPeer};

#[tokio::main]
async fn main() {
    let handle = task::spawn(async move {
        let server = BroadcastServerPeer::new().await;
        server.listen().await;
        println!("SERVER STARTED");
        return;
    });
    println!("SERVER STARTED");

    let content = tokio::fs::read("test.txt").await.unwrap();
    let hashes = client::send_content(content).await;
    let recv_content = client::recv_content(hashes).await;
    tokio::fs::write("recv_test.txt", &recv_content).await.unwrap();
    handle.await.unwrap();
}