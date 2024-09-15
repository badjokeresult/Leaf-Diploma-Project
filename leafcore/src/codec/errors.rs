use std::fmt;
use std::fmt::Formatter;

#[derive(Debug, Clone)]
pub struct FromBase64DecodingError(pub String);

impl fmt::Display for FromBase64DecodingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Error decoding message from BASE64 standard: {}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct NonUnicodeBytesInDecodedStringError(pub String);

impl fmt::Display for NonUnicodeBytesInDecodedStringError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Non Unicode bytes were found in received message: {}", self.0)
    }
}