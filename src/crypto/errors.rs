use std::fmt;
use std::fmt::Formatter;

pub trait CryptoModuleError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result;
}

#[derive(Debug, Clone)]
pub struct CredentialsFileInitializationError(pub String);

impl CryptoModuleError for CredentialsFileInitializationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Error during credentials file init: {}", self.0)
    }
}

impl fmt::Display for CredentialsFileInitializationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        CryptoModuleError::fmt(&self, f)
    }
}

#[derive(Debug, Clone)]
pub struct DataEncryptionError(pub String);

impl CryptoModuleError for DataEncryptionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Error during data encryption: {}", self.0)
    }
}

impl fmt::Display for DataEncryptionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        CryptoModuleError::fmt(&self, f)
    }
}

#[derive(Debug, Clone)]
pub struct DataDecryptionError(pub String);

impl CryptoModuleError for DataDecryptionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Error during data decryption: {}", self.0)
    }
}

impl fmt::Display for DataDecryptionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        CryptoModuleError::fmt(&self, f)
    }
}

#[derive(Debug, Clone)]
pub struct PasswordFromUtf8Error(pub String);

impl CryptoModuleError for PasswordFromUtf8Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Error during parsing password bytes to UTF8 string: {}", self.0)
    }
}

impl fmt::Display for PasswordFromUtf8Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        CryptoModuleError::fmt(&self, f)
    }
}

#[derive(Debug, Clone)]
pub struct UserHomeDirResolvingError;

impl CryptoModuleError for UserHomeDirResolvingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Error during current user home folder resolving")
    }
}

impl fmt::Display for UserHomeDirResolvingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        CryptoModuleError::fmt(&self, f)
    }
}
