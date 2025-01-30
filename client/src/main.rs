#![allow(unused_variables)]
#![allow(dead_code)]

mod args;
mod parts;
mod hashes;
mod socket;

use std::path::PathBuf;
use tokio::net::UdpSocket;
// use tokio::fs;
// use common::{KuznechikEncryptor, ReedSolomonSecretSharer, SecretSharer};

use args::{load_args, Action};
use common::Message;
use crate::parts::{FileParts, Parts};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = load_args();

    let path = args.get_file();
    match args.get_action() {
        Action::Send => send_file(path).await,
        Action::Receive => recv_file(path).await,
    }
}

async fn send_file(file: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // let mut parts = FileParts::from_file(file).await;
    // // Ask for password here
    // let password = "Helloworld";
    // parts.encrypt(password).await;
    // parts.calc_hashes().await;
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    let message = Message::SendingReq([0u8; 256].to_vec());
    socket.set_broadcast(true)?;
    let data: Vec<u8> = message.into();
    socket.send_to(&data, "255.255.255.255:62092").await?;
    let mut buf = [0u8; 1024];
    while let Ok((_, a)) = socket.recv_from(&mut buf).await {
        println!("OK: {}", a);
    }
    Ok(())
}

async fn recv_file(file: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    let message = Message::RetrievingReq([0u8; 256].to_vec());
    socket.set_broadcast(true)?;
    let data: Vec<u8> = message.into();
    socket.send_to(&data, "255.255.255.255:62092").await?;
    let mut buf = [0u8; 1024];
    while let Ok((_, a)) = socket.recv_from(&mut buf).await {
        println!("OK: {}", a);
    }
    Ok(())
}
