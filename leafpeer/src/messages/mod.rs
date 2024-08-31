use std::net::{IpAddr, SocketAddr};

use serde::{Deserialize, Serialize};

use errors::InvalidMessageTypeError;

type Result<T> = std::result::Result<T, InvalidMessageTypeError>;

pub const SENDING_REQ_MSG_TYPE: u8 = 0b0;
pub const RETRIEVING_REQ_MSG_TYPE: u8 = 0b1;
pub const SENDING_ACK_MSG_TYPE: u8 = 0b10;
pub const RETRIEVING_ACK_MSG_TYPE: u8 = 0b11;

#[repr(C)]
#[derive(Serialize, Deserialize)]
pub enum MessageType {
    SendingReq(IpAddr, Vec<u8>),
    RetrievingReq(IpAddr, Vec<u8>),
    SendingAck(Vec<u8>),
    RetrievingAck(Vec<u8>),
}

impl MessageType {
    #[warn(unused_assignments)]
    pub fn build_message(addr: SocketAddr, data: &[u8], msg_type: u8) -> Result<MessageType> {
        let data_vec = data.to_vec();

        let mut message = None;

        if msg_type == SENDING_ACK_MSG_TYPE {
            message = Some(Ok(MessageType::SendingAck(data_vec)));
        } else if msg_type == SENDING_REQ_MSG_TYPE {
            message = Some(Ok(MessageType::SendingReq(addr.ip(), data_vec)));
        } else if msg_type == RETRIEVING_ACK_MSG_TYPE {
            message = Some(Ok(MessageType::RetrievingAck(data_vec)));
        } else if msg_type == RETRIEVING_REQ_MSG_TYPE {
            message = Some(Ok(MessageType::RetrievingReq(addr.ip(), data_vec)));
        } else {
            message = Some(Err(InvalidMessageTypeError(msg_type.to_string())));
        }

        message.unwrap()
    }
}

pub mod errors {
    use std::fmt;
    use std::fmt::Formatter;

    #[derive(Debug, Clone)]
    pub struct InvalidMessageTypeError(pub String);

    impl fmt::Display for InvalidMessageTypeError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Invalid message type provided: {}", self.0)
        }
    }
}
