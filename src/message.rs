use serde::{Deserialize, Serialize};

use errors::*;
use crate::codec::{Codec, DeflateCodec};

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub enum Message {
    SendingReq(Vec<u8>), // from client, to server
    SendingAck(Vec<u8>), // from server, to client
    RetrievingReq(Vec<u8>), // from client, to server
    RetrievingAck(Vec<u8>, Vec<u8>), // from server, to client
    ContentFilled(Vec<u8>, Vec<u8>), // from both, to both
    Empty(Vec<u8>), // sign packet for ending content
}

pub mod consts {
    pub const SENDING_REQUEST_TYPE: u8 = 0;
    pub const SENDING_ACKNOWLEDGEMENT_TYPE: u8 = 1;
    pub const RETRIEVING_REQUEST_TYPE: u8 = 2;
    pub const RETRIEVING_ACKNOWLEDGEMENT_TYPE: u8 = 3;
    pub const CONTENT_FILLED_TYPE: u8 = 4;
    pub const EMPTY_TYPE: u8 = 5;
    pub const MAX_MESSAGE_SIZE: usize = 65243;
}

// #[derive(Serialize, Deserialize, Clone)]
// pub struct Message {
//     r#type: MessageType,
//     hash: Vec<u8>,
//     data: Option<Vec<u8>>,
// }

impl Message {
    pub fn new_with_data(msg_type_num: u8, hash: &[u8], data: Vec<u8>) -> Vec<Message> {
        let chunks = data.chunks(consts::MAX_MESSAGE_SIZE).map(|x| x.to_vec()).collect::<Vec<_>>();

        let mut messages = vec![];

        for chunk in chunks {
            messages.push(
                match msg_type_num {
                    consts::RETRIEVING_ACKNOWLEDGEMENT_TYPE => Message::RetrievingAck(hash.to_vec(), chunk),
                    consts::CONTENT_FILLED_TYPE => Message::ContentFilled(hash.to_vec(), chunk),
                    _ => panic!("Invalid message type selected"),
                }
            );
        }

        messages
    }

    pub fn new(msg_type_num: u8, hash: &[u8]) -> Message {
        match msg_type_num {
            consts::SENDING_REQUEST_TYPE => Message::SendingReq(hash.to_vec()),
            consts::SENDING_ACKNOWLEDGEMENT_TYPE => Message::SendingAck(hash.to_vec()),
            consts::RETRIEVING_REQUEST_TYPE => Message::RetrievingReq(hash.to_vec()),
            consts::EMPTY_TYPE => Message::Empty(hash.to_vec()),
            _ => panic!("Invalid message type selected"),
        }
    }

    fn as_json(&self) -> Result<String, MessageSerializationError> {
        match serde_json::to_string(&self) {
            Ok(j) => Ok(j.to_string()),
            Err(e) => Err(MessageSerializationError(e.to_string())),
        }
    }

    fn from_json(message: &str) -> Result<Self, MessageDeserializationError> {
        match serde_json::from_str(message) {
            Ok(m) => Ok(m),
            Err(e) => Err(MessageDeserializationError(e.to_string())),
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
        let codec = DeflateCodec::new();
        let json = codec.decode_message(&value).unwrap();
        Message::from_json(&json).unwrap()
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