#![allow(unused_variables)]
#![allow(dead_code)]

mod args;
mod parts;
mod socket;

use std::path::PathBuf;

use args::{load_args, Action};
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
    let mut parts = FileParts::from_file(file).await.unwrap();
    let password = "Hello world"; // Should ask for password properly
    parts.encrypt(password).await.unwrap();
    parts.send_into_domain().await.unwrap();
    parts.save_metadata().await.unwrap();
    Ok(())
}

async fn recv_file(file: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let mut parts = FileParts::load_from_metadata(file).await.unwrap();
    parts.recv_from_domain().await.unwrap();
    let password = "Hello world";
    parts.decrypt(password).await.unwrap();
    parts.restore_as_file().await.unwrap();
    Ok(())
}
