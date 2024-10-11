use std::net::SocketAddr;
use std::str::FromStr;

use tokio::net::UdpSocket;

use leaf_common::{Codec, DeflateCodec};
use leaf_common::{Hasher, StreebogHasher};
use leaf_common::{builder, message::consts::*};

use errors::*;
use consts::*;

pub trait ClientPeer {
    async fn send(&self, chunk: &[u8]) -> Result<Vec<u8>, SendingMessageError>;
    async fn recv(&self, hash: &[u8]) -> Result<Vec<u8>, ReceivingMessageError>;
}

pub struct BroadcastClientPeer {
    socket: UdpSocket,
    hasher: StreebogHasher,
    codec: DeflateCodec,
}

impl BroadcastClientPeer {
    pub async fn new() -> Result<BroadcastClientPeer, ClientPeerInitializationError> {
        let addr = SocketAddr::new("192.168.124.1".parse().unwrap(), 0); // TODO
        let socket = match UdpSocket::bind(addr).await {
            Ok(s) => s,
            Err(e) => return Err(ClientPeerInitializationError(e.to_string())),
        };
        match socket.set_broadcast(true) {
            Ok(_) => {},
            Err(e) => return Err(ClientPeerInitializationError(e.to_string())),
        };

        let hasher = StreebogHasher::new();
        let codec = DeflateCodec::new();

        Ok(BroadcastClientPeer {
            socket,
            hasher,
            codec,
        })
    }
}

impl ClientPeer for BroadcastClientPeer {
    async fn send(&self, chunk: &[u8]) -> Result<Vec<u8>, SendingMessageError> {
        let hash = self.hasher.calc_hash_for_chunk(chunk);
        println!("\tHASH WAS CALCULATED");
        let message = match builder::build_encoded_message(&self.codec, SENDING_REQ_MSG_TYPE, &hash, None) {
            Ok(m) => m,
            Err(e) => return Err(SendingMessageError(e.to_string())),
        };
        println!("\tSENDING_REQ WAS BUILT");
        let broadcast_addr = SocketAddr::new("192.168.124.255".parse().unwrap(), DEFAULT_SERVER_PORT);
        match self.socket.send_to(&message, broadcast_addr).await {
            Ok(_) => {},
            Err(e) => return Err(SendingMessageError(e.to_string())),
        };
        println!("\tSENDIND_REQ WAS SENT TO 192.168.124.255");

        let mut peer_addr = None;
        let mut sending_ack_buf = [0u8; MAX_DATAGRAM_SIZE];
        loop {
            let (_, addr) = match self.socket.recv_from(&mut sending_ack_buf).await {
                Ok((s, a)) => (s, a),
                Err(e) => return Err(SendingMessageError(e.to_string())),
            };
            println!("\tSENDING_ACK WAS RECEIVED");
            let message = match builder::get_decode_message(&self.codec, &sending_ack_buf) {
                Ok(m) => m,
                Err(e) => return Err(SendingMessageError(e.to_string())),
            };
            println!("\tMESSAGE WAS DECODED");
            let msg_u8: u8 = message.get_type().into();
            if  msg_u8 == SENDING_ACK_MSG_TYPE && message.get_hash().eq(&hash) {
                peer_addr = Some(addr);
                break;
            }
        };

        let message = match builder::build_encoded_message(&self.codec, CONTENT_FILLED_MSG_TYPE, &hash, Some(chunk.to_vec())) {
            Ok(m) => m,
            Err(e) => return Err(SendingMessageError(e.to_string())),
        };
        println!("CONTENT_FILLED WAS BUILD");
        match self.socket.send_to(&message, peer_addr.unwrap()).await {
            Ok(_) => {},
            Err(e) => return Err(SendingMessageError(e.to_string())),
        };
        println!("CONTENT_FILLED WAS SENT TO {}", peer_addr.unwrap());

        Ok(hash)
    }

    async fn recv(&self, hash: &[u8]) -> Result<Vec<u8>, ReceivingMessageError> {
        let message = match builder::build_encoded_message(&self.codec, RETRIEVING_REQ_MSG_TYPE, &hash, None) {
            Ok(m) => m,
            Err(e) => return Err(ReceivingMessageError(e.to_string())),
        };
        let broadcast_addr = SocketAddr::new("192.168.124.255".parse().unwrap(), DEFAULT_SERVER_PORT);
        self.socket.send_to(&message, broadcast_addr).await.unwrap();

        let mut retrieving_ack_buf = [0u8; MAX_DATAGRAM_SIZE];
        let mut data = None;
        loop {
            let _ = self.socket.recv_from(&mut retrieving_ack_buf).await.unwrap();
            let message = match builder::get_decode_message(&self.codec, &retrieving_ack_buf) {
                Ok(m) => m,
                Err(e) => return Err(ReceivingMessageError(e.to_string())),
            };
            let msg_type: u8 = message.get_type().into();
            if msg_type == CONTENT_FILLED_MSG_TYPE && message.get_hash().eq(hash) {
                data = Some(message.get_data().unwrap());
                break;
            }
        }

        Ok(data.unwrap())
    }
}

mod consts {
    pub const DEFAULT_SERVER_PORT: u16 = 62092;
    pub const MAX_DATAGRAM_SIZE: usize = 65536;
}

mod errors {
    use std::fmt;
    use std::fmt::Formatter;

    #[derive(Debug, Clone)]
    pub struct SendingMessageError(pub String);

    impl fmt::Display for SendingMessageError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error sending message: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct ReceivingMessageError(pub String);

    impl fmt::Display for ReceivingMessageError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error receiving message: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct ClientPeerInitializationError(pub String);

    impl fmt::Display for ClientPeerInitializationError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error initialization client peer: {}", self.0)
        }
    }
}

#[cfg(test)]
mod tests {

}