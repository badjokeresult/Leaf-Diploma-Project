use crate::messages::types::Message;

pub trait Codec {
    fn encode_message(message: Message) -> Vec<u8>;
    fn decode_message(message: &[u8]) -> Message;
}
