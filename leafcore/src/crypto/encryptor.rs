pub trait Encryptor {
    async fn encrypt_chunk(&self, chunk: &[u8]) -> Vec<u8>;
    async fn decrypt_chunk(&self, chunk: &[u8]) -> Vec<u8>;
}
