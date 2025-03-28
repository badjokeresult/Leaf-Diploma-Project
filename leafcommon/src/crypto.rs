#![allow(dead_code)]

use std::path::PathBuf; // Зависимость стандартной библиотеки для использования структуры по работе с файловыми путями

use argon2::Argon2; // Внешняя зависимость для создания ключа из гаммы, соли и пароля
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _}; // Внешняя зависимость для кодирования и декодирования по алгоритму Base64
use kuznyechik::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use kuznyechik::{Block, Key, Kuznyechik};
use rand::{rngs::OsRng, Rng}; // Внешняя зависимость для генерации псевдослучайных последовательностей
use serde::{Deserialize, Serialize}; // Внешняя зависимость для сериализации и десериализации структур
use tokio::fs; // Внешняя зависимость для асинхронной работы c файловой системой // Внешние зависимости для работы с симметричным шифром "Кузнечик (ГОСТ Р 34.12-2015)"

use consts::*; // Внутренняя зависимость модуля констант
use errors::*; // Внутренняя зависимость модуля для использования собственных типов ошибок

mod consts {
    #[cfg(target_os = "windows")]
    pub const HOME_DIR_VAR: &str = "USERPROFILE";

    #[cfg(target_os = "linux")]
    pub const HOME_DIR_VAR: &str = "HOME";

    pub const APP_DIR: &str = ".leaf";
    pub const METADATA_PATH: &str = "metadata.bin";
}

#[derive(Serialize, Deserialize)] // Использование сериализации и десериализации для данной структуры
struct EncryptionMetadata {
    // Структура для хранения гаммы и соли для использования в шифровании "Кузнечиком"
    gamma: Vec<u8>, // Закодированная по Base64 гамма
    salt: Vec<u8>,  // Закодированная по Base64 соль
    token: Vec<u8>, // Закодированный по Base64 токен
}

pub trait Encryptor {
    // Трейт для структур, реализующих шифрование
    fn encrypt_chunk(&self, chunk: &[u8]) -> Vec<u8>; // Прототип метода шифрования массива данных
    fn decrypt_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>, DecryptionError>; // Прототип метода дешифрования массива данных
}

pub struct KuznechikEncryptor {
    // Структура, реализующая шифрование по ГОСТ Р 34.12-2015 "Кузнечик"
    cipher: Kuznyechik,     // Ключ шифрования
    gamma: Vec<u8>,         // Гамма для шифрования
    metadata_path: PathBuf, // Путь к файлу с метаданными
}

impl KuznechikEncryptor {
    pub async fn new() -> Result<Self, InitializationError> {
        // Метод инициализации полей структуры гаммой и ключом
        let metadata_path = Self::get_metadata_path().await?; // Получаем путь до файла с метаданными при помощи метода

        let (gamma, salt, token) = if metadata_path.exists() {
            // Если файл с метаданными существует, то читаем данные из него и идем дальше
            let metadata: EncryptionMetadata = Self::load_metadata(&metadata_path).await?;
            (
                BASE64
                    .decode(&metadata.gamma)
                    .map_err(|e| InitializationError(e.to_string()))?,
                BASE64
                    .decode(&metadata.salt)
                    .map_err(|e| InitializationError(e.to_string()))?,
                BASE64
                    .decode(&metadata.token)
                    .map_err(|e| InitializationError(e.to_string()))?,
            )
        } else {
            // Если такого файла нет, то создаем новые гамму и соль
            let mut gamma = vec![0u8; 16]; // Создаем 128-битный буфер для гаммы
            let mut salt = vec![0u8; 32]; // Создаем 256-битный буфер для соли
            let mut token = vec![0u8; 32]; // Создаем 256-битный буфер для токена
            OsRng.fill(&mut gamma[..]); // Заполняем буфер гаммы случайными данными
            OsRng.fill(&mut salt[..]); // Заполняем буфер соли случайными данными
            OsRng.fill(&mut token[..]);

            let metadata = EncryptionMetadata {
                gamma: BASE64.encode(&gamma).into_bytes(),
                salt: BASE64.encode(&salt).into_bytes(),
                token: BASE64.encode(&token).into_bytes(),
            }; // Создаем новый экземпляр структуры и заполняем его поля соответствующими буферами
            Self::save_metadata(&metadata_path, &metadata).await?; // Сохраняем метаданные в файл

            (gamma, salt, token)
        };

        let config = Argon2::default(); // Создание конфигурации для создания ключа шифрования
        let mut key = vec![0u8; 32]; // Создаем буфер для ключа
        config
            .hash_password_into(&token, &salt, &mut key)
            .map_err(|e| InitializationError(e.to_string()))?; // Создаем ключ и записываем его в буфер

        let cipher_key = Key::from_slice(&key); // Создаем объект ключа шифрования из буфера
        let cipher = Kuznyechik::new(&cipher_key); // Создаем объект шифратора

        Ok(Self {
            cipher,
            gamma,
            metadata_path,
        }) // Создаем и возвращаем новый экземпляр структуры
    }

    async fn get_metadata_path() -> Result<PathBuf, InitializationError> {
        // Метод получения пути файла с метаданными
        let base_path = PathBuf::from(
            std::env::var(HOME_DIR_VAR).map_err(|e| InitializationError(e.to_string()))?,
        ); // Получаем полный путь до директории с конфигурациями приложений в домашнем каталоге пользователя

        // Создаем директорию нашего приложения
        let app_dir = base_path.join(APP_DIR);
        fs::create_dir_all(&app_dir)
            .await
            .map_err(|e| InitializationError(e.to_string()))?;

        Ok(app_dir.join(METADATA_PATH)) // Возвращаем полный путь до файла с метаданными
    }

    async fn load_metadata(path: &PathBuf) -> Result<EncryptionMetadata, InitializationError> {
        // Метод получения данных из файла метаданных
        Ok(serde_json::from_slice(
            &fs::read(path)
                .await
                .map_err(|e| InitializationError(e.to_string()))?,
        )
        .map_err(|e| InitializationError(e.to_string()))?) // Десериализуем прочитанный JSON-текст в структуру и возвращаем его
    }

    async fn save_metadata(
        path: &PathBuf,
        metadata: &EncryptionMetadata,
    ) -> Result<(), InitializationError> {
        // Метод записи данных в файл метаданных
        Ok(fs::write(
            path,
            &serde_json::to_vec(metadata).map_err(|e| InitializationError(e.to_string()))?, // Сериализуем объект в JSON-текст с пробельными символами
        )
        .await
        .map_err(|e| InitializationError(e.to_string()))?) // Записываем текст в файл
    }

    pub async fn regenerate_gamma_and_token(&mut self) -> Result<(), GammaRegenerationError> {
        // Метод регенерации гаммы
        let mut token = vec![0u8; 32];
        OsRng.fill(&mut self.gamma[..]); // Гамма заполняется новыми случайными данными
        OsRng.fill(&mut token[..]);

        let metadata = EncryptionMetadata {
            gamma: BASE64.encode(&self.gamma).into_bytes(),
            salt: BASE64
                .decode(
                    Self::load_metadata(&self.metadata_path)
                        .await
                        .map_err(|e| GammaRegenerationError(e.to_string()))?
                        .salt,
                )
                .map_err(|e| GammaRegenerationError(e.to_string()))?, // Создается новый экземпляр структуры метаданных, все поля предварительно кодируются в Base64
            token: BASE64.encode(&token).into_bytes(),
        };
        Self::save_metadata(&self.metadata_path, &metadata)
            .await
            .map_err(|e| GammaRegenerationError(e.to_string())) // Сохраняем новые метаданные
    }
}

impl Encryptor for KuznechikEncryptor {
    // Блок реализации трейта для структуры
    fn encrypt_chunk(&self, chunk: &[u8]) -> Vec<u8> {
        // Метод шифрования данных на месте
        let mut padded_data = chunk.to_vec(); // Копируем данные в новую переменную
        while padded_data.len() % 16 != 0 {
            // Выравниваем данные по 16 байт
            padded_data.push(0);
        }

        let mut result = Vec::with_capacity(padded_data.len()); // Создаем буфер для зашифрованных данных

        for c in padded_data.chunks(16) {
            // Для каждого блока по 16 байт
            let mut block = [0u8; 16]; // Выделяем место для блока
            block.copy_from_slice(c); // Копируем блок во временный буфер

            for (b, g) in block.iter_mut().zip(self.gamma.iter()) {
                *b ^= g; // Выполняем XOR для блока с гаммой
            }

            let mut block = Block::clone_from_slice(&block); // Создаем структуру блока, с которой умеет работать экземпляр шифратора
            self.cipher.encrypt_block(&mut block); // Выполняем шифрование
            result.extend_from_slice(&block); // Записываем защифрованные данные в конец результирующего буфера
        }

        result
    }

    fn decrypt_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>, DecryptionError> {
        // Метод дешифрования данных на месте
        // Если данные не выравнены по 16 байт, то возвращаем ошибку
        if chunk.len() % 16 != 0 {
            return Err(DecryptionError(String::from(
                "Invalid encrypted data length",
            )));
        }

        let mut result = Vec::with_capacity(chunk.len()); // Создаем буфер для хранения дешифрованных данных

        for c in chunk.chunks(16) {
            // Для каждого блока по 16 байт
            let mut block = [0u8; 16]; // Создаем временный буфер для блока
            block.copy_from_slice(c); // Записываем данные во временный буфер

            let mut block = Block::clone_from_slice(&block); // Создаем объект блока для дешифратора
            self.cipher.decrypt_block(&mut block); // Дешифруем данные

            for (b, g) in block.iter_mut().zip(self.gamma.iter()) {
                *b ^= g; // Выполняем XOR с гаммой для блока
            }

            result.extend_from_slice(&block); // Записываем дешифрованные данные в конец общего буфера
        }

        Ok(result)
    }
}

pub mod hash {
    pub mod streebog {
        use streebog::digest::Update;
        use streebog::Digest;

        pub fn calc_hash(chunk: &[u8]) -> String {
            let mut hasher = streebog::Streebog256::new(); // Создаем новый объект хэшера
            Update::update(&mut hasher, chunk); // Передаем хэшеру данные для вычисления
            let hash = hasher.clone().finalize(); // Вычисляем хэш-сумму для отданных данных
            let hash = hash.to_vec(); // Переводим в тип вектора
            hex::encode(hash)
        }
    }
}

mod errors {
    // Внутренний модуль для собственных типов ошибок
    use std::error::Error;
    use std::fmt; // Зависимость стандартной библиотеки для отображения данных на экране

    #[derive(Debug, Clone)]
    pub struct DecryptionError(pub String); // Ошибка дешифрования данных

    impl fmt::Display for DecryptionError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            // Метод отображения сведений об ошибке на экране
            write!(f, "Error during decryption chunk: {}", self.0)
        }
    }

    impl Error for DecryptionError {}

    #[derive(Debug, Clone)]
    pub struct InitializationError(pub String); // Ошибка инициализации структуры

    impl fmt::Display for InitializationError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            // Метод отображения сведений об ошибке на экране
            write!(f, "Error during initialization encryptor: {}", self.0)
        }
    }

    impl Error for InitializationError {}

    #[derive(Debug, Clone)]
    pub struct GammaRegenerationError(pub String); // Ошибка регенерации гаммы

    impl fmt::Display for GammaRegenerationError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            // Метод отображения сведений об ошибке на экране
            write!(f, "Error during gamma regeneration: {}", self.0)
        }
    }

    impl Error for GammaRegenerationError {}
}
