use std::str;

use kuznechik::{AlgOfb, KeyStore, Kuznechik};

use errors::*;
use init::*;

type Result<T> = std::result::Result<T, Box<dyn CryptoError>>;

pub trait Encryptor {
    async fn encrypt_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>>;
    async fn decrypt_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>>;
}

pub struct KuznechikEncryptor {
    password_file: PasswordFilePathWrapper,
    gamma_file: GammaFilePathWrapper,
}

impl KuznechikEncryptor {
    pub fn new() -> Result<KuznechikEncryptor> {
        let password_file = PasswordFilePathWrapper::new()?;
        let gamma_file = GammaFilePathWrapper::new()?;

        Ok(KuznechikEncryptor {
            password_file,
            gamma_file,
        })
    }

    async fn load_passwd_gamma(&self) -> Result<(String, Vec<u8>)> {
        let password = self.password_file.load_passwd().await?;
        let gamma = self.gamma_file.load_gamma().await?;

        let password = match str::from_utf8(&password) {
            Ok(s) => String::from(s),
            Err(e) => return Err(Box::new(PasswordFromUtf8Error(e.to_string()))),
        };

        Ok((password, gamma))
    }
}

impl Encryptor for KuznechikEncryptor {
    async fn encrypt_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>> {
        let (password, gamma) = self.load_passwd_gamma().await?;

        let key = KeyStore::with_password(&password);
        let mut cipher = AlgOfb::new(&key).gamma(gamma.to_vec());

        let data = Vec::from(chunk);
        let encrypted_chunk = cipher.encrypt(data);

        Ok(encrypted_chunk)
    }

    async fn decrypt_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>> {
        let (password, gamma) = self.load_passwd_gamma().await?;

        let key = KeyStore::with_password(&password);
        let mut cipher = AlgOfb::new(&key).gamma(gamma.to_vec());

        let data = Vec::from(chunk);
        let decrypted_chunk = cipher.decrypt(data);

        Ok(decrypted_chunk)
    }
}

mod init {
    use std::path::PathBuf;

    use rand::{Rng, thread_rng};
    use tokio::fs;

    use super::errors::*;

    const WORKING_FOLDER_NAME: &str = ".leaf";
    const PASSWORD_FILE_NAME: &str = "passwd.txt";
    const GAMMA_FILE_NAME: &str = "gamma.bin";

    type Result<T> = std::result::Result<T, Box<dyn CryptoError>>;

    pub struct PasswordFilePathWrapper(pub PathBuf);

    impl PasswordFilePathWrapper {
        pub fn new() -> Result<PasswordFilePathWrapper> {
            let filepath = match dirs::home_dir() {
                Some(p) => p.join(WORKING_FOLDER_NAME).join(PASSWORD_FILE_NAME),
                None => return Err(Box::new(UserHomeDirResolvingError)),
            };

            Ok(PasswordFilePathWrapper {
                0: filepath,
            })
        }

        pub async fn load_passwd(&self) -> Result<Vec<u8>> {
            if let Err(_) = fs::read_to_string(&self.0).await {
                self.init_password_at_first_launch(32).await?;
            }
            let binding = match fs::read_to_string(&self.0).await {
                Ok(s) => s,
                Err(e) => return Err(Box::new(CredentialsFileInitializationError(e.to_string()))),
            };
            let binding = binding.as_bytes();
            let content = binding.to_vec();

            Ok(content)
        }

        async fn init_password_at_first_launch(&self, len: usize) -> Result<()> {
            let password: String = thread_rng()
                .sample_iter::<u8, _>(rand::distributions::Alphanumeric)
                .take(len)
                .map(|x| x as char)
                .collect();

            let password_as_bytes = password.as_bytes();

            match fs::write(&self.0, password_as_bytes).await {
                Ok(_) => Ok(()),
                Err(e) => Err(Box::new(CredentialsFileInitializationError(e.to_string()))),
            }
        }
    }

    pub struct GammaFilePathWrapper(pub PathBuf);

    impl GammaFilePathWrapper {
        pub fn new() -> Result<GammaFilePathWrapper> {
            let filepath = match dirs::home_dir() {
                Some(p) => p.join(WORKING_FOLDER_NAME).join(GAMMA_FILE_NAME),
                None => return Err(Box::new(UserHomeDirResolvingError)),
            };

            Ok(GammaFilePathWrapper {
                0: filepath,
            })
        }

        pub async fn load_gamma(&self) -> Result<Vec<u8>> {
            if let Err(_) = fs::read_to_string(&self.0).await {
                self.init_gamma_at_first_launch(32).await?;
            }

            let gamma = match fs::read_to_string(&self.0).await {
                Ok(s) => s,
                Err(e) => return Err(Box::new(CredentialsFileInitializationError(e.to_string()))),
            };
            let gamma = gamma.as_bytes();
            let gamma = gamma.to_vec();
            Ok(gamma)
        }

        async fn init_gamma_at_first_launch(&self, len: usize) -> Result<()> {
            let gamma: Vec<u8> = thread_rng()
                .sample_iter::<u8, _>(rand::distributions::Standard)
                .take(len)
                .collect();

            match fs::write(&self.0, &gamma).await {
                Ok(_) => Ok(()),
                Err(e) => Err(Box::new(CredentialsFileInitializationError(e.to_string()))),
            }
        }
    }
}

mod errors {
    use std::fmt;
    use std::fmt::Formatter;

    pub trait CryptoError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result;
    }

    #[derive(Debug, Clone)]
    pub struct PasswordFromUtf8Error(pub String);

    impl CryptoError for PasswordFromUtf8Error {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error getting password from UTF-8 string: {}", self.0)
        }
    }

    impl fmt::Display for PasswordFromUtf8Error {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            CryptoError::fmt(self, f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct UserHomeDirResolvingError;

    impl CryptoError for UserHomeDirResolvingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error resolving current user home dir")
        }
    }

    impl fmt::Display for UserHomeDirResolvingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            CryptoError::fmt(self, f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct CredentialsFileInitializationError(pub String);

    impl CryptoError for CredentialsFileInitializationError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error initializing credentials from file: {}", self.0)
        }
    }

    impl fmt::Display for CredentialsFileInitializationError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            CryptoError::fmt(self, f)
        }
    }
}

mod tests {

}