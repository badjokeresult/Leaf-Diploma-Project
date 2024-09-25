mod errors;

use std::cell::RefCell;
use std::io::Write;

use streebog::digest::consts::U32;
use streebog::digest::core_api::{CoreWrapper, CtVariableCoreWrapper};
use streebog::{Digest, Oid256, StreebogVarCore};

use errors::*;

pub type Result<T> = std::result::Result<T, Box<dyn HashModuleError>>;

pub trait Hasher {
    fn calc_hash_for_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>>;
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
    fn calc_hash_for_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>> {
        self.hasher.borrow_mut().update(chunk);
        let result = self.hasher.borrow_mut().clone().finalize();

        let result = result.to_vec();
        self.hasher.borrow_mut().flush().unwrap();

        Ok(result)
    }
}
