use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io::{Error, ErrorKind};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::message::{Message, consts::*};

mod consts {
    pub const DEFAULT_WORKING_DIR: &str = ".leaf";
    pub const DEFAULT_STOR_FILE_NAME: &str = "stor.bin";
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BroadcastUdpServer {
    storage: RefCell<HashMap<Vec<u8>, Vec<u8>>>,

}

impl BroadcastUdpServer {
    pub fn new() -> BroadcastUdpServer {
        let filepath = dirs::home_dir().unwrap().join(consts::DEFAULT_WORKING_DIR).join(consts::DEFAULT_STOR_FILE_NAME);
        let server = Self::from_file(filepath).unwrap_or_else(|_| BroadcastUdpServer { storage: RefCell::new(HashMap::new()) });
        server
    }

    fn from_file(filepath: PathBuf) -> Result<BroadcastUdpServer, Error> {
        let content = match fs::read(filepath) {
            Ok(c) => c,
            Err(e) => return Err(Error::new(ErrorKind::InvalidData, e.to_string())),
        };
        let server = serde_json::from_slice(&content)?;
        Ok(server)
    }
}

impl BroadcastUdpServer {
    pub fn handle_sending_req(&self, hash: &[u8]) -> Result<Message, Error> {
        match self.alloc_mem_for_chunk(hash) {
            Ok(_) => Ok(Message::new(SENDING_ACKNOWLEDGEMENT_TYPE, hash)),
            Err(e) => Err(e),
        }
    }

    fn alloc_mem_for_chunk(&self, hash: &[u8]) -> Result<(), Error> {
        if self.is_enough_mem_for_chunk() {
            self.storage.borrow_mut().insert(hash.to_vec(), vec![]);
            return Ok(());
        }
        Err(Error::new(ErrorKind::InvalidData, "Not enough memory"))
    }

    fn is_enough_mem_for_chunk(&self) -> bool {
        true
    }

    pub fn handle_retrieving_req(&self, hash: &[u8]) -> Result<Vec<Message>, Error> {
        let chunk = self.retrieve_chunk(hash)?;
        let mut answer = Message::new_with_data(RETRIEVING_ACKNOWLEDGEMENT_TYPE, hash, chunk);
        answer.push(Message::new(EMPTY_TYPE, hash));
        Ok(answer)
    }

    fn retrieve_chunk(&self, hash: &[u8]) -> Result<Vec<u8>, Error> {
        match self.storage.borrow().get(hash) {
            Some(c) => Ok(c.clone()),
            None => Err(Error::new(ErrorKind::InvalidData, "Not found")),
        }
    }

    pub fn handle_content_filled(&self, hash: &[u8], data: &[u8]) -> Result<(), Error> {
        match self.storage.borrow_mut().get(hash) {
            Some(mut c) => {
                let mut binding = c.clone();
                binding.extend_from_slice(data);
            },
            None => return Err(Error::new(ErrorKind::InvalidData, "Not found")),
        };
        Ok(())
    }
}