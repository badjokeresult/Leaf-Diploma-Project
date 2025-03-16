//mod build;

use clap::Parser;
use clap::{arg, command};
use clap_derive::{Parser, ValueEnum};
use common::{
    Chunks, ChunksHashes, Encryptor, Hasher, KuznechikEncryptor, ReedSolomonChunks,
    ReedSolomonChunksHashes, ReedSolomonSecretSharer, SecretSharer, StreebogHasher,
};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Password;
use std::path::{Path, PathBuf};

// Добавляем новые импорты для смены пользователя
#[cfg(unix)]
use nix::unistd::{setgid, setuid, Gid, Uid};
#[cfg(windows)]
use std::ffi::OsStr;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use std::ptr::null_mut;
#[cfg(windows)]
use winapi::um::securitybaseapi::ImpersonateLoggedOnUser;
#[cfg(windows)]
use winapi::um::winbase::LogonUserW;
#[cfg(windows)]
use winapi::um::winnt::{HANDLE, LOGON32_LOGON_INTERACTIVE, LOGON32_PROVIDER_DEFAULT};

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

// Функция для смены пользователя в Unix-системах
#[cfg(unix)]
#[allow(unused_variables)]
fn switch_to_service_user(password: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Константы для идентификаторов сервисного пользователя
    // Замените эти значения на реальные ID вашего сервисного пользователя
    const SERVICE_USER_UID: u32 = 1001;
    const SERVICE_USER_GID: u32 = 1001;

    // В Unix-системах пароль потребуется только если используется PAM/LDAP аутентификация
    // В простом случае если у нас есть SUID-бит, пароль не требуется
    if !Uid::effective().is_root() {
        println!("Предупреждение: Нет прав для смены пользователя. Установите SUID-бит на исполняемый файл.");
        return Ok(());
    }

    // Сначала меняем GID, затем UID
    setgid(Gid::from_raw(SERVICE_USER_GID))?;
    setuid(Uid::from_raw(SERVICE_USER_UID))?;

    println!("Успешно изменен пользователь на сервисного");
    Ok(())
}

// Функция для смены пользователя в Windows
#[cfg(windows)]
fn switch_to_service_user(password: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Функция для конвертации строки в широкие символы для Windows API
    fn to_wide_string(s: &str) -> Vec<u16> {
        OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    // Учетные данные сервисного пользователя
    // Имя пользователя должно быть задано в системе
    const SERVICE_USERNAME: &str = "leaf-client";
    const SERVICE_DOMAIN: &str = "."; // Для локального компьютера

    let username = to_wide_string(SERVICE_USERNAME);
    let domain = to_wide_string(SERVICE_DOMAIN);
    let password = to_wide_string(password); // Используем пароль, введенный пользователем

    let mut token_handle: HANDLE = null_mut();

    // Получаем токен безопасности для сервисного пользователя
    let logon_result = unsafe {
        LogonUserW(
            username.as_ptr(),
            domain.as_ptr(),
            password.as_ptr(),
            LOGON32_LOGON_INTERACTIVE,
            LOGON32_PROVIDER_DEFAULT,
            &mut token_handle,
        )
    };

    if logon_result != 0 {
        let error = unsafe { winapi::um::errhandlingapi::GetLastError() };
        return Err(format!("Не удалось аутентифицировать пользователя: {}", error).into());
    }

    // Применяем олицетворение
    let impersonation_result = unsafe { ImpersonateLoggedOnUser(token_handle) };

    if impersonation_result != 0 {
        unsafe { winapi::um::handleapi::CloseHandle(token_handle) };
        let error = unsafe { winapi::um::errhandlingapi::GetLastError() };
        return Err(format!("Не удалось выполнить impersonation: {}", error).into());
    }

    // Мы специально не освобождаем токен и не вызываем RevertToSelf,
    // так как хотим, чтобы весь дальнейший код выполнялся под сервисным пользователем
    println!("Успешно изменен пользователь на {}", SERVICE_USERNAME);
    Ok(())
}

// Функция-заглушка для других платформ
#[cfg(not(any(unix, windows)))]
fn switch_to_service_user(_password: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Смена пользователя не поддерживается на этой платформе");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = load_args(); // Получение аргументов командной строки

    // Запрашиваем пароль у пользователя
    let password = Password::with_theme(&ColorfulTheme::default())
        .with_prompt("Enter the password")
        .interact()?;

    // Используем введенный пароль для смены пользователя
    switch_to_service_user(&password)?;

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
