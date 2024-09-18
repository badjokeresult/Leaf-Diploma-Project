use std::collections::HashMap;

pub struct ClientCurrentState {
    pub state_db: HashMap<String, Vec<Vec<u8>>>,
}

impl ClientCurrentState {
    pub fn new() -> ClientCurrentState {
        ClientCurrentState{
            state_db: HashMap::new(),
        }
    }
}