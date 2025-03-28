use std::{collections::HashMap, path::PathBuf}; // Зависимость стандартной библиотеки для работы с файловыми путями

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use tokio::fs; // Внешняя зависимость для работы с файловыми операциями асинхронно
use uuid::Uuid; // Внешняя зависимость для генерации UUID

use consts::*; // Внутренний модуль с константами
use errors::*; // Внутренний модуль с составными типами ошибок

mod consts {
    // Модуль с константами
    pub const MAX_OCCUPIED_SPACE: usize = 10 * 1024 * 1024 * 1024; // Максимальный размер хранилища сервера - 10 Гб
}

pub trait ServerStorage {
    // Трейт серверного хранилища
    async fn save(&mut self, hash: &str, data: &[u8]) -> Result<(), SavingDataError>; // Шаблон метода сохранения данных
    async fn get(&mut self, hash: &str) -> Result<Vec<u8>, RetrievingDataError>; // Шаблон метода получения данных
    fn can_save(&self) -> bool; // Шаблон метода проверки возможности сохранения
    async fn shutdown(self, path: PathBuf) -> Result<(), Box<dyn std::error::Error>>;
}

#[derive(Clone, Serialize, Deserialize)]
struct UdpServerStorageState {
    pub hashes: HashMap<String, PathBuf>,
    pub size: usize,
}

#[derive(Clone)]
pub struct UdpServerStorage {
    // Структура серверного хранилища
    path: PathBuf, // Поле со значением пути хранилища
    state: UdpServerStorageState,
}

impl UdpServerStorageState {
    pub async fn new(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        if path.exists() {
            let content = fs::read(path).await?;
            let obj = serde_json::from_slice(&BASE64.decode(&content)?)?;
            return Ok(obj);
        }
        Ok(UdpServerStorageState {
            hashes: HashMap::new(),
            size: 0,
        })
    }

    pub async fn shutdown(self, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        fs::write(path, BASE64.encode(&serde_json::to_vec(&self)?).as_bytes()).await?;
        Ok(())
    }
}

impl UdpServerStorage {
    // Реализация структуры
    pub async fn new(
        storage_path: PathBuf,
        state_path: &PathBuf,
    ) -> Result<UdpServerStorage, Box<dyn std::error::Error>> {
        // Конструктор
        Ok(UdpServerStorage {
            path: storage_path,
            state: UdpServerStorageState::new(&state_path).await?,
        })
    }

    fn get_occupied_space(&self) -> usize {
        // Метод расчета текущего занятого хранилищем места на диске
        self.state.size
    }

    fn is_hash_presented(&self, hash: &str) -> bool {
        // Метод вычисления хэш-сумм всех файлов в директории
        self.state.hashes.contains_key(hash) // В противном случае возвращаем ошибку
    }
}

impl ServerStorage for UdpServerStorage {
    // Реализация трейта для структуры
    async fn save(&mut self, hash: &str, data: &[u8]) -> Result<(), SavingDataError> {
        // Реализация метода сохранения данных на диске
        let hash = String::from(hash); // Переводим хэш в String

        if self.is_hash_presented(&hash) {
            // Если такой хэш уже представлен в хранилище
            return Err(SavingDataError(format!(
                "Hash {} already presents file",
                hash,
            ))); // Возвращаем ошибку
        }

        let filename = self.path.join(format!("{}.bin", Uuid::new_v4())); // Создаем имя нового файла при помощи UUIDv4
        fs::write(&filename, data)
            .await
            .map_err(|e| SavingDataError(e.to_string()))?; // Записываем данные в файл

        self.state.hashes.insert(hash, filename);
        Ok(())
    }

    async fn get(&mut self, hash: &str) -> Result<Vec<u8>, RetrievingDataError> {
        // Реализация метода получения данных из хранилища
        if self.is_hash_presented(hash) {
            // Если такой хэш есть в хранилище
            let path = self.state.hashes.remove(hash).map_or(
                Err(RetrievingDataError(String::from("No such hash was found"))),
                |x| Ok(x),
            )?;
            let data = fs::read(&path)
                .await
                .map_err(|e| RetrievingDataError(e.to_string()))?;
            if let Err(e) = fs::remove_file(&path).await {
                eprintln!("Error removing file {}: {}", path.display(), e.to_string());
            }
            return Ok(data);
        }
        Err(RetrievingDataError(String::from("No such hash was found")))
    }

    fn can_save(&self) -> bool {
        // Реализация метода проверки возможности сохранения файла
        self.get_occupied_space() < MAX_OCCUPIED_SPACE
    }

    async fn shutdown(self, path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        self.state.shutdown(&path).await
    }
}

mod errors {
    // Модуль с составными типами ошибок
    use std::error::Error; // Зависимость стандартной библиотеки для работы с трейтом ошибок
    use std::fmt; // Зависимость стандартной библиотеки для работы с форматированием

    #[derive(Debug, Clone)]
    pub struct SavingDataError(pub String); // Тип ошибки невозможности сохранения файла

    impl fmt::Display for SavingDataError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "Error saving data: {}", self.0)
        }
    }

    impl Error for SavingDataError {}

    #[derive(Debug, Clone)]
    pub struct RetrievingDataError(pub String); // Тип ошибки невозможности получения данных

    impl fmt::Display for RetrievingDataError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Error retrieving data: {}", self.0)
        }
    }

    impl Error for RetrievingDataError {}
}
