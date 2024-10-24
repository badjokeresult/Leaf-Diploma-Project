use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

use errors::*;
use crate::codec::{Codec, DeflateCodec};

#[repr(u8)]
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub enum MessageType {
    SendingReq = 0,
    RetrievingReq = 1,
    SendingAck = 2,
    RetrievingAck = 3,
    ContentFilled = 4,
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
        match serde_json::to_string(&self) {
            Ok(j) => Ok(j.to_string()),
            Err(e) => Err(MessageSerializationError(e.to_string())),
        }
    }

    pub fn from_json(message: &str) -> Result<Self, MessageDeserializationError> {
        match serde_json::from_str(message) {
            Ok(m) => Ok(m),
            Err(e) => Err(MessageDeserializationError(e.to_string())),
        }
    }

    pub fn get_type(&self) -> MessageType {
        self.r#type.clone()
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
            MessageType::SendingAck => 2,
            MessageType::ContentFilled => 4,
            MessageType::RetrievingAck => 3,
            MessageType::RetrievingReq => 1,
            MessageType::SendingReq => 0,
        }
    }
}

impl From<u8> for MessageType {
    fn from(value: u8) -> Self {
        match value {
            0 => MessageType::SendingReq,
            4 => MessageType::ContentFilled,
            2 => MessageType::SendingAck,
            1 => MessageType::RetrievingReq,
            3 => MessageType::RetrievingAck,
            _ => panic!(),
        }
    }
}

impl Into<Vec<u8>> for Message {
    fn into(self) -> Vec<u8> {
        let codec = DeflateCodec::new();
        let json = self.as_json().unwrap();
        codec.encode_message(&json).unwrap()
    }
}

impl From<Vec<u8>> for Message {
    fn from(value: Vec<u8>) -> Self {
        let json = std::str::from_utf8(&value).unwrap();
        Message::from_json(json).unwrap()
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

    #[derive(Debug, Clone)]
    pub struct BuildingMessageError(pub String);

    impl fmt::Display for BuildingMessageError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error building message: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct ReconstructingMessageError(pub String);

    impl fmt::Display for ReconstructingMessageError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error reconstructing message: {}", self.0)
        }
    }
}

#[cfg(test)]
mod tests {

}