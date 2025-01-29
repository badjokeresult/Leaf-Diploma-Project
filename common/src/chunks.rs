use std::cmp::{max, min}; // Зависимость стандартной библиотеки для вычисления размера блока

use reed_solomon_erasure::{galois_8, ReedSolomon}; // Внешняя зависимость для создания блоков по схеме Рида-Соломона
use rayon::prelude::*; // Внешняя зависимость для параллельной обработки блоков

use consts::*; // Внутренний модуль с константами
use errors::*; // Внутренний модуль со специфическими ошибками

mod consts { // Модуль с константами
    pub const MIN_BLOCK_SIZE: usize = 64; // Минимальный размер блока - 64 байта
    pub const MAX_BLOCK_SIZE: usize = 4 * 1024 * 1024 * 1024; // Максимальный размер блока - 4 гигабайта
    pub const GROWTH_FACTOR: f64 = 0.5_f64; // Коэффициент роста - 0.5

    #[cfg(target_pointer_width = "64")]
    pub const ALIGNMENT: usize = 64; // Если система 64-битная - выравнивание по 64

    #[cfg(target_pointer_width = "32")]
    pub const ALIGNMENT: usize = 32; // Если система 32-битная - выравнивание по 32
}

pub trait SecretSharer { // Трейт, которому должна удовлетворять структура
    fn split_into_chunks(&self, secret: &[u8]) -> Result<Vec<Vec<u8>>, DataSplittingError>; // Метод для разбиения файлов на куски
    fn recover_from_chunks(&self, blocks: Vec<Vec<u8>>) -> Result<Vec<u8>, DataRecoveringError>; // Метод восстановления файлов из блоков
}

pub struct ReedSolomonSecretSharer; // Структура схемы Рида-Соломона

impl ReedSolomonSecretSharer {
    pub fn new() -> Result<ReedSolomonSecretSharer, InitializationError> { // Метод создания нового экземпляра структуры
        Ok(ReedSolomonSecretSharer{}) // Создание нового экземпляра структуры
    }

    fn calc_block_size(&self, file_size: usize) -> usize { // Метод рассчета размера блока
        let bs = MIN_BLOCK_SIZE as f64 * ((file_size as f64 / MIN_BLOCK_SIZE as f64).powf(GROWTH_FACTOR));
        let bs = max(MIN_BLOCK_SIZE, min(bs as usize, MAX_BLOCK_SIZE));
        let bs = ((bs + ALIGNMENT - 1) / ALIGNMENT) * ALIGNMENT;
        bs
    }

    fn calc_amount_of_blocks(file_size: usize, block_size: usize) -> usize { // Метод вычисления количества блоков
        (file_size + block_size - 1) / block_size // Вычисление количества блоков
    }
}

impl SecretSharer for ReedSolomonSecretSharer { // Реализация трейта
    fn split_into_chunks(&self, secret: &[u8]) -> Result<Vec<Vec<u8>>, DataSplittingError> { // Метод разбиения файла на блоки
        let block_size = self.calc_block_size(secret.len()); // Получение размера блока
        let amount_of_blocks = Self::calc_amount_of_blocks(secret.len(), block_size); // Получение количества блоков
        let mut buf = vec![0u8; block_size * amount_of_blocks]; // Создание буфера для хранения блоков
        for i in secret {
            buf.push(*i); // Перемещение байтов файла в буфер
        }

        let amount_of_recovers = amount_of_blocks; // Количество блоков восстановления равно количеству блоков данных
        let encoder: ReedSolomon<galois_8::Field> = match ReedSolomon::new(amount_of_blocks, amount_of_recovers) {
            Ok(e) => e,
            Err(e) => {
                return Err(DataSplittingError(e.to_string()));
            },
        }; // Создание кодировщика схемы Рида-Соломона
        let mut blocks = vec![]; // Вектор для хранения блоков
        let blocks_chunks = buf
            .par_iter()
            .chunks(block_size)
            .map(|x| {
                let mut v = vec![];
                for i in x {
                    v.push(i.clone());
                }
                v
            })
            .collect::<Vec<_>>(); // Параллельная обработка и перемещение всех байтов в векторы
        for chunk in blocks_chunks {
            blocks.push(chunk); // Заполнение основного буфера
        }

        blocks.append(&mut vec![vec![0u8; block_size]; amount_of_recovers]); // Выделение места в векторе блоков для блоков восстановления
        if blocks.len() < amount_of_blocks * 2 {
            eprintln!("ERROR BLOCKS_LEN < amount_of_blocks * 2");
            panic!(); // Ошибка, если длина вектора меньше двух длин количества блоков
        }

        encoder.encode(&mut blocks).unwrap(); // Создание блоков восстановления при помощи кодировщика
        Ok(blocks)
    }

    fn recover_from_chunks(&self, blocks: Vec<Vec<u8>>) -> Result<Vec<u8>, DataRecoveringError> { // Метод восстановления файла из блоков
        let mut full_data = blocks.par_iter().cloned().map(Some).collect::<Vec<_>>(); // Все блоки оборачиваются в Option
        let (data_len, recovery_len) = (full_data.len() / 2, full_data.len() / 2); // Получение длин данных и восстановления

        let decoder: ReedSolomon<galois_8::Field> = ReedSolomon::new(data_len, recovery_len).unwrap(); // Создание декодера Рида-Соломона
        decoder.reconstruct_data(&mut full_data).unwrap(); // Восстановление данных из блоков восстановления, если каких-то данных нет

        let content = full_data[..data_len].par_iter().cloned().filter_map(|x| x).collect::<Vec<_>>(); // Очистка пустых значений
        let mut secret = vec![]; // Результирующий вектор
        for i in 0..data_len {
            let mut value = content[i].clone();
            secret.append(&mut value); // Перемещение данных в вектор
        }

        let secret = match secret.iter().position(|x| 0u8.eq(x)) {
            Some(p) => secret.split_at(p).0.to_vec(),
            None => secret,
        }; // Удаление нулей в конце последовательности

        Ok(secret)
    }
}

mod errors { // Модуль с ошибками
    use std::fmt;
    use std::fmt::Formatter;

    #[derive(Debug, Clone)]
    pub struct DataSplittingError(pub String); // Тип ошибки разбиения файла

    impl fmt::Display for DataSplittingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result { // Метод отображения сведений об ошибке на экране
            write!(f, "Error attempting to split a data into chunks: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct DataRecoveringError(pub String); // Тип ошибки восстановления файла

    impl fmt::Display for DataRecoveringError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result { // Метод отображения сведений об ошибке на экране
            write!(f, "Error recovering data from chunks: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct InitializationError(pub String); // Тип ошибки инициализации структуры разбиения файла

    impl fmt::Display for InitializationError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result { // Метод отображения сведений об ошибке на экране
            write!(f, "Error initializing data: {}", self.0)
        }
    }
}

#[cfg(test)]
mod tests { // Модуль юнит-тестирования

}
