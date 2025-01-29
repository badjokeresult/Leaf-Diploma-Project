use serde::{Deserialize, Serialize};

use common::{Hasher, StreebogHasher};

pub trait Hashes {
    fn new(data: &Vec<Vec<u8>>, recovery: &Vec<Vec<u8>>) -> Self;
}

#[derive(Serialize, Deserialize)]
pub struct FileHashes {
    data: Vec<Vec<u8>>,
    recovery: Vec<Vec<u8>>,
}

impl Hashes for FileHashes {
    fn new(data: &Vec<Vec<u8>>, recovery: &Vec<Vec<u8>>) -> Self {
        let hasher: Box<dyn Hasher> = Box::new(StreebogHasher::new());

        let mut dt = Vec::with_capacity(data.len());
        let mut rc = Vec::with_capacity(recovery.len());

        for d in data {
            dt.push(hasher.calc_hash_for_chunk(d));
        }
        for r in recovery {
            rc.push(hasher.calc_hash_for_chunk(r));
        }

        FileHashes {
            data: dt,
            recovery: rc,
        }
    }
}
