mod codec;
mod errors;

use serde::{Deserialize, Serialize};

use codec::{Base64Codec, Codec};
use errors::MessageSerializationError;

#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageType {
    SendingReq,
    RetrievingReq,
    SendingAck,
    RetrievingAck,
    ContentFilled,
}

#[derive(Serialize, Deserialize)]
pub struct Message {
    pub msg_type: MessageType,
    pub hash: Vec<u8>,
    pub data: Option<Vec<u8>>,
}

impl Message {
    pub fn new(msg_type: MessageType, hash: &[u8], data: Option<Vec<u8>>) -> Message {
        Message {
            msg_type,
            hash: hash.to_vec(),
            data,
        }
    }

    pub fn as_encoded_json(&self) -> Result<Vec<u8>, MessageSerializationError> {
        let b64_encoder: Box<dyn Codec> = Box::new(Base64Codec::new());

        let json = match serde_json::to_string(&self) {
            Ok(j) => j,
            Err(e) => return Err(MessageSerializationError(e.to_string())),
        }.to_string();
        Ok(b64_encoder.encode_message(&json))
    }

    pub fn from_encoded_json(data: &[u8]) -> Result<Self, MessageSerializationError> {
        let b64_encoder: Box<dyn Codec> = Box::new(Base64Codec::new());

        let message: Self = match serde_json::from_str(match &b64_encoder.decode_message(data) {
            Ok(s) => s,
            Err(e) => return Err(MessageSerializationError(e.to_string())),
        }) {
            Ok(m) => m,
            Err(e) => return Err(MessageSerializationError(e.to_string())),
        };
        Ok(message)
    }
}
