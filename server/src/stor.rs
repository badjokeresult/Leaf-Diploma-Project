use std::collections::HashMap; // Зависимость стандартной библиотеки для работы с хэш-таблицами
use std::path::PathBuf; // Зависимость стандартной библиотеки для работы с файловыми путями
                        // Защита БД

use std::sync::Arc;
use tokio::fs; // Внешняя зависимость для асинхронной работы с файловой системой
use tokio::sync::{RwLock, RwLockWriteGuard};
use uuid::Uuid; // Внешняя зависимость для создания UUID
use walkdir::WalkDir; // Внешняя зависимость для прохода по директориям файловой системы // Внешняя зависимость для обеспечения атомарной внутренней изменяемости объектов

use consts::*;
use errors::*; // Внутренняя зависимость для использования составных типов ошибок // Внутренняя зависимость для использования констант

mod consts {
    // Модуль с константами
    pub const MAX_OCCUPIED_SPACE: usize = 10 * 1024 * 1024 * 1024; // Максимальный размер диска - 10 Гб
}

pub trait ServerStorage {
    // Трейт для хранилища сервера
    async fn save(&self, hash: &str, data: &[u8]) -> Result<(), SavingDataError>; // Метод для сохранения данных по хэш-сумме на диске
    async fn get(&self, hash: &str) -> Result<Vec<u8>, RetrievingDataError>; // Метод для получения данных из хранилища
    async fn can_save(&self) -> bool; // Метод для проверки возможности сохранения данных
}

#[derive(Clone)]
pub struct UdpServerStorage {
    // Структура хранилища
    database: Arc<RwLock<HashMap<String, PathBuf>>>, // Хэш-таблица, хранящая хэш-суммы и пути к файлам на диске, выполненная в атомарном исполнении с внутренней изменяемостью
    path: PathBuf,                                   // Путь до хранилища
}

impl UdpServerStorage {
    pub fn new(path: PathBuf) -> UdpServerStorage {
        // Создание нового экземпляра хранилища
        UdpServerStorage {
            database: Arc::new(RwLock::new(HashMap::new())),
            path,
        }
    }

    async fn get_occupied_space(&self) -> Result<usize, RetrievingDataError> {
        // Метод вычисления занятого дискового пространства
        let mut size = 0; // Счетчик занятых байт
        for entry in WalkDir::new(&self.path) {
            // Для каждого объекта по указанному пути
            let entry = match entry {
                // Если удалось получить доступ к объекту
                Ok(entry) => entry, // Читаем его
                Err(e) => return Err(RetrievingDataError(format!("{:?}", e))), // Иначе возвращаем ошибку
            };
            if entry.path().is_file() {
                // Если объект является файлом
                if let Ok(meta) = entry.path().metadata() {
                    // Получаем сведения о файла
                    size += meta.len() as usize; // Получаем размер файла в байтах и добавляем к счетчику
                }
            }
        }
        Ok(size) // Возвращаем счетчик
    }
}

impl ServerStorage for UdpServerStorage {
    async fn save(&self, hash: &str, data: &[u8]) -> Result<(), SavingDataError> {
        let hash = String::from(hash);

        let mut db = self.database.write().await;

        // Проверяем, существует ли уже такой хэш
        if db.contains_key(&hash) {
            return Err(SavingDataError(format!(
                "Hash already presents file {:#?}",
                db.get(&hash).unwrap()
            )));
        }

        // Генерируем имя файла и записываем данные
        let filename = self.path.join(format!("{}.bin", Uuid::new_v4()));
        tokio::fs::write(&filename, data)
            .await
            .map_err(|e| SavingDataError(e.to_string()))?;

        // Добавляем хэш и путь к файлу в хранилище
        db.insert(hash, filename);
        Ok(())
    }

    async fn get(&self, hash: &str) -> Result<Vec<u8>, RetrievingDataError> {
        for key in self.database.read().await.keys() {
            println!("{:#?}", key);
        }
        let mut db: RwLockWriteGuard<'_, HashMap<String, PathBuf>> = self.database.write().await;
        // Метод чтения данных с диска
        if let Some(x) = db.remove(hash) {
            // Если полученный хэш указывает на файл, то удаляем запись из таблицы
            let data = fs::read(x).await.unwrap(); // Читаем файл
            return Ok(data); // Возвращаем содержимое файла
        }
        Err(RetrievingDataError(String::from(
            "No data for such hash sum",
        ))) // Иначе возвращаем ошибку
    }

    async fn can_save(&self) -> bool {
        // Функция проверки возможности сохранения данных на диске
        self.get_occupied_space().await.unwrap() < MAX_OCCUPIED_SPACE
    }
}

mod errors {
    // Модуль с составными типами ошибок
    use std::fmt; // Зависимость стандартной библиотеки для отображения сведений на экране

    #[derive(Debug, Clone)]
    pub struct SavingDataError(pub String); // Ошибка сохранения данных на диске

    impl fmt::Display for SavingDataError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            // Метод отображения сведений об ошибке на экране
            write!(f, "Error saving data: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct RetrievingDataError(pub String); // Ошибка чтения данных с диска

    impl fmt::Display for RetrievingDataError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            // Метод отображения сведений об ошибке на экране
            write!(f, "Error retrieving data: {}", self.0)
        }
    }
}
