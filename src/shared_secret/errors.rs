use std::fmt;
use std::fmt::Formatter;

#[derive(Debug, Clone)]
pub struct DataSplittingError;

impl fmt::Display for DataSplittingError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Error attempting to split a data into chunks")
    }
}

#[derive(Debug, Clone)]
pub struct DataRecoveringError;

impl fmt::Display for DataRecoveringError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Error recovering data from chunks")
    }
}
