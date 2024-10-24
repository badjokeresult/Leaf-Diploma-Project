pub mod peer;
pub mod client;
pub mod server;
mod codec;

use std::net::SocketAddr;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use tools::Message;
use crate::peer::Peer;

pub fn start(addr: SocketAddr, num_threads: usize) {

}