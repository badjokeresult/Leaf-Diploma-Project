use std::str;

use base64::prelude::*;

use super::errors::*;
use crate::messages::types::Message;

type Result<T> = std::result::Result<T, Box<dyn CodecModuleError>>;

pub trait Codec {
    fn encode_message(message: Message) -> Vec<u8>;
    fn decode_message(message: &[u8]) -> Message;
}

pub struct Base64Codec;

impl Codec for Base64Codec {
    fn encode_message(message: Message) -> Result<Vec<u8>> {
        let json = message.as_json()?;

        let b64_string = BASE64_STANDARD.encode(&json).as_bytes();

        let b64_bytes_vec = b64_string.to_vec();
        Ok(b64_bytes_vec)
    }

    fn decode_message(message: &[u8]) -> Result<Message> {
        let json_bytes_vec = match BASE64_STANDARD.decode(message) {
            Ok(b) => b,
            Err(e) => return Err(Box::new(FromBase64DecodingError(e.to_string()))),
        };

        let json_str = match str::from_utf8(&json_bytes_vec) {
            Ok(s) => s,
            Err(e) => return Err(Box::new(NonUtf8BytesInDecodedStringError(e.to_string()))),
        };

        let message = Message::from_json(json_str)?;
        Ok(message)
    }
}
