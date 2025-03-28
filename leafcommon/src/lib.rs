pub mod message;
pub use message::Message;

mod crypto;
mod shards;

mod chunks;

pub mod reed_solomon_scheme {
    use super::chunks::{Chunks, ChunksHashes, ReedSolomonChunks, ReedSolomonChunksHashes};
    use super::crypto::{Encryptor, KuznechikEncryptor};

    use std::error::Error;
    use std::path::Path;

    pub async fn send_file(path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
        let encryptor: Box<dyn Encryptor> = Box::new(KuznechikEncryptor::new().await?);

        let mut chunks = ReedSolomonChunks::from_file(&path).await?; // Получаем чанки
        chunks.encrypt(&encryptor)?; // Шифруем их
        chunks.update_hashes()?; // Обновляем их хэш-суммы
        let hashes = chunks.send().await?; // Отправляем чанки в домен и получаем назад их хэш-суммы
        hashes.save_to(path).await?; // Сохраняем хэш-суммы в целевом файле
        Ok(())
    }

    pub async fn recv_file(path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
        let decryptor: Box<dyn Encryptor> = Box::new(KuznechikEncryptor::new().await?);

        let hashes = ReedSolomonChunksHashes::load_from(&path).await?; // Получаем хэш-суммы из файла
        let mut chunks = ReedSolomonChunks::recv(hashes).await?; // Получаем чанки по хэшам
        chunks.decrypt(&decryptor)?; // Расшифровываем чанки
        chunks.into_file(path).await?; // Восстанавливаем из них содержимое и записываем его в целевой файл
        Ok(())
    }
}
