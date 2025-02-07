#![allow(unused_variables)]
#![allow(dead_code)]

mod args;
mod parts;
mod socket;

use std::path::PathBuf;

use crate::parts::{FileParts, Parts};
use args::{load_args, Action};

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
    let (mut parts, mut recovery) = FileParts::from_file(&file).await.unwrap();
    let password = "Hello world"; // Should ask for password properly
    parts.encrypt(password).await.unwrap();
    recovery.encrypt(password).await.unwrap();

    parts.send_into_domain().await.unwrap();
    recovery.send_into_domain().await.unwrap();

    parts.save_metadata(&file, recovery).await.unwrap();
    Ok(())
}

async fn recv_file(file: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let (mut parts, mut recovery) = FileParts::load_from_metadata(&file).await.unwrap();
    parts.recv_from_domain().await.unwrap();
    if let Err(_) = parts.len() {
        recovery.recv_from_domain().await.unwrap();
    }

    let password = "Hello world";
    parts.decrypt(password).await.unwrap();
    recovery.decrypt(password).await.unwrap();

    parts.restore_as_file(&file, recovery).await.unwrap();
    Ok(())
}
