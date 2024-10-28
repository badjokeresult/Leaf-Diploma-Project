use std::io::Error;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::{mpsc, mpsc::{Receiver, Sender}, Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use net2::UdpBuilder;
use net2::unix::UnixUdpBuilderExt;
use crate::message::Message;

pub struct BroadcastUdpServer {
    socket: UdpSocket,
    to_client_sender: Sender<(Message, SocketAddr)>,
}

const MAX_DATAGRAM_SIZE: usize = 65_507;

impl BroadcastUdpServer {
    pub fn new(local_ip: IpAddr, local_broadcast: IpAddr) -> Result<(BroadcastUdpServer, Receiver<(Message, SocketAddr)>), Error> {
        let addr = SocketAddr::new(local_ip, 62092);
        let socket = UdpBuilder::new_v4()?.reuse_address(true)?.reuse_port(true)?.bind(addr)?;
        socket.set_broadcast(true)?;
        socket.set_read_timeout(Some(Duration::new(5, 0)))?;
        socket.set_write_timeout(Some(Duration::new(3, 0)))?;
        let server = Arc::new(Mutex::new(BroadcastUdpServer::new()));
        let (to_client_sender, to_client_receiver) = mpsc::channel::<(Message, SocketAddr)>();

        Ok((BroadcastUdpServer {
            socket,
            server,
            to_client_sender,
            broadcast_addr: local_broadcast,
        }, to_client_receiver))
    }

    pub fn listen(&self) -> JoinHandle<()> {
        let server = self.server.clone();
        let to_client_sender = self.to_client_sender.clone();
        let mut buf = [0u8; MAX_DATAGRAM_SIZE];

        thread::spawn(move || {
            loop {
                match self.socket.recv_from(&mut buf) {
                    Ok((s, a)) => {
                        let message = Message::from(buf[..s].to_vec());
                        match message {
                            Message::SendingReq(h) => {
                                let answer = server.lock().unwrap().handle_sending_req(&h).unwrap();
                                self.socket.send_to(Into::<Vec<_>>::into(answer).as_slice(), a).unwrap();
                            },
                            Message::RetrievingReq(h) => {
                                let chunks = server.lock().unwrap().handle_retrieving_req(&h).unwrap();
                                for chunk in chunks {
                                    self.socket.send_to(Into::<Vec<_>>::into(chunk).as_slice(), a).unwrap();
                                }
                            },
                            Message::ContentFilled(h, d) => match server.lock().unwrap().handle_content_filled(&h, &d) {
                                Ok(_) => {},
                                Err(e) => eprintln!("{}", e.to_string()),
                            },
                            _ => to_client_sender.send((message, a)).unwrap(),
                        };
                    },
                    Err(e) => panic!("{}", e.to_string()),
                };
            }
        })
    }

    pub fn send_req(&self, data: &[u8]) -> Result<(), Error> {
        let broadcast = SocketAddr::new(self.broadcast_addr, 62092);
        self.socket.send_to(data, broadcast)?;
        Ok(())
    }

    pub fn send_content(&self, data: &[u8], addr: SocketAddr) -> Result<(), Error> {
        self.socket.send_to(data, addr)?;
        Ok(())
    }
}
