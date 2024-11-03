use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct MetaFileInfo {
    recovering_level: usize,
    data_parts_hashes: Vec<Option<Vec<u8>>>,
    recovery_parts_hashes: Vec<Vec<Option<Vec<u8>>>>,
}

impl MetaFileInfo {
    pub fn new(recovering_level: usize, data: Vec<Option<Vec<u8>>>, recovery: Vec<Vec<Option<Vec<u8>>>>) -> MetaFileInfo {
        MetaFileInfo {
            recovering_level,
            data_parts_hashes: data,
            recovery_parts_hashes: recovery,
        }
    }

    pub fn deconstruct(self) -> (Vec<Option<Vec<u8>>>, Vec<Vec<Option<Vec<u8>>>>) {
        (self.data_parts_hashes, self.recovery_parts_hashes)
    }
}

impl From<Vec<u8>> for MetaFileInfo {
    fn from(value: Vec<u8>) -> Self {
        let obj: MetaFileInfo = serde_json::from_slice(&value).unwrap();
        obj
    }
}

impl Into<Vec<u8>> for MetaFileInfo {
    fn into(self) -> Vec<u8> {
        serde_json::to_vec(&self).unwrap()
    }
}