use std::io::{Read, Write};
use std::str;

use flate2::write::ZlibEncoder;
use flate2::read::ZlibDecoder;
use flate2::Compression;

use errors::*;

pub trait Codec {
    fn encode_message(&self, message: &str) -> Result<Vec<u8>, DataEncodingError>;
    fn decode_message(&self, message: &[u8]) -> Result<String, DataDecodingError>;
}

pub struct DeflateCodec {
    compression: Compression,
}

impl DeflateCodec {
    pub fn new() -> DeflateCodec {
        DeflateCodec {
            compression: Compression::default(),
        }
    }
}

impl Codec for DeflateCodec {
    fn encode_message(&self, message: &str) -> Result<Vec<u8>, DataEncodingError> {
        let mut encoder = ZlibEncoder::new(Vec::new(), self.compression);
        return match encoder.write_all(message.as_bytes()) {
            Ok(_) => match encoder.finish() {
                Ok(d) => Ok(d),
                Err(e) => Err(DataEncodingError(e.to_string())),
            },
            Err(e) => Err(DataEncodingError(e.to_string())),
        }
    }

    fn decode_message(&self, message: &[u8]) -> Result<String, DataDecodingError> {
        let mut decoder = ZlibDecoder::new(message);
        let mut decoded_message = String::new();

        match decoder.read_to_string(&mut decoded_message) {
            Ok(_) => Ok(decoded_message),
            Err(e) => Err(DataDecodingError(e.to_string())),
        }
    }
}

mod errors {
    use std::fmt;
    use std::fmt::Formatter;

    #[derive(Debug, Clone)]
    pub struct DataEncodingError(pub String);

    impl fmt::Display for DataEncodingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error encoding data: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct DataDecodingError(pub String);

    impl fmt::Display for DataDecodingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error decoding data: {}", self.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deflate_encode_message_successful_encoding_returns_bytevec() {
        let codec = DeflateCodec::new();
        let message = "Hello World";
        let result: Vec<u8> = vec![120, 156, 243, 72, 205, 201, 201, 87, 8, 207, 47, 202, 73, 1, 0, 24, 11, 4, 29];

        let encoded = codec.encode_message(message).unwrap();

        assert_eq!(result, encoded);
    }

    #[test]
    fn test_deflate_decode_message_successful_decoding_returns_str() {
        let codec = DeflateCodec::new();
        let encoded = vec![120, 156, 243, 72, 205, 201, 201, 87, 8, 207, 47, 202, 73, 1, 0, 24, 11, 4, 29];
        let result = "Hello World";

        let decoded = codec.decode_message(&encoded).unwrap();

        assert_eq!(result, &decoded);
    }
}