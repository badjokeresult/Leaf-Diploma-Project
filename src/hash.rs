use std::cell::RefCell;
use std::io::Write;

use streebog::digest::consts::U32;
use streebog::digest::core_api::{CoreWrapper, CtVariableCoreWrapper};
use streebog::{Digest, Oid256, StreebogVarCore};

pub trait Hasher {
    fn calc_hash_for_chunk(&self, chunk: &[u8]) -> Vec<u8>;
}

pub struct StreebogHasher {
    hasher: RefCell<CoreWrapper<CtVariableCoreWrapper<StreebogVarCore, U32, Oid256>>>,
}

impl StreebogHasher {
    pub fn new() -> StreebogHasher {
        StreebogHasher {
            hasher: RefCell::new(streebog::Streebog256::new()),
        }
    }
}

impl Hasher for StreebogHasher {
    fn calc_hash_for_chunk(&self, chunk: &[u8]) -> Vec<u8> {
        self.hasher.borrow_mut().update(chunk);
        let result = self.hasher.borrow_mut().clone().finalize();

        let result = result.to_vec();
        self.hasher.borrow_mut().flush().unwrap();

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streebog_calc_hash_for_chunk_successful_calculation() {
        let hasher = StreebogHasher::new();
        let message = b"Hello World";
        let result = vec![102, 108, 244, 251, 247, 78, 198, 138, 102, 232, 221, 61, 48, 97, 176, 51, 117, 104, 206, 33, 161, 4, 84, 29, 77, 238, 3, 245, 68, 140, 41, 175];

        let hash = hasher.calc_hash_for_chunk(message);

        assert_eq!(hash, result);
    }
}