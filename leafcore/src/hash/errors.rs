use std::fmt;
use std::fmt::Formatter;

pub trait HashModuleError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result;
}

#[derive(Debug, Clone)]
pub struct HashCalculationError(pub String);

impl HashModuleError for HashCalculationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Error during calculating hash for chunk: {}", self.0)
    }
}

impl fmt::Display for HashCalculationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        HashModuleError::fmt(&self, f)
    }
}
