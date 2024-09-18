pub type Result<T> = std::result::Result<T, Box<dyn super::errors::HashModuleError>>;

pub trait Hasher {
    fn calc_hash_for_chunk(chunk: &[u8]) -> Result<Vec<u8>>;
    fn is_hash_equal_to_model(chunk: &[u8], model: &[u8]) -> bool {
        chunk.iter().eq(model.iter())
    }
}

pub mod sha {
    use sha3::Digest;

    use super::{Hasher, Result};

    pub struct Sha3_256Hasher;

    impl Hasher for Sha3_256Hasher {
        fn calc_hash_for_chunk(chunk: &[u8]) -> Result<Vec<u8>> {
            let mut hasher = sha3::Sha3_256::new();

            hasher.update(chunk);

            let result = hasher.finalize();
            Ok(result.to_vec())
        }
    }
}

pub mod gost {
    use streebog::Digest;

    use super::{Result, Hasher};

    pub struct StreebogHasher;

    impl Hasher for StreebogHasher {
        fn calc_hash_for_chunk(chunk: &[u8]) -> Result<Vec<u8>> {
            let mut hasher = streebog::Streebog256::new();

            hasher.update(chunk);
            let result = hasher.finalize();

            let result = result.to_vec();
            Ok(result)
        }
    }
}