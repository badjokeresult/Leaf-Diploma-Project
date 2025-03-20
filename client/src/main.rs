#![allow(unused_variables)]

use std::path::{Path, PathBuf};

use clap::Parser;
use clap::{arg, command};
use clap_derive::{Parser, ValueEnum};

use dialoguer::theme::ColorfulTheme;
use dialoguer::Password;

use common::{
    Chunks, ChunksHashes, Encryptor, Hasher, KuznechikEncryptor, ReedSolomonChunks,
    ReedSolomonChunksHashes, ReedSolomonSecretSharer, SecretSharer, StreebogHasher,
};

use consts::*;
use errors::SwitchUserError;

mod consts {
    pub const USER_NAME: &str = "leaf-client";
    pub const GROUP_NAME: &str = "leaf-client";
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(value_enum, short, long)]
    action: Action,
    #[arg(short, long)]
    file: String,
}

impl Args {
    pub fn get_action(&self) -> Action {
        self.action
    }
    pub fn get_file(&self) -> PathBuf {
        PathBuf::from(&self.file)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, ValueEnum)]
pub enum Action {
    Send,
    Receive,
}

pub fn load_args() -> Args {
    Args::parse()
}

#[cfg(target_os = "linux")]
fn switch_user(password: &str) -> Result<(), Box<dyn std::error::Error>> {
    use nix::libc::{setgid, setuid};

    let uid = users::get_user_by_name(USER_NAME).unwrap();
    let gid = users::get_group_by_name(GROUP_NAME).unwrap();

    if unsafe { setgid(gid.gid()) } != 0 && unsafe { setuid(uid.uid()) } != 0 {
        return Err(Box::new(SwitchUserError));
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn switch_user(password: &str) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: заменить на impersonate
    use windows_sys::Win32::Foundation::{FALSE, HANDLE};
    use windows_sys::Win32::Security::{
        ImpersonateLoggedOnUser, LogonUserW, RevertToSelf, LOGON32_LOGON_INTERACTIVE,
        LOGON32_PROVIDER_DEFAULT,
    };

    let username = String::from(USER_NAME) + "\0";
    let password = String::from(password) + "\0";
    let username = username.encode_utf16().collect::<Vec<u16>>();
    let password = password.encode_utf16().collect::<Vec<u16>>();
    let mut token: HANDLE = 0;

    // Аутентификация пользователя
    let success = unsafe {
        LogonUserW(
            username.as_ptr(),
            std::ptr::null(),
            password.as_ptr(),
            LOGON32_LOGON_INTERACTIVE,
            LOGON32_PROVIDER_DEFAULT,
            &mut token,
        )
    };

    if success != FALSE {
        // Запуск процесса от имени другого пользователя
        let result = ImpersonateLoggedOnUser(token);

        if result != FALSE {
            return Ok(());
        }
    }

    Err(Box::new(SwitchUserError))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = load_args(); // Получение аргументов командной строки

    // Запрашиваем пароль у пользователя
    let password = Password::with_theme(&ColorfulTheme::default())
        .with_prompt("Enter the password")
        .interact()?;

    switch_user(&password)?;

    // Используем тот же пароль для шифрования
    let sharer: Box<dyn SecretSharer> = Box::new(ReedSolomonSecretSharer::new()?);
    let encryptor: Box<dyn Encryptor> = Box::new(KuznechikEncryptor::new(&password).await?);
    let path = args.get_file(); // Получение пути к файлу

    match args.get_action() {
        Action::Send => {
            let hasher: Box<dyn Hasher> = Box::new(StreebogHasher::new());
            send_file(path, sharer, encryptor, hasher).await
        }
        Action::Receive => recv_file(path, sharer, encryptor).await,
    }
}

async fn send_file(
    path: impl AsRef<Path>,
    sharer: Box<dyn SecretSharer>,
    encryptor: Box<dyn Encryptor>,
    hasher: Box<dyn Hasher>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut chunks = ReedSolomonChunks::from_file(&path, &sharer).await?;
    chunks.encrypt(&encryptor)?;
    chunks.update_hashes(&hasher)?;
    let hashes = chunks.send().await?;
    hashes.save_to(path).await?;
    Ok(())
}

async fn recv_file(
    path: impl AsRef<Path>,
    sharer: Box<dyn SecretSharer>,
    encryptor: Box<dyn Encryptor>,
) -> Result<(), Box<dyn std::error::Error>> {
    let hashes = ReedSolomonChunksHashes::load_from(&path).await?;
    let mut chunks = ReedSolomonChunks::recv(hashes).await?;
    chunks.decrypt(&encryptor)?;
    chunks.into_file(path, &sharer).await?;
    Ok(())
}

mod errors {
    use std::error::Error;
    use std::fmt::{Display, Formatter};

    #[derive(Debug, Clone)]
    pub struct SwitchUserError;

    impl Display for SwitchUserError {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "Error switching user or group")
        }
    }

    impl Error for SwitchUserError {}
}
