use std::io::{Read, Write};
use std::str;

use base64::engine::GeneralPurpose;
use base64::prelude::*;
use flate2::write::ZlibEncoder;
use flate2::read::ZlibDecoder;
use flate2::Compression;

use errors::*;

type Result<T> = std::result::Result<T, Box<dyn CodecError>>;

pub trait Codec {
    fn encode_message(&self, message: &str) -> Result<Vec<u8>>;
    fn decode_message(&self, message: &[u8]) -> Result<String>;
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
    fn encode_message(&self, message: &str) -> Result<Vec<u8>> {
        let binding = self.standard.encode(message);
        let b64_string = binding.as_bytes();
        let b64_bytes_vec = b64_string.to_vec();
        Ok(b64_bytes_vec)
    }

    fn decode_message(&self, message: &[u8]) -> Result<String> {
        let json_bytes_vec = match self.standard.decode(message) {
            Ok(b) => b,
            Err(e) => return Err(Box::new(FromBase64DecodingError(e.to_string()))),
        };

        Ok(String::from(str::from_utf8(&json_bytes_vec).unwrap()))
    }
}

pub struct DeflateCodec {
    compression: Compression,
}

impl DeflateCodec {
    pub fn new(compression: Compression) -> DeflateCodec {
        DeflateCodec {
            compression,
        }
    }
}

impl Codec for DeflateCodec {
    fn encode_message(&self, message: &str) -> Result<Vec<u8>> {
        let mut encoder = ZlibEncoder::new(Vec::new(), self.compression);
        match encoder.write_all(message.as_bytes()) {
            Ok(_) => Ok(return match encoder.finish() {
                Ok(d) => Ok(d),
                Err(e) => Err(Box::new(DeflateEncodingError(e.to_string()))),
            }),
            Err(e) => Err(Box::new(DeflateEncodingError(e.to_string()))),
        }
    }

    fn decode_message(&mut self, message: &[u8]) -> Result<String> {
        let mut decoder = ZlibDecoder::new(message);
        let mut decoded_message = String::new();

        match decoder.read_to_string(&mut decoded_message) {
            Ok(_) => Ok(decoded_message),
            Err(e) => Err(Box::new(DeflateDecodingError(e.to_string()))),
        }
    }
}


mod errors {
    use std::fmt;
    use std::fmt::Formatter;

    pub trait CodecError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result;
    }

    #[derive(Debug, Clone)]
    pub struct FromBase64DecodingError(pub String);

    impl CodecError for FromBase64DecodingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error decoding from base64: {}", self.0)
        }
    }

    impl fmt::Display for FromBase64DecodingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            CodecError::fmt(self, f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct DeflateEncodingError(pub String);

    impl CodecError for DeflateEncodingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error encoding with deflate: {}", self.0)
        }
    }

    impl fmt::Display for DeflateEncodingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            CodecError::fmt(self, f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct DeflateDecodingError(pub String);

    impl CodecError for DeflateDecodingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error decoding from deflate: {}", self.0)
        }
    }

    impl fmt::Display for DeflateDecodingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            CodecError::fmt(self, f)
        }
    }
}

mod tests {

}