use serde::{Deserialize, Serialize};

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

    pub fn as_json(&self) -> Result<String> {
        return match serde_json::to_string(&self) {
            Ok(j) => Ok(j.to_string()),
            Err(e) => Err(Box::new(MessageSerializationError(e.to_string()))),
        };
    }

    pub fn from_json(message: &str) -> Result<Self> {
        return match serde_json::from_str(message) {
            Ok(m) => Ok(m),
            Err(e) => Err(Box::new(MessageDeserializationError(e.to_string()))),
        };
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