use std::str;
use base64::engine::GeneralPurpose;
use base64::prelude::*;

use super::errors::*;

pub trait Codec {
    fn encode_message(&self, message: &str) -> Vec<u8>;
    fn decode_message(&self, message: &[u8]) -> Result<String, FromBase64DecodingError>;
}

pub struct Base64Codec {
    standard: GeneralPurpose,
}

impl Base64Codec {
    pub fn new() -> Base64Codec {
        Base64Codec {
            standard: BASE64_STANDARD,
        }
    }
}

impl Codec for Base64Codec {
    fn encode_message(&self, message: &str) -> Vec<u8> {
        let binding = self.standard.encode(message);
        let b64_string = binding.as_bytes();
        let b64_bytes_vec = b64_string.to_vec();
        b64_bytes_vec
    }

    fn decode_message(&self, message: &[u8]) -> Result<String, FromBase64DecodingError> {
        let json_bytes_vec = match self.standard.decode(message) {
            Ok(b) => b,
            Err(e) => return Err(FromBase64DecodingError(e.to_string())),
        };

        Ok(String::from(str::from_utf8(&json_bytes_vec).unwrap()))
    }
}
