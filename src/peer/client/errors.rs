use std::fmt;
use std::fmt::Formatter;

pub trait ClientSidePeerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result;
}

#[derive(Debug, Clone)]
pub struct ClientInitError(pub String);

impl ClientSidePeerError for ClientInitError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Error during initialization of client part: {}", self.0)
    }
}

impl fmt::Display for ClientInitError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        ClientSidePeerError::fmt(&self, f)
    }
}

#[derive(Debug, Clone)]
pub struct SendingDataError(pub String);

impl ClientSidePeerError for SendingDataError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Error sending data from client: {}", self.0)
    }
}

impl fmt::Display for SendingDataError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        ClientSidePeerError::fmt(&self, f)
    }
}

#[derive(Debug, Clone)]
pub struct ReceivingDataError(pub String);

impl ClientSidePeerError for ReceivingDataError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Error receiving data by client: {}", self.0)
    }
}

impl fmt::Display for ReceivingDataError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        ClientSidePeerError::fmt(&self, f)
    }
}

#[derive(Debug, Clone)]
pub struct RetrievingTimeoutError(pub String);

impl ClientSidePeerError for RetrievingTimeoutError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Timeout exceeded: {}", self.0)
    }
}

impl fmt::Display for RetrievingTimeoutError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        ClientSidePeerError::fmt(&self, f)
    }
}
