use std::str;

use base64::prelude::*;

use super::errors::*;
use crate::messages::types::Message;

pub trait Codec {
    fn encode_message(message: Message) -> Vec<u8>;
    fn decode_message(message: &[u8]) -> Message;
}

pub struct Base64Codec;

impl Codec for Base64Codec {
    fn encode_message(message: &str) -> Vec<u8> {
        let b64_string = BASE64_STANDARD.encode(message).as_bytes();
        let b64_bytes_vec = b64_string.to_vec();
        b64_bytes_vec
    }

    fn decode_message(message: &[u8]) -> Result<String, Box<dyn CodecModuleError>> {
        let json_bytes_vec = match BASE64_STANDARD.decode(message) {
            Ok(b) => b,
            Err(e) => return Err(Box::new(FromBase64DecodingError(e.to_string()))),
        };

        let json_str = match str::from_utf8(&json_bytes_vec) {
            Ok(s) => s,
            Err(e) => return Err(Box::new(NonUtf8BytesInDecodedStringError(e.to_string()))),
        };

        Ok(String::from(json_str))
    }
}
