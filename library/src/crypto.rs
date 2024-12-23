use std::path::PathBuf;
use std::str;

use kuznechik::{AlgOfb, KeyStore, Kuznechik};

use std::cell::RefCell;
use std::io::Write;

use streebog::digest::consts::U32;
use streebog::digest::core_api::{CoreWrapper, CtVariableCoreWrapper};
use streebog::{Digest, Oid256, StreebogVarCore};

use errors::*;
use init::*;

pub trait Encryptor {
    fn encrypt_chunk(&self, chunk: &[u8]) -> impl std::future::Future<Output = Result<Vec<u8>, DataEncryptionError>> + Send;
    fn decrypt_chunk(&self, chunk: &[u8]) -> impl std::future::Future<Output = Result<Vec<u8>, DataDecryptionError>> + Send;
}

pub struct KuznechikEncryptor {
    password_file: PasswordFilePathWrapper,
    gamma_file: GammaFilePathWrapper,
}

impl KuznechikEncryptor {
    pub fn new(password_file: &PathBuf, gamma_file: &PathBuf) -> Result<KuznechikEncryptor, InitializeEncryptorError> {
        let password_file = match PasswordFilePathWrapper::new(password_file) {
            Ok(p) => p,
            Err(e) => return Err(InitializeEncryptorError(e.to_string())),
        };
        let gamma_file = match GammaFilePathWrapper::new(gamma_file) {
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

    pub struct PasswordFilePathWrapper(pub PathBuf);

    impl PasswordFilePathWrapper {
        pub fn new(password_file: &PathBuf) -> Result<PasswordFilePathWrapper, UserHomeDirResolvingError> {
            Ok(PasswordFilePathWrapper {
                0: password_file.clone(),
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
        pub fn new(gamma_file: &PathBuf) -> Result<GammaFilePathWrapper, UserHomeDirResolvingError> {
            Ok(GammaFilePathWrapper {
                0: gamma_file.clone(),
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

pub trait Hasher {
    fn calc_hash_for_chunk(&self, chunk: &[u8]) -> Vec<u8>;
}

pub struct StreebogHasher {
    hasher: RefCell<CoreWrapper<CtVariableCoreWrapper<StreebogVarCore, U32, Oid256>>>,
}

impl StreebogHasher {
    pub fn new() -> StreebogHasher {
        StreebogHasher {
            hasher: RefCell::new(streebog::Streebog256::new()),
        }
    }
}

impl Hasher for StreebogHasher {
    fn calc_hash_for_chunk(&self, chunk: &[u8]) -> Vec<u8> {
        self.hasher.borrow_mut().update(chunk);
        let result = self.hasher.borrow_mut().clone().finalize();

        let result = result.to_vec();
        self.hasher.borrow_mut().flush().unwrap();

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streebog_calc_hash_for_chunk_successful_calculation() {
        let hasher = StreebogHasher::new();
        let message = b"Hello World";
        let result = vec![102, 108, 244, 251, 247, 78, 198, 138, 102, 232, 221, 61, 48, 97, 176, 51, 117, 104, 206, 33, 161, 4, 84, 29, 77, 238, 3, 245, 68, 140, 41, 175];

        let hash = hasher.calc_hash_for_chunk(message);

        assert_eq!(hash, result);
    }
}
