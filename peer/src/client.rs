use std::io::Error;
use std::net::SocketAddr;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use tools::Message;
use crate::peer::Peer;

pub struct Client {
    peer: Peer,
}

impl Client {
    pub fn new(addr: SocketAddr) -> Client {
        let peer = Peer::new(addr);
        Client { peer }
    }
}

impl Client {
    pub fn send_content(&self, content: &[u8]) {

    }
}