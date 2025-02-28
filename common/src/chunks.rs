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
    pub const MAX_BLOCK_SIZE: usize = 1024; // Максимальный размер блока - 65251 байта, т.к. максимальный размер нагрузки UDP пакета - 65535 байт, из которых вычитаем 256 байт хэша и 8 байт заголовков
    pub const GROWTH_FACTOR: f64 = 0.5_f64; // Коэффициент роста - 0.5

    #[cfg(target_pointer_width = "64")]
    pub const ALIGNMENT: usize = 64; // Если система 64-битная - выравнивание по 64

    #[cfg(target_pointer_width = "32")]
    pub const ALIGNMENT: usize = 32; // Если система 32-битная - выравнивание по 32

    pub const MAX_AMOUNT_OF_BLOCKS: usize = 128;
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
}

impl SecretSharer for ReedSolomonSecretSharer {
    // Реализация трейта
    fn split_into_chunks(&self, secret: &[u8]) -> Result<ReedSolomonChunks, DataSplittingError> {
        // Метод разбиения файла на блоки
        let block_size = self.calc_block_size(secret.len()); // Получение размера блока

        let mut blocks = secret
            .par_iter()
            .cloned()
            .chunks(block_size)
            .collect::<Vec<_>>(); // Перемещение байтов файла в буфер
        let blocks_len = blocks.len();
        let last_block_size = blocks.last().unwrap().len();
        if last_block_size != block_size {
            blocks[blocks_len - 1].append(&mut vec![0u8; block_size - last_block_size]);
        }

        let mut parity = vec![vec![0u8; block_size]; blocks.len()];
        let mut i = 0;
        while i < blocks.len() {
            let remaining_blocks = blocks.len() - i;
            let block_size = remaining_blocks.min(MAX_AMOUNT_OF_BLOCKS); // определяем, сколько элементов обрабатывать

            let encoder: ReedSolomon<galois_8::Field> = ReedSolomon::new(block_size, block_size)
                .map_err(|e| DataSplittingError(e.to_string()))?;

            encoder
                .encode_sep(
                    &blocks[i..i + block_size],     // обрабатываем оставшиеся блоки
                    &mut parity[i..i + block_size], // обрабатываем оставшиеся элементы для parity
                )
                .map_err(|e| DataSplittingError(e.to_string()))?;

            i += block_size; // увеличиваем i на количество обработанных блоков
        }
        Ok(ReedSolomonChunks::new(blocks, parity)) // Возврат структуры с блоками
    }

    fn recover_from_chunks(
        &self,
        blocks: ReedSolomonChunks,
    ) -> Result<Vec<u8>, DataRecoveringError> {
        // Метод восстановления файла из блоков
        let (mut data, mut recovery) = blocks.deconstruct();
        let data_len = data.len();

        // Объединяем data и recovery в один массив для восстановления
        data.append(&mut recovery);
        let full_data = data.par_iter().cloned().map(Some).collect::<Vec<_>>();

        let mut result = Vec::with_capacity(data_len);

        // Обрабатываем блоки последовательно, по MAX_AMOUNT_OF_BLOCKS за раз
        let mut i = 0;
        while i < data_len {
            let remaining_blocks = data_len - i;
            let block_size = remaining_blocks.min(MAX_AMOUNT_OF_BLOCKS);

            // // Создаем декодер для текущего набора блоков
            let decoder: ReedSolomon<galois_8::Field> = ReedSolomon::new(block_size, block_size)
                .map_err(|e| DataRecoveringError(e.to_string()))?;
            let mut curr_slice = Vec::with_capacity(block_size * 2);
            let mut tmp_data = full_data[i..block_size + i].to_vec();
            let mut tmp_recv = full_data[data_len + i..block_size + i + data_len].to_vec();
            println!("{} - {}", tmp_data.len(), tmp_recv.len());
            curr_slice.append(&mut tmp_data);
            curr_slice.append(&mut tmp_recv);
            decoder
                .reconstruct_data(&mut curr_slice)
                .map_err(|e| DataRecoveringError(e.to_string()))?;

            result.append(&mut curr_slice[..block_size].to_vec());
            i += block_size;
        }

        // Извлекаем только блоки данных (без блоков восстановления)
        let content = result
            .par_iter()
            .cloned()
            .filter_map(|x| x)
            .flatten()
            .collect::<Vec<_>>();
        println!("{}", content.len());
        // Удаление нулей в конце последовательности
        let content = match content.iter().position(|x| 0u8.eq(x)) {
            Some(p) => content.split_at(p).0.to_vec(),
            None => content,
        };
        println!("{}", content.len());
        Ok(content)
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
