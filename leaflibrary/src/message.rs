use serde::{Deserialize, Serialize};

use consts::*;
use crate::codec::{Codec, DeflateCodec};

mod consts {
    pub const MAX_MESSAGE_SIZE: usize = 65243;
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub enum Message {
    SendingReq(Vec<u8>),
    SendingAck(Vec<u8>),
    RetrievingReq(Vec<u8>),
    RetrievingAck(Vec<u8>),
    ContentFilled(Vec<u8>, Vec<u8>),
    Empty(Vec<u8>),
}

impl Message {
    pub fn new_with_data(hash: &[u8], data: &[u8]) -> Vec<Message> {
        let chunks = data.chunks(MAX_MESSAGE_SIZE).map(|x| x.to_vec()).collect::<Vec<_>>();

        let mut messages = vec![];

        for chunk in chunks {
            messages.push(Message::ContentFilled(hash.to_vec(), chunk));
        }

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
