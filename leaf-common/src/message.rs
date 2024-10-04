use serde::{Deserialize, Serialize};

use errors::*;
use consts::*;

type Result<T> = std::result::Result<T, Box<dyn MessageError>>;

#[repr(u8)]
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum MessageType {
    SendingReq = SENDING_REQ_MSG_TYPE, // keeps hash
    RetrievingReq = RETRIEVING_REQ_MSG_TYPE, // keeps hash
    SendingAck = SENDING_ACK_MSG_TYPE, // keeps hash
    RetrievingAck = RETRIEVING_ACK_MSG_TYPE, // keeps data and its hash
    ContentFilled = CONTENT_FILLED_MSG_TYPE, // keeps data and its hash
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Message {
    r#type: MessageType,
    hash: Vec<u8>,
    data: Option<Vec<u8>>,
}

impl Message {
    pub fn new(msg_type_num: u8, hash: &[u8], data: Option<Vec<u8>>) -> Result<Message> {
        let r#type = MessageType::from(msg_type_num);

        Ok(Message {
            r#type,
            hash: hash.to_vec(),
            data,
        })
    }

    pub fn as_json(&self) -> Result<String> {
        return match serde_json::to_string(&self) {
            Ok(j) => Ok(j.to_string()),
            Err(_) => Err(Box::new(MessageSerializationError)),
        };
    }

    pub fn from_json(message: &str) -> Result<Self> {
        return match serde_json::from_str(message) {
            Ok(m) => Ok(m),
            Err(_) => Err(Box::new(MessageDeserializationError)),
        };
    }

    pub fn get_type(&self) -> u8 {
        self.r#type.clone().into()
    }

    pub fn get_hash(&self) -> Vec<u8> {
        self.hash.to_vec()
    }

    pub fn get_data(&self) -> Option<Vec<u8>> {
        let data = self.data.clone().unwrap();
        Some(data)
    }
}

impl Into<u8> for MessageType {
    fn into(self) -> u8 {
        match self {
            MessageType::SendingAck => SENDING_ACK_MSG_TYPE,
            MessageType::ContentFilled => CONTENT_FILLED_MSG_TYPE,
            MessageType::RetrievingAck => RETRIEVING_ACK_MSG_TYPE,
            MessageType::RetrievingReq => RETRIEVING_REQ_MSG_TYPE,
            MessageType::SendingReq => SENDING_REQ_MSG_TYPE,
        }
    }
}

impl From<u8> for MessageType {
    fn from(value: u8) -> Self {
        match value {
            SENDING_REQ_MSG_TYPE => MessageType::SendingReq,
            CONTENT_FILLED_MSG_TYPE => MessageType::ContentFilled,
            SENDING_ACK_MSG_TYPE => MessageType::SendingAck,
            RETRIEVING_REQ_MSG_TYPE => MessageType::RetrievingReq,
            RETRIEVING_ACK_MSG_TYPE => MessageType::RetrievingAck,
            _ => panic!(),
        }
    }
}

pub mod consts {
    pub const SENDING_REQ_MSG_TYPE: u8 = 0;
    pub const RETRIEVING_REQ_MSG_TYPE: u8 = 1;
    pub const SENDING_ACK_MSG_TYPE: u8 = 2;
    pub const RETRIEVING_ACK_MSG_TYPE: u8 = 3;
    pub const CONTENT_FILLED_MSG_TYPE: u8 = 4;
}

pub mod builder {
    use crate::codec::Codec;
    use super::Message;

    use super::errors::*;
    use super::Result;

    pub fn build_encoded_message(codec: &Box<dyn Codec>, msg_type: u8, hash: &[u8], data: Option<Vec<u8>>) -> Result<Vec<u8>> {
        match Message::new(msg_type, hash, data) {
            Ok(m) => match m.as_json() {
                Ok(j) => match codec.encode_message(&j) {
                    Ok(b) => Ok(b),
                    Err(_) => return Err(Box::new(MessageSerializationError)),
                },
                Err(e) => Err(e)
            },
            Err(e) => Err(e),
        }
    }

    pub fn get_decode_message(codec: &Box<dyn Codec>, buf: &[u8]) -> Result<Message> {
        match codec.decode_message(buf) {
            Ok(s) => match Message::from_json(&s) {
                Ok(m) => Ok(m),
                Err(e) => Err(e),
            },
            Err(_) => Err(Box::new(MessageDeserializationError)),
        }
    }
}

mod errors {
    use std::fmt;
    use std::fmt::Formatter;

    pub trait MessageError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result;
    }

    #[derive(Debug, Clone)]
    pub struct MessageSerializationError;

    impl MessageError for MessageSerializationError {
        fn fmt(&self, f: &mut Formatter) -> fmt::Result {
            write!(f, "Error serialization message into JSON")
        }
    }

    impl fmt::Display for MessageSerializationError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            MessageError::fmt(self, f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct MessageDeserializationError;

    impl MessageError for MessageDeserializationError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error deserializing message from JSON")
        }
    }

    impl fmt::Display for MessageDeserializationError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            MessageError::fmt(self, f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct InvalidMessageTypeError(pub u8);

    impl MessageError for InvalidMessageTypeError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Invalid message type id: {}", self.0)
        }
    }

    impl fmt::Display for InvalidMessageTypeError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            MessageError::fmt(self, f)
        }
    }
}

mod tests {

}