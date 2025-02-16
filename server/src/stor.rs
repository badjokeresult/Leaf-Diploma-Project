use std::path::PathBuf; // Зависимость стандартной библиотеки для работы с файловыми путями

use tokio::{fs, task}; // Внешняя зависимость для работы с файловыми операциями асинхронно
use uuid::Uuid; // Внешняя зависимость для генерации UUID
use walkdir::WalkDir; // Внешняя зависимость для рекурсивного обхода директорий

use common::{Hasher, StreebogHasher}; // Зависимость внутренней библиотеки для вычисления хэш-сумм

use consts::*; // Внутренний модуль с константами
use errors::*; // Внутренний модуль с составными типами ошибок

mod consts {
    // Модуль с константами
    pub const MAX_OCCUPIED_SPACE: usize = 10 * 1024 * 1024 * 1024; // Максимальный размер хранилища сервера - 10 Гб
}

pub trait ServerStorage {
    // Трейт серверного хранилища
    async fn save(&self, hash: &str, data: &[u8]) -> Result<(), SavingDataError>; // Шаблон метода сохранения данных
    async fn get(&self, hash: &str) -> Result<Vec<u8>, RetrievingDataError>; // Шаблон метода получения данных
    async fn can_save(&self) -> Result<bool, SavingDataError>; // Шаблон метода проверки возможности сохранения
}

pub struct UdpServerStorage {
    // Структура серверного хранилища
    hasher: StreebogHasher, // Поле с вычислителем хэша
    path: PathBuf,          // Поле со значением пути хранилища
}

impl UdpServerStorage {
    // Реализация структуры
    pub fn new(path: PathBuf) -> UdpServerStorage {
        // Конструктор
        UdpServerStorage {
            hasher: StreebogHasher::new(),
            path,
        }
    }

    async fn get_occupied_space(&self) -> Result<usize, RetrievingDataError> {
        // Метод расчета текущего занятого хранилищем места на диске
        let path = self.path.clone();

        let size = task::spawn_blocking(move || {
            // Запускаем блокирующую асинхронную задачу для прохода по директории
            let mut total_size = 0; // Счетчик занятого места в байтах
            for entry in WalkDir::new(&path) {
                let entry = entry.map_err(|e| RetrievingDataError(e.to_string()))?; // Пытаемся получить объект в директории
                if entry.path().is_file() {
                    // Проверяем, что объект является файлом
                    if let Ok(meta) = std::fs::metadata(entry.path()) {
                        // Пытаемся получить сведения о файле
                        total_size += meta.len() as usize; // Добавляем размер файла к общему счетчику
                    }
                }
            }
            Ok(total_size) // Возвращаем счетчик
        })
        .await
        .map_err(|e| RetrievingDataError(e.to_string()))??;

        Ok(size) // Возвращаем размер директории
    }

    async fn search_for_hash(&self, hash: &str) -> Result<(PathBuf, Vec<u8>), RetrievingDataError> {
        // Метод вычисления хэш-сумм всех файлов в директории
        for entry in WalkDir::new(&self.path) {
            let entry = entry.map_err(|e| RetrievingDataError(e.to_string()))?; // Пытаемся получить объект директории
            if entry.path().is_file() {
                // Проверяем, что объект является файлом
                let content = fs::read(entry.path()).await.unwrap(); // Читаем содержимое файла
                let h = self.hasher.calc_hash_for_chunk(&content); // Вычисляем хэш-сумму данных
                if hash.eq(&h) {
                    // Если вычисленный хэш равен эталонному
                    return Ok((PathBuf::from(entry.path()), content)); // Возвращаем путь к файлу и его содержимое
                }
            }
        }
        Err(RetrievingDataError(format!("hash not found: {}", hash))) // В противном случае возвращаем ошибку
    }
}

impl ServerStorage for UdpServerStorage {
    // Реализация трейта для структуры
    async fn save(&self, hash: &str, data: &[u8]) -> Result<(), SavingDataError> {
        // Реализация метода сохранения данных на диске
        let hash = String::from(hash); // Переводим хэш в String

        if let Ok(_) = self.search_for_hash(&hash).await {
            // Если такой хэш уже представлен в хранилище
            return Err(SavingDataError(format!(
                "Hash {} already presents file",
                hash,
            ))); // Возвращаем ошибку
        }

        let filename = self.path.join(format!("{}.bin", Uuid::new_v4())); // Создаем имя нового файла при помощи UUIDv4
        fs::write(&filename, data)
            .await
            .map_err(|e| SavingDataError(e.to_string())) // Записываем данные в файл
    }

    async fn get(&self, hash: &str) -> Result<Vec<u8>, RetrievingDataError> {
        // Реализация метода получения данных из хранилища
        if let Ok((p, c)) = self.search_for_hash(hash).await {
            // Если такой хэш есть в хранилище
            fs::remove_file(&p)
                .await
                .map_err(|e| RetrievingDataError(e.to_string()))?; // Удаляем файл
            return Ok(c); // Возвращаем его содержимое
        }
        Err(RetrievingDataError(format!(
            // Иначе возвращаем ошибку
            "No data for hash sum {}",
            hash,
        )))
    }

    async fn can_save(&self) -> Result<bool, SavingDataError> {
        // Реализация метода проверки возможности сохранения файла
        Ok(self
            .get_occupied_space()
            .await
            .map_err(|e| SavingDataError(e.to_string()))?
            < MAX_OCCUPIED_SPACE)
    }
}

mod errors {
    use std::error::Error;
    use std::fmt;

    #[derive(Debug, Clone)]
    pub struct SavingDataError(pub String);

    impl fmt::Display for SavingDataError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "Error saving data: {}", self.0)
        }
    }

    impl Error for SavingDataError {}

    #[derive(Debug, Clone)]
    pub struct RetrievingDataError(pub String);

    impl fmt::Display for RetrievingDataError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Error retrieving data: {}", self.0)
        }
    }

    impl Error for RetrievingDataError {}
}
