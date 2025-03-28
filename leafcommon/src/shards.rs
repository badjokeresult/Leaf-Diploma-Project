pub mod reed_solomon {
    use std::cmp::{max, min};

    use rayon::prelude::*;
    use reed_solomon_erasure::{galois_8, ReedSolomon}; // Внешняя зависимость для создания блоков по схеме Рида-Соломона

    use super::errors::*;
    use consts::*;

    mod consts {
        // Модуль с константами
        pub const MIN_BLOCK_SIZE: usize = 64; // Минимальный размер блока - 64 байта
        pub const MAX_BLOCK_SIZE: usize = 65216; // Максимальный размер блока - 65251 байта, т.к. максимальный размер нагрузки UDP пакета - 65535 байт, из которых вычитаем 256 байт хэша и 8 байт заголовков
        pub const GROWTH_FACTOR: f64 = 0.5_f64; // Коэффициент роста - 0.5
        pub const ALIGNMENT: usize = 64; // выравнивание по 64 бита
        pub const MAX_AMOUNT_OF_BLOCKS: usize = 128; // Максимальный размер блоков для разделения за одну итерацию
    }

    fn calc_block_size(file_size: usize) -> usize {
        // Метод рассчета размера блока
        let bs = MIN_BLOCK_SIZE as f64
            * ((file_size as f64 / MIN_BLOCK_SIZE as f64).powf(GROWTH_FACTOR));
        let bs = max(MIN_BLOCK_SIZE, min(bs as usize, MAX_BLOCK_SIZE));
        let bs = ((bs + ALIGNMENT - 1) / ALIGNMENT) * ALIGNMENT;
        bs
    }

    pub fn split(
        secret: Vec<u8>,
    ) -> Result<(Vec<Vec<u8>>, Vec<Vec<u8>>), Box<dyn std::error::Error>> {
        // Метод разбиения файла на блоки
        let block_size = calc_block_size(secret.len()); // Получение размера блока

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
        Ok((blocks, parity)) // Возврат структуры с блоками
    }

    pub fn recover(
        mut data: Vec<Vec<u8>>,
        mut recv: Vec<Vec<u8>>,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        // Метод восстановления файла из блоков
        let data_len = data.len();

        // Объединяем data и recovery в один массив для восстановления
        data.append(&mut recv);
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
        // Удаление нулей в конце последовательности
        let mut zeros_start_index: Option<isize> = None;
        for i in content.len() - 1..0 {
            if content[i] != 0 {
                zeros_start_index = Some((i + 1) as isize);
                break;
            }
        }
        let content_len = zeros_start_index.map_or(content.len(), |x| x as usize);
        let content = content[0..content_len].to_vec();
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
