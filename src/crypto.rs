use std::str;

use kuznechik::{AlgOfb, KeyStore, Kuznechik};

use errors::*;
use init::*;

pub trait Encryptor {
    async fn encrypt_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>, DataEncryptionError>;
    async fn decrypt_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>, DataDecryptionError>;
}

pub struct KuznechikEncryptor {
    password_file: PasswordFilePathWrapper,
    gamma_file: GammaFilePathWrapper,
}

impl KuznechikEncryptor {
    pub fn new() -> Result<KuznechikEncryptor, InitializeEncryptorError> {
        let password_file = match PasswordFilePathWrapper::new() {
            Ok(p) => p,
            Err(e) => return Err(InitializeEncryptorError(e.to_string())),
        };
        let gamma_file = match GammaFilePathWrapper::new() {
            Ok(g) => g,
            Err(e) => return Err(InitializeEncryptorError(e.to_string())),
        };

        Ok(KuznechikEncryptor {
            password_file,
            gamma_file,
        })
    }

    async fn load_passwd_gamma(&self) -> Result<(String, Vec<u8>), LoadingCredentialsError> {
        let password = match self.password_file.load_passwd().await {
            Ok(p) => p,
            Err(e) => return Err(LoadingCredentialsError(e.to_string())),
        };
        let gamma = match self.gamma_file.load_gamma().await {
            Ok(g) => g,
            Err(e) => return Err(LoadingCredentialsError(e.to_string())),
        };

        let password = match str::from_utf8(&password) {
            Ok(s) => String::from(s),
            Err(e) => return Err(LoadingCredentialsError(e.to_string())),
        };

        Ok((password, gamma))
    }
}

impl Encryptor for KuznechikEncryptor {
    async fn encrypt_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>, DataEncryptionError> {
        let (password, gamma) = match self.load_passwd_gamma().await {
            Ok((p, g)) => (p, g),
            Err(e) => return Err(DataEncryptionError(e.to_string())),
        };

        let key = KeyStore::with_password(&password);
        let mut cipher = AlgOfb::new(&key).gamma(gamma.to_vec());

        let data = Vec::from(chunk);
        let encrypted_chunk = cipher.encrypt(data);

        Ok(encrypted_chunk)
    }

    async fn decrypt_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>, DataDecryptionError> {
        let (password, gamma) = match self.load_passwd_gamma().await {
            Ok((p, g)) => (p, g),
            Err(e) => return Err(DataDecryptionError(e.to_string())),
        };

        let key = KeyStore::with_password(&password);
        let mut cipher = AlgOfb::new(&key).gamma(gamma.to_vec());

        let data = Vec::from(chunk);
        let decrypted_chunk = cipher.decrypt(data);

        Ok(decrypted_chunk)
    }
}

mod init {
    use std::path::PathBuf;

    use tokio::fs;

    use rand::{Rng, thread_rng};

    use super::errors::*;
    use crate::consts::*;

    pub struct PasswordFilePathWrapper(pub PathBuf);

    impl PasswordFilePathWrapper {
        pub fn new() -> Result<PasswordFilePathWrapper, UserHomeDirResolvingError> {
            let filepath = match dirs::home_dir() {
                Some(p) => p.join(WORKING_FOLDER_NAME).join(PASSWORD_FILE_NAME),
                None => return Err(UserHomeDirResolvingError),
            };

            Ok(PasswordFilePathWrapper {
                0: filepath,
            })
        }

        pub async fn load_passwd(&self) -> Result<Vec<u8>, LoadingCredentialsError> {
            if let Err(_) = fs::read(&self.0).await {
                match self.init_password_at_first_launch(32).await {
                    Ok(_) => {},
                    Err(e) => return Err(LoadingCredentialsError(e.to_string())),
                };
            }
            let content = match fs::read(&self.0).await {
                Ok(s) => s,
                Err(e) => return Err(LoadingCredentialsError(e.to_string())),
            };

            Ok(content)
        }

        async fn init_password_at_first_launch(&self, len: usize) -> Result<(), InitializingCredentialsError> {
            let password: String = thread_rng()
                .sample_iter::<u8, _>(rand::distributions::Alphanumeric)
                .take(len)
                .map(|x| x as char)
                .collect();

            let password_as_bytes = password.as_bytes();

            match fs::write(&self.0, password_as_bytes).await {
                Ok(_) => Ok(()),
                Err(e) => Err(InitializingCredentialsError(e.to_string())),
            }
        }
    }

    pub struct GammaFilePathWrapper(pub PathBuf);

    impl GammaFilePathWrapper {
        pub fn new() -> Result<GammaFilePathWrapper, UserHomeDirResolvingError> {
            let filepath = match dirs::home_dir() {
                Some(p) => p.join(WORKING_FOLDER_NAME).join(GAMMA_FILE_NAME),
                None => return Err(UserHomeDirResolvingError),
            };

            Ok(GammaFilePathWrapper {
                0: filepath,
            })
        }

        pub async fn load_gamma(&self) -> Result<Vec<u8>, LoadingCredentialsError> {
            if let Err(_) = fs::read(&self.0).await {
                match self.init_gamma_at_first_launch(32).await {
                    Ok(_) => {},
                    Err(e) => return Err(LoadingCredentialsError(e.to_string())),
                };
            }

            let gamma = match fs::read(&self.0).await {
                Ok(s) => s,
                Err(e) => return Err(LoadingCredentialsError(e.to_string())),
            };

            Ok(gamma)
        }

        async fn init_gamma_at_first_launch(&self, len: usize) -> Result<(), InitializingCredentialsError> {
            let gamma: Vec<u8> = thread_rng()
                .sample_iter::<u8, _>(rand::distributions::Standard)
                .take(len)
                .collect();

            match fs::write(&self.0, &gamma).await {
                Ok(_) => Ok(()),
                Err(e) => Err(InitializingCredentialsError(e.to_string())),
            }
        }
    }

    #[cfg(test)]
    mod tests {

    }
}

pub mod errors {
    use std::fmt;
    use std::fmt::Formatter;

    #[derive(Debug, Clone)]
    pub struct UserHomeDirResolvingError;

    impl fmt::Display for UserHomeDirResolvingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error resolving user home dir")
        }
    }

    #[derive(Debug, Clone)]
    pub struct InitializeEncryptorError(pub String);

    impl fmt::Display for InitializeEncryptorError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error initialize encryptor: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct LoadingCredentialsError(pub String);

    impl fmt::Display for LoadingCredentialsError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error loading credentials: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct DataEncryptionError(pub String);

    impl fmt::Display for DataEncryptionError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error encrypting data: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct DataDecryptionError(pub String);

    impl fmt::Display for DataDecryptionError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error decrypting data: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct InitializingCredentialsError(pub String);

    impl fmt::Display for InitializingCredentialsError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error initializing credentials: {}", self.0)
        }
    }
}

#[cfg(test)]
mod tests {

}