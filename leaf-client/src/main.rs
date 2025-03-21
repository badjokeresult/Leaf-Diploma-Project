#![allow(unused_variables)] // Директива для игнорирования неиспользуемых переменных функций (переменная пароля не используется для смены пользователя в Linux, но она нужна для поддержания единого интерфейса)

use std::path::{Path, PathBuf}; // Зависимость стандартной библиотеки для работы с файловыми путями

use clap::Parser; // Внешние зависимости для работы с аргументами командной строки
use clap::{arg, command};
use clap_derive::{Parser, ValueEnum};

use dialoguer::theme::ColorfulTheme; // Внешние зависимости для работы с безопасным пользовательским вводом в терминале
use dialoguer::Password;

use common::{
    // Зависимости внутренней библиотеки проекта
    Chunks,
    ChunksHashes,
    Encryptor,
    Hasher,
    KuznechikEncryptor,
    ReedSolomonChunks,
    ReedSolomonChunksHashes,
    ReedSolomonSecretSharer,
    SecretSharer,
    StreebogHasher,
};

use consts::*; // Зависимость внутреннего модуля, содержит нужные для работы константы
use errors::*; // Зависимость внутреннего модуля, содержит составные типы ошибок

mod consts {
    // Модуль констант
    pub const USER_NAME: &str = "leaf-client"; // Имя сервисной УЗ, от имени которой будут выполняться все последующие действия
    #[cfg(target_os = "linux")]
    pub const GROUP_NAME: &str = "leaf-client"; // Имя группы сервисной УЗ, используется только в Linux
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    // Структура для хранения аргументов командной строки
    #[arg(value_enum, short, long)]
    action: Action, // Аргумент, отвечающий за реализуемое действие
    #[arg(short, long)]
    file: String, // Аргумент, указывающий целевой файл
}

impl Args {
    pub fn get_action(&self) -> Action {
        // Получение аргумента действия
        self.action
    }
    pub fn get_file(&self) -> PathBuf {
        // Получение аргумента пути к файлу
        PathBuf::from(&self.file)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, ValueEnum)]
pub enum Action {
    // Перечисление, хранящее возможные варианты действий
    Send,    // Действие по отправке файла
    Receive, // Действие по получению файла
}

pub fn load_args() -> Args {
    // Функция парсинга полученных аргументов
    Args::parse()
}

#[cfg(target_os = "linux")]
fn switch_user(password: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Функция смены эффективного пользователя в Linux
    use nix::libc::{setgid, setuid}; // Внешние зависимости для смены GID и UID POSIX

    let uid = users::get_user_by_name(USER_NAME).unwrap(); // Получаем UID по имени пользователя
    let gid = users::get_group_by_name(GROUP_NAME).unwrap(); // Получаем GID по имени пользователя

    if unsafe { setgid(gid.gid()) } != 0 && unsafe { setuid(uid.uid()) } != 0 {
        // Сначала меняем GID, только потом UID
        return Err(Box::new(SwitchUserError)); // Если произошла ошибка - пробрасываем ее наверх
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn switch_user(password: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Смена пользователя в Windows, использует механизм имперсонизации
    use windows_sys::Win32::Foundation::{FALSE, HANDLE}; // Внешние зависимости для работы с нужными функциями WinAPI
    use windows_sys::Win32::Security::{
        ImpersonateLoggedOnUser, LogonUserW, RevertToSelf, LOGON32_LOGON_INTERACTIVE,
        LOGON32_PROVIDER_DEFAULT,
    };

    let username = String::from(USER_NAME) + "\0"; // Добавляем нулевой символ в конец имени
    let password = String::from(password) + "\0"; // Добавляем нулевой символ в конец пароля
    let username = username.encode_utf16().collect::<Vec<u16>>(); // Кодируем имя в UTF-16
    let password = password.encode_utf16().collect::<Vec<u16>>(); // Кодируем пароль в UTF-16
    let mut token: HANDLE = 0; // Создаем токен авторизации

    // Выполняем аутентификацию пользователя
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
        // Смена пользователя механизмом имперсонизации
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
        .interact()?; // Запрашиваем пароль от СЕРВИСНОЙ УЗ в интерактивном режиме

    switch_user(&password)?; // Выполняем смену пользователя на сервисную УЗ

    // Используем тот же пароль для шифрования
    let sharer: Box<dyn SecretSharer> = Box::new(ReedSolomonSecretSharer::new()?); // Создаем объекты разделителя секрета, шифровальщика
    let encryptor: Box<dyn Encryptor> = Box::new(KuznechikEncryptor::new(&password).await?);
    let path = args.get_file(); // Получаем путь к файлу

    match args.get_action() {
        Action::Send => {
            // Если файл отправляется
            let hasher: Box<dyn Hasher> = Box::new(StreebogHasher::new()); // Дополнительно создаем объект хэш-вычислителя
            send_file(path, sharer, encryptor, hasher).await // Отправляем файл
        }
        Action::Receive => recv_file(path, sharer, encryptor).await, // Если получение - вызываем функцию получения
    }
}

async fn send_file(
    path: impl AsRef<Path>,
    sharer: Box<dyn SecretSharer>,
    encryptor: Box<dyn Encryptor>,
    hasher: Box<dyn Hasher>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Функция отправки файла
    let mut chunks = ReedSolomonChunks::from_file(&path, &sharer).await?; // Получаем чанки
    chunks.encrypt(&encryptor)?; // Шифруем их
    chunks.update_hashes(&hasher)?; // Обновляем их хэш-суммы
    let hashes = chunks.send().await?; // Отправляем чанки в домен и получаем назад их хэш-суммы
    hashes.save_to(path).await?; // Сохраняем хэш-суммы в целевом файле
    Ok(())
}

async fn recv_file(
    path: impl AsRef<Path>,
    sharer: Box<dyn SecretSharer>,
    encryptor: Box<dyn Encryptor>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Функция получения файла
    let hashes = ReedSolomonChunksHashes::load_from(&path).await?; // Получаем хэш-суммы из файла
    let mut chunks = ReedSolomonChunks::recv(hashes).await?; // Получаем чанки по хэшам
    chunks.decrypt(&encryptor)?; // Расшифровываем чанки
    chunks.into_file(path, &sharer).await?; // Восстанавливаем из них содержимое и записываем его в целевой файл
    Ok(())
}

mod errors {
    // Модуль с составными типами ошибок
    use std::error::Error; // Трейт ошибки из стандартной библиотеки
    use std::fmt::{Display, Formatter}; // Зависимости стандартной библиотеки для отображения данных на экране

    #[derive(Debug, Clone)]
    pub struct SwitchUserError; // Ошибка смены пользователя

    impl Display for SwitchUserError {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "Error switching user or group") // Текст ошибки смены пользователя
        }
    }

    impl Error for SwitchUserError {} // Реализация ошибки по умолчанию
}
