use base64::{prelude::BASE64_STANDARD as BASE64, Engine as _};
// use bincode::{deserialize, serialize};
use serde::{Deserialize, Serialize}; // Внешняя зависимость для сериализации и десериализации в JSON
                                     //use zstd::{decode_all, encode_all};

use errors::*;

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum Message {
    // Тип сообщения
    SendingReq(String), // Запрос на отправку данных клиентом, содержит только хэш-сумму
    SendingAck(String), // Подтверждение на отправку от сервера, содержит только хэш-сумму
    RetrievingReq(String), // Запрос на получение данных клиентом, содержит только хэш-сумму
    ContentFilled(String, Vec<u8>), // Сообщение с данными, может быть отправлено клиентом или сервером, содержит хэш-сумму и данные
}

impl Message {
    pub fn into_bytes(self) -> Result<Vec<u8>, IntoBytesCastError> {
        Ok(BASE64
            .encode(serde_json::to_vec(&self).map_err(|e| IntoBytesCastError(e.to_string()))?)
            .as_bytes()
            .to_vec())
    }

    pub fn from_bytes(value: Vec<u8>) -> Result<Message, FromBytesCastError> {
        serde_json::from_slice(
            &BASE64
                .decode(&value)
                .map_err(|e| FromBytesCastError(e.to_string()))?,
        )
        .map_err(|e| FromBytesCastError(e.to_string()))
    }
}

mod errors {
    // Модуль с составными типами ошибок
    use std::error::Error;
    use std::fmt; // Зависимость стандартной библиотеки

    #[derive(Debug, Clone)]
    pub struct IntoBytesCastError(pub String); // Тип ошибки перевода сообщения в вектор

    impl fmt::Display for IntoBytesCastError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Error casting to bytes slice: {}", self.0)
        }
    }

    impl Error for IntoBytesCastError {}

    #[derive(Debug, Clone)]
    pub struct FromBytesCastError(pub String); // Тип ошибка перевода вектора в сообщение

    impl fmt::Display for FromBytesCastError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Error casting from bytes slice: {}", self.0)
        }
    }

    impl Error for FromBytesCastError {}
}
