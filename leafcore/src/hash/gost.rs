use streebog::{Digest, Streebog256};
use crate::hash::errors::HashModuleError;
use super::hasher::Hasher;

pub struct StreebogHasher;

impl Hasher for StreebogHasher {
    fn calc_hash_for_chunk(chunk: &[u8]) -> Result<Vec<u8>, Box<dyn HashModuleError>> {
        let mut hasher = Streebog256::new();

        hasher.update(chunk);
        let result = hasher.finalize();

        let result = result.to_vec();
        Ok(result)
    }
}
