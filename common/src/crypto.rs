use std::env; // Зависимость стандартной библиотеки для получения системных переменных среды
use std::path::PathBuf; // Зависимость стандартной библиотеки для использования структуры по работе с файловыми путями

use tokio::fs; // Внешняя зависимость для асинхронной работы операциями ввода-вывода файловой системы

use argon2::Argon2; // Внешняя зависимость для создания ключа из гаммы и пароля

use rand::{Rng, rngs::OsRng}; // Внешняя зависимость для генерации псевдослучайных последовательностей

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64}; // Внешняя зависимость для кодирования и декодирования по алгоритму Base64

use serde::{Serialize, Deserialize}; // Внешняя зависимость для сериализации и десериализации структур

use kuznyechik::{Kuznyechik, Key, Block};
use kuznyechik::cipher::{BlockDecrypt, BlockEncrypt, KeyInit}; // Внешние зависимости для работы с симметричным шифром "Кузнечик (ГОСТ Р 34.12-2018)"

use streebog::Digest;
use streebog::digest::Update; // Внешние зависимости для работы с алгоритмом вычисления хэш-сумм "Стрибог" (ГОСТ Р 34.11-2012)

use errors::*; // Внутренняя зависимость модуля для использования собственных типов ошибок

#[derive(Serialize, Deserialize)] // Использование сериализации и десериализации для данной структуры
struct EncryptionMetadata { // Структура для хранения гаммы и соли для использования в шифровании "Кузнечиком"
    gamma: Vec<u8>,  // Закодированная по Base64 гамма
    salt: Vec<u8>,   // Закодированная по Base64 соль
}

pub trait Encryptor { // Трейт для структур, реализующих шифрование
    fn encrypt_chunk(&self, chunk: &mut [u8]) -> Result<(), EncryptionError>; // Прототип метода шифрования массива данных
    fn decrypt_chunk(&self, chunk: &mut [u8]) -> Result<(), DecryptionError>; // Прототип метода дешифрования массива данных
}

pub struct KuznechikEncryptor { // Структура, реализующая шифрование по ГОСТ Р 34.12-2018 "Кузнечик"
    cipher: Kuznyechik, // Ключ шифрования
    gamma: Vec<u8>, // Гамма для шифрования
    metadata_path: PathBuf, // Путь к файлу с метаданными
}

impl KuznechikEncryptor {
    #[cfg(target_os = "linux")]
    pub async fn new(password: &str) -> Result<Self, InitializationError> { // Метод создания нового экземпляра структуры, получающая на вход строку с паролем (реализация для Linux)
        let username = env::var("USER")?; // Получаем имя текущего пользователя из переменной среды
        let mut authenticator = pam::Authenticator::with_password("system-auth")?; // Получаем экземпляр PAM-аутентификатора
        authenticator.get_handler().set_credentials(&username, password); // Отдаем аутентификатору имя пользователя и пароль

        if authenticator.authenticate().is_err() { // Проверяем правильность введенных данных
            return Err("Invalid password".into()); // Если аутентификация провалена, возвращаем ошибку
        }

        Self::initialize(password).await // Запускаем метод инициализации ключа и гаммы
    }

    #[cfg(target_os = "windows")]
    pub async fn new(password: &str) -> Result<Self, InitializationError> { // Метод создания нового экземпляра структуры, получающая на вход строку с паролем (реализация для Windows)
        // Загружаем необходимые зависимости
        use windows_sys::Win32::Security::LogonUserW;
        use windows_sys::Win32::Security::LOGON32_LOGON_INTERACTIVE;
        use windows_sys::Win32::Security::LOGON32_PROVIDER_DEFAULT;

        use std::ptr::null_mut;

        let username = env::var("USERNAME").unwrap(); // Получаем имя текущего пользователя при помощи переменной среды
        let mut token_handle = null_mut(); // Создаем пустой указатель для токена авторизации

        let username_wide: Vec<u16> = username.encode_utf16().chain(std::iter::once(0)).collect(); // Получаем имя пользователя в кодировке UTF-16
        let password_wide: Vec<u16> = password.encode_utf16().chain(std::iter::once(0)).collect(); // Получаем пароль в кодировке UTF-16
        let domain_wide: Vec<u16> = ".".encode_utf16().chain(std::iter::once(0)).collect(); // Получаем имя домена в кодировке UTF-16

        // Выполняем проверку правильности введенных данных
        let result = unsafe {
            LogonUserW(
                username_wide.as_ptr(),
                domain_wide.as_ptr(),
                password_wide.as_ptr(),
                LOGON32_LOGON_INTERACTIVE,
                LOGON32_PROVIDER_DEFAULT,
                &mut token_handle
            )
        };

        if result != 0 {
            return Err(InitializationError(String::from("Invalid password"))); // Если данные неверны - возвращаем ошибку
        }

        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(token_handle) // Удаляем указатель на токен авторизации
        };

        Self::initialize(password).await // Запускаем метод инициализации ключа и гаммы
    }

    async fn initialize(password: &str) -> Result<Self, InitializationError> { // Метод инициализации полей структуры гаммой и ключом
        let metadata_path = Self::get_metadata_path().await?; // Получаем путь до файла с метаданными при помощи метода

        let (gamma, salt) = if metadata_path.exists() {
            // Если файл с метаданными существует, то читаем данные из него и идем дальше
            let metadata: EncryptionMetadata = Self::load_metadata(&metadata_path).await?;
            (
                BASE64.decode(&metadata.gamma).unwrap(),
                BASE64.decode(&metadata.salt).unwrap()
            )
        } else {
            // Если такого файла нет, то создаем новые гамму и соль
            let mut gamma = vec![0u8; 16]; // Создаем 128-битный буфер для гаммы
            let mut salt = vec![0u8; 32]; // Создаем 256-битный буфер для соли
            OsRng.fill(&mut gamma[..]); // Заполняем буфер гаммы случайными данными
            OsRng.fill(&mut salt[..]); // Заполняем буфер соли случайными данными

            let metadata = EncryptionMetadata {
                gamma: BASE64.encode(&gamma).into_bytes(),
                salt: BASE64.encode(&salt).into_bytes(),
            }; // Создаем новый экземпляр структуры и заполняем его поля соответствующими буферами
            Self::save_metadata(&metadata_path, &metadata).await?; // Сохраняем метаданные в файл

            (gamma, salt)
        };

        let config = Argon2::default(); // Создание конфигурации для создания ключа шифрования
        let mut key = vec![0u8; 32]; // Создаем буфер для ключа
        config.hash_password_into(password.as_bytes(), &salt, &mut key).unwrap(); // Создаем ключ и записываем его в буфер

        let cipher_key = Key::from_slice(&key); // Создаем объект ключа шифрования из буфера
        let cipher = Kuznyechik::new(&cipher_key); // Создаем объект шифратора

        Ok(Self {
            cipher,
            gamma,
            metadata_path,
        }) // Создаем и возвращаем новый экземпляр структуры
    }

    async fn get_metadata_path() -> Result<PathBuf, InitializationError> { // Метод получения пути файла с метаданными
        #[cfg(target_os = "linux")]
        let base_path = PathBuf::from("/etc"); // Получаем полный путь до директории с конфигурациями приложений в домашнем каталоге пользователя (реализация для Linux)

        #[cfg(target_os = "windows")]
        let base_path = PathBuf::from(env::var("APPDATA").unwrap()); // Получаем полный путь до директории приложений при помощи переменной среды (реализация для Windows)

        // Создаем директорию нашего приложения
        let app_dir = base_path.join("leaf");
        fs::create_dir_all(&app_dir).await.unwrap();

        Ok(app_dir.join("metadata.json")) // Возвращаем полный путь до файла с метаданными
    }

    async fn load_metadata(path: &PathBuf) -> Result<EncryptionMetadata, InitializationError> { // Метод получения данных из файла метаданных
        let data = fs::read_to_string(path).await.unwrap(); // Получаем строковые данные из файла
        Ok(serde_json::from_str(&data).unwrap()) // Десериализуем прочитанный JSON-текст в структуру и возвращаем его
    }

    async fn save_metadata(path: &PathBuf, metadata: &EncryptionMetadata) -> Result<(), InitializationError> { // Метод записи данных в файл метаданных
        let data = serde_json::to_string_pretty(metadata).unwrap(); // Сериализуем объект в JSON-текст с пробельными символами
        fs::write(path, data).await.unwrap(); // Записываем текст в файл
        Ok(())
    }

    pub async fn regenerate_gamma(&mut self) -> Result<(), GammaRegenerationError> { // Метод регенерации гаммы
        OsRng.fill(&mut self.gamma[..]); // Гамма заполняется новыми случайными данными

        let metadata = EncryptionMetadata {
            gamma: BASE64.encode(&self.gamma).into_bytes(),
            salt: BASE64.decode(&Self::load_metadata(&self.metadata_path).await.unwrap().salt).unwrap(),
        }; // Создается новый экземпляр структуры метаданных, все поля предварительно кодируются в Base64

        Self::save_metadata(&self.metadata_path, &metadata).await.unwrap(); // Сохраняем новые метаданные
        Ok(())
    }
}

impl Encryptor for KuznechikEncryptor { // Блок реализации трейта для структуры
    fn encrypt_chunk(&self, chunk: &mut [u8]) -> Result<(), EncryptionError> { // Метод шифрования данных на месте
        let mut padded_data = chunk.to_vec(); // Копируем данные в новую переменную
        while padded_data.len() % 16 != 0 { // Выравниваем данные по 16 байт
            padded_data.push(0);
        }

        let mut result = Vec::with_capacity(padded_data.len()); // Создаем буфер для зашифрованных данных

        for c in padded_data.chunks(16) { // Для каждого блока по 16 байт
            let mut block = [0u8; 16]; // Выделяем место для блока
            block.copy_from_slice(c); // Копируем блок во временный буфер

            for (b, g) in block.iter_mut().zip(self.gamma.iter()) {
                *b ^= g; // Выполняем XOR для блока с гаммой
            }

            let mut block = Block::clone_from_slice(&block); // Создаем структуру блока, с которой умеет работать экземпляр шифратора
            self.cipher.encrypt_block(&mut block); // Выполняем шифрование
            result.extend_from_slice(&block); // Записываем защифрованные данные в конец результирующего буфера
        }

        chunk.copy_from_slice(&result); // Заполняем массив со входа новыми данными
        Ok(())
    }

    fn decrypt_chunk(&self, chunk: &mut [u8]) -> Result<(), DecryptionError> { // Метод дешифрования данных на месте
        // Если данные не выравнены по 16 байт, то возвращаем ошибку
        if chunk.len() % 16 != 0 {
            return Err(DecryptionError(String::from("Invalid encrypted data length")));
        }

        let mut result = Vec::with_capacity(chunk.len()); // Создаем буфер для хранения дешифрованных данных

        for c in chunk.chunks(16) { // Для каждого блока по 16 байт
            let mut block = [0u8; 16]; // Создаем временный буфер для блока
            block.copy_from_slice(c); // Записываем данные во временный буфер

            let mut block = Block::clone_from_slice(&block); // Создаем объект блока для дешифратора
            self.cipher.decrypt_block(&mut block); // Дешифруем данные

            for (b, g) in block.iter_mut().zip(self.gamma.iter()) {
                *b ^= g; // Выполняем XOR с гаммой для блока
            }

            result.extend_from_slice(&block); // Записываем дешифрованные данные в конец общего буфера
        }

        // Remove padding
        while result.last() == Some(&0) {
            result.pop(); // Удаляем нулевые байты в конце буфера
        }

        chunk.copy_from_slice(&result); // Записываем новые данные в массив входа
        Ok(())
    }
}

pub trait Hasher { // Трейт для структур, реализующий вычисление хэш-суммы
    fn calc_hash_for_chunk(&self, chunk: &[u8]) -> Vec<u8>; // Метод вычисления хэш-суммы
}

pub struct StreebogHasher; // Структура для вычисления хэш-суммы

impl StreebogHasher {
    pub fn new() -> StreebogHasher { // Метод создания нового экземпляра структуры
        StreebogHasher {  }
    }
}

impl Hasher for StreebogHasher { // Реализация трейта
    fn calc_hash_for_chunk(&self, chunk: &[u8]) -> Vec<u8> { // Метод вычисления хэш-суммы
        let mut hasher = streebog::Streebog256::new(); // Создаем новый объект хэшера
        Update::update(&mut hasher, chunk); // Передаем хэшеру данные для вычисления
        let hash = hasher.clone().finalize(); // Вычисляем хэш-сумму для отданных данных
        let hash = hash.to_vec(); // Возвращаем как вектор
        hash
    }
}

mod errors { // Внутренний модуль для собственных типов ошибок
    use std::fmt; // Зависимость стандартной библиотеки для отображения данных на экране

    #[derive(Debug, Clone)]
    pub struct EncryptionError(pub String); // Ошибка шифрования данных

    impl fmt::Display for EncryptionError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { // Метод отображения сведений об ошибке на экране
            write!(f, "Error during encryption chunk: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct DecryptionError(pub String); // Ошибка дешифрования данных

    impl fmt::Display for DecryptionError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { // Метод отображения сведений об ошибке на экране
            write!(f, "Error during decryption chunk: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct InitializationError(pub String); // Ошибка инициализации структуры

    impl fmt::Display for InitializationError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { // Метод отображения сведений об ошибке на экране
            write!(f, "Error during initialization encryptor: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct GammaRegenerationError(pub String); // Ошибка регенерации гаммы

    impl fmt::Display for GammaRegenerationError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { // Метод отображения сведений об ошибке на экране
            write!(f, "Error during gamma regeneration: {}", self.0)
        }
    }
}
