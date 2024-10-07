use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::net::UdpSocket;

use leaf_common::{Codec, DeflateCodec};
use leaf_common::{Hasher, StreebogHasher};
use leaf_common::{message_builder, message::consts::*};

use errors::*;
use consts::*;

type Result<T> = std::result::Result<T, Box<dyn ClientPeerError>>;

pub trait ClientPeer {
    fn send(&self, chunk: &[u8]) -> Result<Vec<u8>>;
    fn recv(&self, hash: &[u8]) -> Result<Vec<u8>>;
}

pub struct BroadcastClientPeer {
    socket: UdpSocket,
    hasher: StreebogHasher,
    codec: DeflateCodec,
}

impl BroadcastClientPeer {
    pub fn new() -> Result<BroadcastClientPeer> {
        let addr = "192.168.124.1:0";
        let socket = match UdpSocket::bind(addr) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("ERROR BINDING SOCKET CLIENT : {}", e.to_string());
                return Err(Box::new(HashCalculationError));
            }
        };
        match socket.set_broadcast(true) {
            Ok(_) => {},
            Err(e) => {
                eprintln!("ERROR SETTING SOCKET TO BROADCAST : {}", e.to_string());
                return Err(Box::new(HashCalculationError));
            }
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
    fn send(&self, chunk: &[u8]) -> Result<Vec<u8>> {
        let hash = match self.hasher.calc_hash_for_chunk(chunk) {
            Ok(h) => h,
            Err(_) => {
                eprintln!("ERROR CALC HASH");
                return Err(Box::new(HashCalculationError));
            },
        };
        let message = match message_builder::build_encoded_message(&self.codec, SENDING_REQ_MSG_TYPE, &hash, None) {
            Ok(m) => m,
            Err(_) => {
                eprintln!("ERROR BUILDING SENDING_REQ MESSAGE");
                return Err(Box::new(BuildingMessageError));
            }
        };
        let broadcast_addr = IpAddr::from_str("192.168.124.255").unwrap();
        let broadcast_addr = SocketAddr::new(broadcast_addr, DEFAULT_SERVER_PORT);
        self.socket.send_to(&message, broadcast_addr).unwrap();

        let mut peer_addr = None;
        let mut sending_ack_buf = [0u8; MAX_DATAGRAM_SIZE];
        loop {
            let (_, addr) = self.socket.recv_from(&mut sending_ack_buf).unwrap();
            let message = match message_builder::get_decode_message(&self.codec, &sending_ack_buf) {
                Ok(m) => m,
                Err(_) => {
                    eprintln!("ERROR DECODING MESSAGE");
                    return Err(Box::new(CollectingMessageError));
                }
            };
            let msg_u8: u8 = message.get_type();
            if  msg_u8 == SENDING_ACK_MSG_TYPE && message.get_hash().eq(&hash) {
                peer_addr = Some(addr);
                break;
            }
        };

        let message = match message_builder::build_encoded_message(&self.codec, CONTENT_FILLED_MSG_TYPE, &hash, Some(chunk.to_vec())) {
            Ok(m) => m,
            Err(_) => {
                eprintln!("ERROR BUILDING SENDING_REQ MESSAGE");
                return Err(Box::new(BuildingMessageError));
            }
        };
        self.socket.send_to(&message, peer_addr.unwrap()).unwrap();

        eprintln!("SENT!!!");
        Ok(hash)
    }

    fn recv(&self, hash: &[u8]) -> Result<Vec<u8>> {
        let message = match message_builder::build_encoded_message(&self.codec, RETRIEVING_REQ_MSG_TYPE, &hash, None) {
            Ok(m) => m,
            Err(_) => return Err(Box::new(BuildingMessageError))
        };
        let broadcast = IpAddr::from_str("192.168.124.255").unwrap();
        let broadcast_addr = SocketAddr::new(broadcast, DEFAULT_SERVER_PORT);
        self.socket.send_to(&message, broadcast_addr).unwrap();

        let mut retrieving_ack_buf = [0u8; MAX_DATAGRAM_SIZE];
        let mut data = None;
        loop {
            let _ = self.socket.recv_from(&mut retrieving_ack_buf).unwrap();
            let message = match message_builder::get_decode_message(&self.codec, &retrieving_ack_buf) {
                Ok(m) => m,
                Err(_) => return Err(Box::new(CollectingMessageError)),
            };
            if message.get_type() == CONTENT_FILLED_MSG_TYPE && message.get_hash().eq(hash) {
                data = Some(message.get_data().unwrap());
                break;
            }
        }

        Ok(data.unwrap())
    }
}

mod consts {
    pub const DEFAULT_SERVER_PORT: u16 = 62092;
    pub const MAX_DATAGRAM_SIZE: usize = 508;
}

mod errors {
    use std::fmt;
    use std::fmt::{Display, Formatter};

    pub trait ClientPeerError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result;
        fn to_string(&self) -> String;
    }

    #[derive(Debug, Clone)]
    pub struct HashCalculationError;

    impl ClientPeerError for HashCalculationError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error calculation hash for chunk")
        }

        fn to_string(&self) -> String {
            String::from("Error calculation hash for chunk")
        }
    }

    impl fmt::Display for HashCalculationError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            ClientPeerError::fmt(self, f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct BuildingMessageError;

    impl ClientPeerError for BuildingMessageError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error building message")
        }
        fn to_string(&self) -> String {
            String::from("Error building message")
        }
    }

    impl fmt::Display for BuildingMessageError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            ClientPeerError::fmt(self, f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct CollectingMessageError;

    impl ClientPeerError for CollectingMessageError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error collecting message")
        }

        fn to_string(&self) -> String {
            String::from("Error collecting message")
        }
    }

    impl fmt::Display for CollectingMessageError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            ClientPeerError::fmt(self, f)
        }
    }
}

#[cfg(test)]
mod tests {

}