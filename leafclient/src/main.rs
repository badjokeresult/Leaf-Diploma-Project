#![allow(unused_variables)] // Директива для игнорирования неиспользуемых переменных функций (переменная пароля не используется для смены пользователя в Linux, но она нужна для поддержания единого интерфейса)

use std::path::{Path, PathBuf}; // Зависимость стандартной библиотеки для работы с файловыми путями

use clap::Parser; // Внешние зависимости для работы с аргументами командной строки
use clap::{arg, command};
use clap_derive::{Parser, ValueEnum};

use dialoguer::theme::ColorfulTheme; // Внешние зависимости для работы с безопасным пользовательским вводом в терминале
use dialoguer::Password;

use leafcommon::{
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = load_args(); // Получение аргументов командной строки

    // Запрашиваем пароль у пользователя
    let password = Password::with_theme(&ColorfulTheme::default())
        .with_prompt("Enter the password")
        .interact()?; // Запрашиваем пароль от СЕРВИСНОЙ УЗ в интерактивном режиме

    // Используем тот же пароль для шифрования
    let sharer: Box<dyn SecretSharer<Vec<Vec<u8>>, Vec<u8>>> =
        Box::new(ReedSolomonSecretSharer::new()?); // Создаем объекты разделителя секрета, шифровальщика
    let encryptor: Box<dyn Encryptor<Vec<u8>>> =
        Box::new(KuznechikEncryptor::new(&password).await?);

    let path = &args.file;
    match args.get_action() {
        Action::Send => {
            // Если файл отправляется
            let hasher: Box<dyn Hasher<String>> = Box::new(StreebogHasher::new()); // Дополнительно создаем объект хэш-вычислителя
            send_file(path, sharer, encryptor, hasher).await // Отправляем файл
        }
        Action::Receive => recv_file(path, sharer, encryptor).await, // Если получение - вызываем функцию получения
    }
}

async fn send_file(
    path: impl AsRef<Path>,
    sharer: Box<dyn SecretSharer<Vec<Vec<u8>>, Vec<u8>>>,
    encryptor: Box<dyn Encryptor<Vec<u8>>>,
    hasher: Box<dyn Hasher<String>>,
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
    sharer: Box<dyn SecretSharer<Vec<Vec<u8>>, Vec<u8>>>,
    encryptor: Box<dyn Encryptor<Vec<u8>>>,
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
