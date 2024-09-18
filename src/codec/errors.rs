use std::fmt;
use std::fmt::Formatter;

pub trait CodecModuleError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result;
}

#[derive(Debug, Clone)]
pub struct FromBase64DecodingError(pub String);

impl CodecModuleError for FromBase64DecodingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Error decoding message from BASE64 standard: {}", self.0)
    }
}

impl fmt::Display for FromBase64DecodingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        CodecModuleError::fmt(&self, f)
    }
}

#[derive(Debug, Clone)]
pub struct NonUtf8BytesInDecodedStringError(pub String);

impl CodecModuleError for NonUtf8BytesInDecodedStringError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Non Unicode bytes were found in received message: {}", self.0)
    }
}

impl fmt::Display for NonUtf8BytesInDecodedStringError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        CodecModuleError::fmt(&self, f)
    }
}
