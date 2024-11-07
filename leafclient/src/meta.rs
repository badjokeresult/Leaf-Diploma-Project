use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct MetaFileInfo {
    data_parts_hashes: Vec<Option<Vec<u8>>>,
    recovery_parts_hashes: Vec<Option<Vec<u8>>>,
}

impl MetaFileInfo {
    pub fn new(data: Vec<Option<Vec<u8>>>, recovery: Vec<Option<Vec<u8>>>) -> MetaFileInfo {
        MetaFileInfo {
            data_parts_hashes: data,
            recovery_parts_hashes: recovery,
        }
    }

    pub fn deconstruct(self) -> (Vec<Option<Vec<u8>>>, Vec<Option<Vec<u8>>>) {
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