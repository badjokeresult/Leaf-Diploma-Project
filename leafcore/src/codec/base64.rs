use openssl::base64;

use crate::codec::codec::Codec;
use crate::messages::types::MessageType;

pub struct Base64Codec;

impl Codec for Base64Codec {
    fn encode_message(message: MessageType) -> Vec<u8> {
        let json = serde_json::to_string(&message).unwrap();
        let json_bytes = json.as_bytes();
        let b64_string = base64::encode_block(json_bytes);
        let b64_bytes = b64_string.as_bytes();
        let b64_bytes_vec = b64_bytes.to_vec();
        b64_bytes_vec
    }

    fn decode_message(message: &[u8]) -> MessageType {
        let b64_str = str::from_utf8(message).unwrap();
        let json_bytes_vec = base64::decode_block(b64_str).unwrap();
        let json_bytes = json_bytes_vec.as_slice();
        let json_str = str::from_utf8(json_bytes).unwrap();
        let message: MessageType = serde_json::from_str(json_str).unwrap();
        message
    }
}
