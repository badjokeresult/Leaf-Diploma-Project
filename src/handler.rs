use std::cell::RefCell;
use std::io::Error;
use std::net::SocketAddr;

use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::message::consts::*;
use crate::message::Message;
use crate::peer::BroadcastUdpPeer;

pub struct BroadcastUdpHandler {
    peer: BroadcastUdpPeer,
    peer_sender: Sender<(Vec<u8>, SocketAddr)>,
    peer_receiver: RefCell<Receiver<(Vec<u8>, SocketAddr)>>,
}

impl BroadcastUdpHandler {
    pub async fn new(num_threads: usize) -> BroadcastUdpHandler {
        let (sender, receiver) = channel::<(Vec<u8>, SocketAddr)>(1024);
        let (sender1, receiver1) = channel::<(Vec<u8>, SocketAddr)>(1024);
        let peer = BroadcastUdpPeer::new();
        peer.start(sender, receiver1, num_threads).await;

        BroadcastUdpHandler {
            peer,
            peer_sender: sender1,
            peer_receiver: RefCell::new(receiver),
        }
    }

    pub async fn send(&self, msg_type: u8, hash: &[u8], data: &[u8], addr: SocketAddr) {
        let message = Message::new(msg_type, hash).into();
        self.peer_sender.send((message, addr)).await.unwrap();
        let (_, addr) = self.recv_message(SENDING_ACKNOWLEDGEMENT_TYPE, hash).await.unwrap();
        let messages = Message::new_with_data(CONTENT_FILLED_TYPE, hash, data.to_vec())
            .iter().map(|x| <Message as Into<Vec<u8>>>::into(x.clone())).collect();
        for message in messages {
            self.peer_sender.send((message, addr)).await.unwrap();
        }
    }

    async fn recv_message(&self, msg_type: u8, hash: &[u8]) -> Result<(Vec<u8>, SocketAddr), Error> {
        while let Some((d, a)) = self.peer_receiver.borrow_mut().recv().await {
            let message = Message::from(d);
            match message.clone() {
                Message::SendingAck(h) => {
                    if h.eq(hash) && msg_type == SENDING_ACKNOWLEDGEMENT_TYPE {
                        return Ok((vec![], a));
                    }
                },
                Message::RetrievingAck(h, d) => {
                    if h.eq(hash) && msg_type == RETRIEVING_ACKNOWLEDGEMENT_TYPE {
                        return Ok((d, a));
                    }
                },
                _ => return Err(Error::last_os_error()),
            }
        };
        Err(Error::last_os_error())
    }

    pub async fn recv_ack(&self, hash: &[u8], addr: SocketAddr) -> Vec<u8> {
        let req = Message::new(RETRIEVING_REQUEST_TYPE, hash).into();
        self.peer_sender.send((req, addr)).await.unwrap();
        let (data, _) = self.recv_message(RETRIEVING_ACKNOWLEDGEMENT_TYPE, hash).await.unwrap();
        data
    }

    pub async fn recv_req(&self, sender: Sender<(Message, SocketAddr)>) {
        loop {
            while let Some((d, a)) = self.peer_receiver.borrow_mut().recv().await {
                let message = Message::from(d);
                match message.clone() {
                    Message::SendingReq(_) | Message::RetrievingReq(_) => sender.send((message, a)).await.unwrap(),
                    _ => continue,
                }
            }
        }
    }

    pub async fn recv_content(&self, hash: &[u8], sender: Sender<(Message, SocketAddr)>) {
        loop {
            while let Some((d, a)) = self.peer_receiver.borrow_mut().recv().await {
                let message = Message::from(d);
                match message.clone() {
                    Message::ContentFilled(h, _) => {
                        if h.eq(hash) {
                            sender.send((message, a)).await.unwrap();
                        }
                    },
                    Message::Empty(h) => {
                        if h.eq(hash) {
                            sender.send((message, a)).await.unwrap();
                        }
                    },
                    _ => continue,
                }
            }
        }
    }

    pub async fn send_ack(&self, msg_type: u8, hash: &[u8], data: Option<Vec<u8>>, addr: SocketAddr) {
        if msg_type == SENDING_ACKNOWLEDGEMENT_TYPE {
            let message = Message::new(msg_type, hash).into();
            self.peer_sender.send((message, addr)).await.unwrap();
        } else if msg_type == RETRIEVING_ACKNOWLEDGEMENT_TYPE {
            let message = Message::new_with_data(CONTENT_FILLED_TYPE, hash, data.unwrap())
                .iter().map(|x| <Message as Into<Vec<u8>>>::into(x.clone())).into();
            self.peer_sender.send((message, addr)).await.unwrap();
        }
    }
}