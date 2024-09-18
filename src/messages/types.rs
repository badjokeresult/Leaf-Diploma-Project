use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use super::errors::MessageSerializationError;

#[derive(Serialize, Deserialize)]
pub enum MessageType {
    SendingReq,
    RetrievingReq,
    SendingAck,
    RetrievingAck,
}

#[derive(Serialize, Deserialize)]
pub struct Message {
    pub msg_type: MessageType,
    pub addr: Option<SocketAddr>,
    pub hash: Vec<u8>,
    pub data: Option<Vec<u8>>,
}

impl Message {
    pub fn new(msg_type: MessageType, addr: Option<SocketAddr>, hash: &[u8], data: Option<Vec<u8>>) -> Message {
        Message {
            msg_type,
            addr,
            hash: hash.to_vec(),
            data,
        }
    }

    pub fn as_json(&self) -> Result<String, MessageSerializationError> {
        let json = match serde_json::to_string(&self) {
            Ok(j) => j,
            Err(e) => return Err(MessageSerializationError(e.to_string())),
        }.to_string();
        Ok(json)
    }

    pub fn from_json(json: &str) -> Result<Self, MessageSerializationError> {
        let message: Self = match serde_json::from_str(json) {
            Ok(m) => m,
            Err(e) => return Err(MessageSerializationError(e.to_string())),
        };
        Ok(message)
    }
}
