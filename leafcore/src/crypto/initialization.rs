use std::path::PathBuf;

use rand::{Rng, thread_rng};
use tokio::fs;

use super::errors::*;

pub struct PasswordFilePathWrapper(pub PathBuf);

impl PasswordFilePathWrapper {
    pub fn new() -> PasswordFilePathWrapper {
        PasswordFilePathWrapper {
            0: dirs::home_dir().unwrap().join(".leaf").join("passwd.txt"),
        }
    }

    pub async fn load_passwd(&self) -> Result<Vec<u8>, Box<dyn CryptoModuleError>> {
        if let Err(_) = fs::read_to_string(&self.0).await {
            self.init_password_at_first_launch(32).await?;
        }
        let binding = fs::read_to_string(&self.0).await?;
        let binding = binding.as_bytes();
        let content = binding.to_vec();

        Ok(content)
    }

    async fn init_password_at_first_launch(&self, len: usize) -> Result<(), CredentialsFileInitializationError> {
        let password: String = thread_rng()
            .sample_iter::<u8, _>(rand::distributions::Alphanumeric)
            .take(len)
            .map(|x| x as char)
            .collect();

        let password_as_bytes = password.as_bytes();

        match fs::write(&self.0, password_as_bytes).await {
            Ok(_) => Ok(()),
            Err(e) => Err(CredentialsFileInitializationError(e.to_string())),
        }
    }
}

pub struct GammaFilePathWrapper(pub PathBuf);

impl GammaFilePathWrapper {
    pub fn new() -> GammaFilePathWrapper {
        GammaFilePathWrapper {
            0: dirs::home_dir().unwrap().join(".leaf").join("gamma.txt"),
        }
    }

    pub async fn load_gamma(&self) -> Result<Vec<u8>, Box<dyn CryptoModuleError>> {
        if let Err(_) = fs::read_to_string(&self.0).await {
            self.init_gamma_at_first_launch(32).await?;
        }

        let gamma = fs::read_to_string(&self.0).await?;
        let gamma = gamma.as_bytes();
        let gamma = gamma.to_vec();
        Ok(gamma)
    }

    async fn init_gamma_at_first_launch(&self, len: usize) -> Result<(), CredentialsFileInitializationError> {
        let gamma: Vec<u8> = thread_rng()
            .sample_iter::<u8, _>(rand::distributions::Standard)
            .take(len)
            .collect();

        match fs::write(&self.0, &gamma).await {
            Ok(_) => Ok(()),
            Err(e) => Err(CredentialsFileInitializationError(e.to_string())),
        }
    }
}
