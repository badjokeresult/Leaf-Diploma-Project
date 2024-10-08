use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

use errors::*;
use consts::*;

#[repr(u8)]
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum MessageType {
    SendingReq = SENDING_REQ_MSG_TYPE,
    RetrievingReq = RETRIEVING_REQ_MSG_TYPE,
    SendingAck = SENDING_ACK_MSG_TYPE,
    RetrievingAck = RETRIEVING_ACK_MSG_TYPE,
    ContentFilled = CONTENT_FILLED_MSG_TYPE,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Message {
    r#type: MessageType,
    hash: Vec<u8>,
    data: Option<Vec<u8>>,
}

impl Message {
    pub fn new(msg_type_num: u8, hash: &[u8], data: Option<Vec<u8>>) -> Message {
        let r#type = MessageType::from(msg_type_num);

        Message {
            r#type,
            hash: hash.to_vec(),
            data,
        }
    }

    pub fn as_json(&self) -> Result<String, MessageSerializationError> {
        return match serde_json::to_string(&self) {
            Ok(j) => Ok(j.to_string()),
            Err(e) => Err(MessageSerializationError(e.to_string())),
        };
    }

    pub fn from_json(message: &str) -> Result<Self, MessageDeserializationError> {
        return match serde_json::from_str(message) {
            Ok(m) => Ok(m),
            Err(e) => Err(MessageDeserializationError(e.to_string())),
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

impl Display for Message {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Message : [ type : {} , hash : {:#?} , data : {:#?} ]", match self.r#type {
            MessageType::SendingReq => "SendingReq",
            MessageType::RetrievingReq => "RetrievingReq",
            MessageType::SendingAck => "SendingAck",
            MessageType::RetrievingAck => "RetrievingAck",
            MessageType::ContentFilled => "ContentFilled",
        }, self.hash, self.data.clone().unwrap())
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
    use crate::Codec;
    use crate::DeflateCodec;

    use super::Message;
    use super::errors::*;

    pub fn build_encoded_message(codec: &DeflateCodec, msg_type: u8, hash: &[u8], data: Option<Vec<u8>>) -> Result<Vec<u8>, MessageSerializationError> {
        match Message::new(msg_type, hash, data) {
            Ok(m) => match m.as_json() {
                Ok(j) => match codec.encode_message(&j) {
                    Ok(b) => Ok(b),
                    Err(e) => return Err(Box::new(MessageSerializationError(e.to_string()))),
                },
                Err(e) => Err(MessageSerializationError(e.to_string()))
            },
            Err(e) => Err(e),
        }
    }

    pub fn get_decode_message(codec: &DeflateCodec, buf: &[u8]) -> Result<Message, MessageDeserializationError> {
        match codec.decode_message(buf) {
            Ok(s) => match Message::from_json(&s) {
                Ok(m) => Ok(m),
                Err(e) => Err(e),
            },
            Err(e) => Err(MessageDeserializationError(e.to_string())),
        }
    }

    #[cfg(test)]
    mod tests {

    }
}

mod errors {
    use std::fmt;
    use std::fmt::Formatter;

    #[derive(Debug, Clone)]
    pub struct MessageSerializationError(pub String);

    impl fmt::Display for MessageSerializationError {
        fn fmt(&self, f: &mut Formatter) -> fmt::Result {
            write!(f, "Error serialization message into JSON: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct MessageDeserializationError(pub String);

    impl fmt::Display for MessageDeserializationError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error deserializing message from JSON: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct InvalidMessageTypeError(pub u8);

    impl fmt::Display for InvalidMessageTypeError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Invalid message type id: {}", self.0)
        }
    }
}

#[cfg(test)]
mod tests {

}