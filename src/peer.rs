use std::io::Error;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{mpsc, Arc, Mutex, mpsc::{Receiver, Sender}};
use std::thread::{JoinHandle, spawn};

use local_ip_address::{local_broadcast_ip, local_ip};

use crate::message::Message;
use crate::server::BroadcastUdpServer;

pub struct BroadcastUdpPeer {
    socket: Arc<Mutex<UdpSocket>>,
    server: Arc<Mutex<BroadcastUdpServer>>,
    to_client_sender: Sender<(Message, SocketAddr)>,
}

const MAX_DATAGRAM_SIZE: usize = 65_507;

impl BroadcastUdpPeer {
    pub fn new() -> Result<(BroadcastUdpPeer, Receiver<(Message, SocketAddr)>), Error> {
        let addr = SocketAddr::new(local_ip().unwrap(), 62092);
        let socket = Arc::new(Mutex::new(UdpSocket::bind(addr)?));
        let server = Arc::new(Mutex::new(BroadcastUdpServer::new()));
        let (to_client_sender, to_client_receiver) = mpsc::channel::<(Message, SocketAddr)>();

        Ok((BroadcastUdpPeer {
            socket,
            server,
            to_client_sender,
        }, to_client_receiver))
    }

    pub fn listen(&self, num_threads: usize) -> Vec<JoinHandle<()>> {
        let mut handles = vec![];

        for _ in 0..num_threads {
            let server = Arc::clone(&self.server);
            let socket = Arc::clone(&self.socket);
            let sender = self.to_client_sender.clone();
            let mut buf = [0u8; MAX_DATAGRAM_SIZE];

            let handle = spawn(move || {
                loop {
                    match socket.lock().unwrap().recv_from(&mut buf) {
                        Ok((s, a)) => {
                            let message = Message::from(buf[..s].to_vec());
                            match message {
                                Message::SendingReq(h) => {
                                    let answer = server.lock().unwrap().handle_sending_req(&h).unwrap();
                                    socket.lock().unwrap().send_to(Into::<Vec<_>>::into(answer).as_slice(), a).unwrap();
                                },
                                Message::RetrievingReq(h) => {
                                    let chunks = server.lock().unwrap().handle_retrieving_req(&h).unwrap();
                                    for chunk in chunks {
                                        socket.lock().unwrap().send_to(Into::<Vec<_>>::into(chunk).as_slice(), a).unwrap();
                                    }
                                },
                                _ => sender.send((message, a)).unwrap(),
                            };
                        },
                        Err(e) => panic!("{}", e.to_string()),
                    };
                }
                ()
            });
            handles.push(handle);
        }
        handles
    }

    pub fn send_req(&self, data: &[u8]) -> Result<(), Error> {
        let broadcast = SocketAddr::new(local_broadcast_ip().unwrap(), 62092);
        self.socket.lock().unwrap().send_to(data, broadcast)?;
        Ok(())
    }

    pub fn send_content(&self, data: &[u8], addr: SocketAddr) -> Result<(), Error> {
        self.socket.lock().unwrap().send_to(data, addr)?;
        Ok(())
    }
}