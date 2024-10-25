use std::io::Error;
use std::net::SocketAddr;
use std::sync::mpsc::Receiver;
use crate::message::{Message, consts::*};
use crate::peer::BroadcastUdpPeer;

pub struct BroadcastUdpClient {
    peer: BroadcastUdpPeer,
    from_peer_receiver: Receiver<(Message, SocketAddr)>,
}

impl BroadcastUdpClient {
    pub fn new() -> BroadcastUdpClient {
        let (peer, from_peer_receiver) = BroadcastUdpPeer::new().unwrap();

        BroadcastUdpClient {
            peer,
            from_peer_receiver,
        }
    }

    pub fn send(&self, hash: &[u8], chunk: &[u8]) -> Result<(), Error> {
        let message = Message::new(SENDING_REQUEST_TYPE, hash);
        self.peer.send_req(Into::<Vec<_>>::into(message).as_slice())?;
        loop {
            if let Ok((m, a)) = self.from_peer_receiver.recv() {
                if let Message::SendingAck(h) = m {
                    if h == hash {
                        let messages = Message::new_with_data(CONTENT_FILLED_TYPE, hash, chunk.to_vec());
                        for msg in messages {
                            self.peer.send_content(&Into::<Vec<_>>::into(msg), a)?;
                        }
                        return Ok(());
                    }
                }
            }
        };
    }

    pub fn recv(&self, hash: &[u8]) -> Result<Vec<u8>, Error> {
        let mut result = vec![];

        let message = Message::new(RETRIEVING_REQUEST_TYPE, hash);
        self.peer.send_req(Into::<Vec<_>>::into(message).as_slice())?;
        loop {
            if let Ok((m, _)) = self.from_peer_receiver.recv() {
                if let Message::RetrievingAck(h, mut d) = m {
                    if h == hash {
                        result.append(&mut d);
                    }
                } else if let Message::Empty(h) = m {
                    if h == hash {
                        return Ok(result);
                    }
                }
            }
        }
    }

    pub fn shutdown(&self) {

    }
}