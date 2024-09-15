use std::net::{IpAddr, SocketAddr};

use serde::{Deserialize, Serialize};

use super::errors::{InvalidMessageTypeError, MessageSerializationError};

pub const SENDING_REQ_MSG_TYPE: u8 = 0b0;
pub const RETRIEVING_REQ_MSG_TYPE: u8 = 0b1;
pub const SENDING_ACK_MSG_TYPE: u8 = 0b10;
pub const RETRIEVING_ACK_MSG_TYPE: u8 = 0b11;

#[repr(C)]
#[derive(Serialize, Deserialize)]
pub enum Message {
    SendingReq(IpAddr, Vec<u8>),
    RetrievingReq(IpAddr, Vec<u8>),
    SendingAck(Vec<u8>),
    RetrievingAck(Vec<u8>),
}

impl Message {
    pub fn build_message(addr: SocketAddr, data: &[u8], msg_type: u8) -> Result<Message, InvalidMessageTypeError> {
        let data_vec = data.to_vec();

        let mut message = None;

        if msg_type == SENDING_ACK_MSG_TYPE {
            message = Some(Ok(Message::SendingAck(data_vec)));
        } else if msg_type == SENDING_REQ_MSG_TYPE {
            message = Some(Ok(Message::SendingReq(addr.ip(), data_vec)));
        } else if msg_type == RETRIEVING_ACK_MSG_TYPE {
            message = Some(Ok(Message::RetrievingAck(data_vec)));
        } else if msg_type == RETRIEVING_REQ_MSG_TYPE {
            message = Some(Ok(Message::RetrievingReq(addr.ip(), data_vec)));
        } else {
            message = Some(Err(InvalidMessageTypeError(msg_type.to_string())));
        }

        message.unwrap()
    }

    pub fn as_json(&self) -> Result<Vec<u8>, MessageSerializationError> {
        let json = match serde_json::to_string(&self) {
            Ok(j) => j,
            Err(e) => return Err(MessageSerializationError(e.to_string())),
        }.as_bytes();
        Ok(json.to_vec())
    }

    pub fn from_json(json: &str) -> Result<Self, MessageSerializationError> {
        let message: Self = match serde_json::from_str(json) {
            Ok(m) => m,
            Err(e) => return Err(MessageSerializationError(e.to_string())),
        };
        Ok(message)
    }
}
