use std::fmt;
use std::fmt::Formatter;

#[derive(Debug, Clone)]
pub struct InvalidMessageTypeError(pub String);

impl fmt::Display for InvalidMessageTypeError {
   fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
       write!(f, "Invalid message type provided: {}", self.0)
   }
}

#[derive(Debug, Clone)]
pub struct MessageSerializationError(pub String);

impl fmt::Display for MessageSerializationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Message serialization or deserialization error: {}", self.0)
    }
}
