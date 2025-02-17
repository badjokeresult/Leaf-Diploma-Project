mod socket;
mod stor;

use std::path::PathBuf;

use tokio::fs;
use tokio::sync::mpsc::{channel, Receiver};

use common::Message;

use socket::{Packet, Socket};
use stor::{ServerStorage, UdpServerStorage};

use consts::*;
use errors::*;

mod consts {
    #[cfg(windows)]
    pub const APPS_DIR_ABS_PATH: &str = "APPDATA";

    #[cfg(not(windows))]
    pub const APPS_DIR_ABS_PATH: &str = "/var/local";

    pub const APP_DIR: &str = "leaf";
    pub const CHUNKS_DIR: &str = "chunks";
}

async fn process_packet(
    packet: Packet,
    storage: &UdpServerStorage,
    socket: &Socket,
) -> Result<(), Box<dyn std::error::Error>> {
    let (data, addr) = packet.deconstruct();
    let message = Message::from_bytes(data)?;
    match message.clone() {
        Message::SendingReq(h) => {
            let ack = Message::SendingAck(h).into_bytes()?;
            let packet = Packet::new(ack, addr);
            socket.send(packet).await?;
            Ok(())
        }
        Message::RetrievingReq(h) => {
            if let Ok(d) = storage.get(&h).await {
                let message = Message::ContentFilled(h.clone(), d).into_bytes()?;
                let packet = Packet::new(message, addr);
                socket.send(packet).await?;
            }
            Err(Box::new(NoHashError(h)))
        }
        Message::ContentFilled(h, d) => {
            storage.save(&h, &d).await?;
            Ok(())
        }
        _ => Err(Box::new(InvalidMessageError)),
    }
}

async fn packet_handler(mut rx: Receiver<Packet>, storage: &UdpServerStorage, socket: &Socket) {
    while let Some(p) = rx.recv().await {
        if let Err(e) = process_packet(p, storage, socket).await {
            eprintln!("{}", e.to_string());
        };
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket = Socket::new().await?;

    let (tx, rx) = channel(100);

    #[cfg(windows)]
    let base_path = PathBuf::from(std::env::var(APPS_DIR_ABS_PATH)?);

    #[cfg(not(windows))]
    let base_path = PathBuf::from(APPS_DIR_ABS_PATH);

    let path = base_path.join(APP_DIR).join(CHUNKS_DIR);
    fs::create_dir_all(&path).await?;

    let storage = UdpServerStorage::new(path);

    let socket_clone = socket.clone();
    tokio::spawn(async move {
        packet_handler(rx, &storage, &socket_clone).await;
    });

    loop {
        socket.recv(&tx).await;
    }
}

mod errors {
    use std::error::Error;
    use std::fmt;

    #[derive(Debug, Clone)]
    pub struct NoFreeSpaceError;

    impl fmt::Display for NoFreeSpaceError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "No free space left for keeping data")
        }
    }

    impl Error for NoFreeSpaceError {}

    #[derive(Debug, Clone)]
    pub struct NoHashError(pub String);

    impl fmt::Display for NoHashError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "No hash {} was found", self.0)
        }
    }

    impl Error for NoHashError {}

    #[derive(Debug, Clone)]
    pub struct InvalidMessageError;

    impl fmt::Display for InvalidMessageError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Got invalid message")
        }
    }

    impl Error for InvalidMessageError {}
}
