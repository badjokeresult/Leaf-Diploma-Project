use std::cmp::{max, min}; // Зависимость стандартной библиотеки для вычисления размера блока

use rayon::prelude::*; // Внешняя зависимость для параллельной обработки блоков
use reed_solomon_erasure::{galois_8, ReedSolomon}; // Внешняя зависимость для создания блоков по схеме Рида-Соломона

use consts::*; // Внутренний модуль с константами
use errors::*; // Внутренний модуль со специфическими ошибками

pub struct ReedSolomonChunks {
    // Структура абстракции блоков данных и восстановления
    data: Vec<Vec<u8>>,     // Блоки данных
    recovery: Vec<Vec<u8>>, // Блоки восстановления
}

impl ReedSolomonChunks {
    // Реализация структуры
    pub fn new(data: Vec<Vec<u8>>, recovery: Vec<Vec<u8>>) -> ReedSolomonChunks {
        // Конструктор структуры
        ReedSolomonChunks { data, recovery }
    }

    pub fn deconstruct(self) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
        // Уничтожение ссылки на объект и возврат всех блоков из полей
        (self.data, self.recovery)
    }
}

mod consts {
    // Модуль с константами
    pub const MIN_BLOCK_SIZE: usize = 64; // Минимальный размер блока - 64 байта
    pub const MAX_BLOCK_SIZE: usize = 65251; // Максимальный размер блока - 65251 байта, т.к. максимальный размер нагрузки UDP пакета - 65535 байт, из которых вычитаем 256 байт хэша и 28 байт заголовков (IP + UDP)
    pub const GROWTH_FACTOR: f64 = 0.5_f64; // Коэффициент роста - 0.5

    #[cfg(target_pointer_width = "64")]
    pub const ALIGNMENT: usize = 64; // Если система 64-битная - выравнивание по 64

    #[cfg(target_pointer_width = "32")]
    pub const ALIGNMENT: usize = 32; // Если система 32-битная - выравнивание по 32
}

pub trait SecretSharer {
    // Трейт, которому должна удовлетворять структура
    fn split_into_chunks(&self, secret: &[u8]) -> Result<ReedSolomonChunks, DataSplittingError>; // Метод для разбиения файлов на куски
    fn recover_from_chunks(
        &self,
        blocks: ReedSolomonChunks,
    ) -> Result<Vec<u8>, DataRecoveringError>; // Метод восстановления файлов из блоков
}

pub struct ReedSolomonSecretSharer; // Структура схемы Рида-Соломона

impl ReedSolomonSecretSharer {
    pub fn new() -> Result<ReedSolomonSecretSharer, InitializationError> {
        // Конструктор структуры схемы Рида-Соломона
        Ok(ReedSolomonSecretSharer {})
    }

    fn calc_block_size(&self, file_size: usize) -> usize {
        // Метод рассчета размера блока
        let bs = MIN_BLOCK_SIZE as f64
            * ((file_size as f64 / MIN_BLOCK_SIZE as f64).powf(GROWTH_FACTOR));
        let bs = max(MIN_BLOCK_SIZE, min(bs as usize, MAX_BLOCK_SIZE));
        let bs = ((bs + ALIGNMENT - 1) / ALIGNMENT) * ALIGNMENT;
        bs
    }

    fn calc_amount_of_blocks(file_size: usize, block_size: usize) -> usize {
        // Метод вычисления количества блоков
        (file_size + block_size - 1) / block_size
    }
}

impl SecretSharer for ReedSolomonSecretSharer {
    // Реализация трейта
    fn split_into_chunks(&self, secret: &[u8]) -> Result<ReedSolomonChunks, DataSplittingError> {
        // Метод разбиения файла на блоки
        let block_size = self.calc_block_size(secret.len()); // Получение размера блока
        let amount_of_blocks = Self::calc_amount_of_blocks(secret.len(), block_size); // Получение количества блоков
        let amount_of_recovers = amount_of_blocks; // Количество блоков восстановления равно количеству блоков данных
        let blocks = secret
            .par_iter()
            .cloned()
            .chunks(block_size)
            .collect::<Vec<_>>(); // Перемещение байтов файла в буфер

        let encoder: ReedSolomon<galois_8::Field> =
            ReedSolomon::new(amount_of_blocks, amount_of_recovers)
                .map_err(|e| DataSplittingError(e.to_string()))?; // Создание кодировщика схемы Рида-Соломона
        let mut parity = vec![vec![0u8; block_size]; amount_of_recovers];
        encoder
            .encode_sep(&blocks, &mut parity)
            .map_err(|e| DataSplittingError(e.to_string()))?; // Создание блоков восстановления при помощи кодировщика

        Ok(ReedSolomonChunks::new(blocks, parity)) // Возврат структуры с блоками
    }

    fn recover_from_chunks(
        &self,
        blocks: ReedSolomonChunks,
    ) -> Result<Vec<u8>, DataRecoveringError> {
        // Метод восстановления файла из блоков
        let (mut data, mut recovery) = blocks.deconstruct();
        data.append(&mut recovery);
        let mut full_data = data.par_iter().cloned().map(Some).collect::<Vec<_>>(); // Все блоки оборачиваются в Option
        let (data_len, recovery_len) = (full_data.len() / 2, full_data.len() / 2); // Получение длин данных и восстановления

        let decoder: ReedSolomon<galois_8::Field> = ReedSolomon::new(data_len, recovery_len)
            .map_err(|e| DataRecoveringError(e.to_string()))?; // Создание декодера Рида-Соломона
        decoder
            .reconstruct(&mut full_data)
            .map_err(|e| DataRecoveringError(e.to_string()))?; // Восстановление данных из блоков восстановления, если каких-то данных нет

        let content = full_data[..data_len]
            .par_iter()
            .cloned()
            .filter_map(|x| x)
            .flatten()
            .collect::<Vec<_>>();

        match content.iter().position(|x| 0u8.eq(x)) {
            Some(p) => Ok(content.split_at(p).0.to_vec()),
            None => Ok(content),
        } // Удаление нулей в конце последовательности
    }
}

mod errors {
    // Модуль с ошибками
    use std::error::Error;
    use std::fmt;
    use std::fmt::Formatter;

    #[derive(Debug, Clone)]
    pub struct DataSplittingError(pub String); // Тип ошибки разбиения файла

    impl fmt::Display for DataSplittingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            // Метод отображения сведений об ошибке на экране
            write!(
                f,
                "Error attempting to split a data into chunks: {}",
                self.0
            )
        }
    }

    impl Error for DataSplittingError {}

    #[derive(Debug, Clone)]
    pub struct DataRecoveringError(pub String); // Тип ошибки восстановления файла

    impl fmt::Display for DataRecoveringError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            // Метод отображения сведений об ошибке на экране
            write!(f, "Error recovering data from chunks: {}", self.0)
        }
    }

    impl Error for DataRecoveringError {}

    #[derive(Debug, Clone)]
    pub struct InitializationError(pub String); // Тип ошибки инициализации структуры разбиения файла

    impl fmt::Display for InitializationError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            // Метод отображения сведений об ошибке на экране
            write!(f, "Error initializing data: {}", self.0)
        }
    }

    impl Error for InitializationError {}
}

#[cfg(test)]
mod tests { // Модуль юнит-тестирования
}
