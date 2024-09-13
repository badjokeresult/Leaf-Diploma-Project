use crate::messages::types::MessageType;

pub trait Codec {
    fn encode_message(message: MessageType) -> Vec<u8>;
    fn decode_message(message: &[u8]) -> MessageType;
}
