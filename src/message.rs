use serde::{Deserialize, Serialize};

use crate::codec::{Base64Codec, Codec};

use errors::*;

type Result<T> = std::result::Result<T, Box<dyn MessageError>>;

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

    pub fn as_encoded_json(&self) -> Result<Vec<u8>> {
        let b64_encoder: Box<dyn Codec> = Box::new(Base64Codec::new());

        let json = match serde_json::to_string(&self) {
            Ok(j) => j,
            Err(e) => return Err(Box::new(MessageSerializationError(e.to_string()))),
        }.to_string();

        match b64_encoder.encode_message(&json) {
            Ok(a) => Ok(a),
            Err(e) => Err(Box::new(MessageSerializationError(e.to_string()))),
        }
    }

    pub fn from_encoded_json(data: &[u8]) -> Result<Self> {
        let b64_encoder: Box<dyn Codec> = Box::new(Base64Codec::new());

        let message: Self = match serde_json::from_str(match &b64_encoder.decode_message(data) {
            Ok(s) => s,
            Err(e) => return Err(MessageDeserializationError(e.to_string())),
        }) {
            Ok(m) => m,
            Err(e) => return Err(MessageDeserializationError(e.to_string())),
        };

        Ok(message)
    }
}

mod errors {
    use std::fmt;
    use std::fmt::Formatter;

    pub trait MessageError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result;
    }

    #[derive(Debug, Clone)]
    pub struct MessageSerializationError(pub String);

    impl MessageError for MessageSerializationError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "Error serialization message into JSON: {}", self.0)
        }
    }

    impl fmt::Display for MessageSerializationError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            MessageError::fmt(self, f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct MessageDeserializationError(pub String);

    impl MessageError for MessageDeserializationError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error deserializing message from JSON: {}", self.0)
        }
    }

    impl fmt::Display for MessageDeserializationError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            MessageError::fmt(self, f)
        }
    }
}

mod tests {

}