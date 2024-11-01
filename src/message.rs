use serde::{Deserialize, Serialize};

use crate::consts::*;
use crate::codec::{Codec, DeflateCodec};

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub enum Message {
    SendingReq(Vec<u8>),
    SendingAck(Vec<u8>),
    RetrievingReq(Vec<u8>),
    RetrievingAck(Vec<u8>, Option<Vec<u8>>),
    ContentFilled(Vec<u8>, Vec<u8>),
    Empty(Vec<u8>),
}

impl Message {
    pub fn new_with_data(msg_type_num: u8, hash: &[u8], data: &[u8]) -> Vec<Message> {
        let chunks = data.chunks(MAX_MESSAGE_SIZE).map(|x| x.to_vec()).collect::<Vec<_>>();

        let mut messages = vec![];

        for chunk in chunks {
            match msg_type_num {
                RETRIEVING_ACKNOWLEDGEMENT_TYPE => messages.push(Message::RetrievingAck(hash.to_vec(), Some(chunk))),
                CONTENT_FILLED_TYPE => messages.push(Message::ContentFilled(chunk.to_vec(), chunk)),
                _ => panic!(),
            }
        }

        messages.push(Message::Empty(hash.to_vec()));

        messages
    }
}

impl Into<Vec<u8>> for Message {
    fn into(self) -> Vec<u8> {
        let codec = DeflateCodec::new();
        let json = serde_json::to_string(&self).unwrap();
        codec.encode_message(&json).unwrap()
    }
}

impl From<Vec<u8>> for Message {
    fn from(value: Vec<u8>) -> Self {
        let codec = DeflateCodec::new();
        let json = codec.decode_message(&value).unwrap();
        serde_json::from_str(&json).unwrap()
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