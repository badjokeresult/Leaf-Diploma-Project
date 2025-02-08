use serde::{Deserialize, Serialize}; // Внешняя зависимость для сериализации и десериализации структуры
use serde_json; // Внешняя зависимость для сериализации и десериализации в JSON

use base64::prelude::*; // Внешняя зависимость для кодирования и декодирования по алгоритму Base64

use consts::*;
use errors::*; // Внутренний модуль для использования составных типов ошибок // Внутренний модуль для использования констант

mod consts {
    // Модуль с константами
    // Максимальный размер блока данных в датаграмме
    // Всего можно передать 65535 байт данных
    // Из них вычитаем 8 байт заголовка UDP
    // Из них вычитаем 20 байт заголовка IP
    // Из них вычитаем 256 байт хэш-суммы
    pub const MAX_MESSAGE_SIZE: usize = 65251;
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub enum Message {
    // Тип сообщения
    SendingReq(Vec<u8>), // Запрос на отправку данных клиентом, содержит только хэш-сумму
    SendingAck(Vec<u8>), // Подтверждение на отправку от сервера, содержит только хэш-сумму
    RetrievingReq(Vec<u8>), // Запрос на получение данных клиентом, содержит только хэш-сумму
    RetrievingAck(Vec<u8>), // Подтверждение получения от сервера, содержит только хэш-сумму
    ContentFilled(Vec<u8>, Vec<u8>), // Сообщение с данными, может быть отправлено клиентом или сервером, содержит хэш-сумму и данные
    Empty(Vec<u8>), // Сообщение-заглушка, сигнализирует об окончании потока сообщений с данными
}

impl Message {
    pub fn generate_stream_for_chunk(
        hash: &[u8],
        chunk: &[u8],
    ) -> Result<Vec<Message>, MessageStreamGenerationError> {
        // Метод генерации потока сообщений с данными
        let mut stream = vec![]; // Создаем буфер для потока сообщений
        let chunks = chunk
            .chunks(MAX_MESSAGE_SIZE)
            .map(|x| x.to_vec())
            .collect::<Vec<_>>(); // Данные разбиваются на куски максимально допустимого размера
        for c in chunks {
            stream.push(Message::ContentFilled(hash.to_vec(), c.to_vec())); // На каждый кусок данных создается свое сообщение
        }
        Ok(stream)
    }
}

impl Into<Vec<u8>> for Message {
    fn into(self) -> Vec<u8> {
        // Метод сериализации сообщения в JSON с последующим кодированием в Base64 для компактной отправки
        let json = serde_json::to_string_pretty(&self).unwrap(); // Сериализация
        BASE64_STANDARD.encode(json.as_bytes()).into_bytes() // Кодирование
    }
}

impl From<Vec<u8>> for Message {
    fn from(value: Vec<u8>) -> Self {
        // Метод декодирования из Base64 и десериализации из JSON
        let json = BASE64_STANDARD.decode(&value).unwrap(); // Декодирование
        let message = serde_json::from_slice(&json).unwrap(); // Десериализация
        message
    }
}

mod errors {
    // Модуль с составными типами ошибок
    use std::fmt; // Зависимость стандартной библиотеки

    #[derive(Debug, Clone)]
    pub struct MessageStreamGenerationError(pub String); // Ошибка генерации потока сообщений

    impl fmt::Display for MessageStreamGenerationError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            // Метод отображения сведений об ошибке на экране
            write!(f, "Error generation message stream: {}", self.0)
        }
    }
}
