use crate::hash::errors::HashModuleError;

pub trait Hasher {
    fn calc_hash_for_chunk(chunk: &[u8]) -> Result<Vec<u8>, Box<dyn HashModuleError>>;
    fn is_hash_equal_to_model(chunk: &[u8], model: &[u8]) -> bool {
        chunk.iter().eq(model.iter())
    }
}
