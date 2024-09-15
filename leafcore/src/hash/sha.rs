use openssl::hash::{hash, MessageDigest};
use crate::hash::errors::{HashCalculationError, HashModuleError};
use super::hasher::Hasher;

pub struct Sha3_256Hasher;

impl Hasher for Sha3_256Hasher {
    fn calc_hash_for_chunk(chunk: &[u8]) -> Result<Vec<u8>, Box<dyn HashModuleError>> {
        let hash = match hash(MessageDigest::sha3_256(), chunk) {
            Ok(h) => h,
            Err(e) => return Err(Box::new(HashCalculationError(e.to_string()))),
        }.to_vec();

        Ok(hash)
    }
}
