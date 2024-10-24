use std::net::UdpSocket;
use std::sync::{Arc, Mutex};

pub struct UdpClient {
    socket: Arc<Mutex<UdpSocket>>,
}

impl