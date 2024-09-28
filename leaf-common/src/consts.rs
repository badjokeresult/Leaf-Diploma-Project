use std::time::Duration;

pub const MAX_DATAGRAM_SIZE: usize = 508;
pub const WORKING_FOLDER_NAME: &str = ".leaf";
pub const PASSWORD_FILE_NAME: &str = "passwd.txt";
pub const GAMMA_FILE_NAME: &str = "gamma.bin";
pub const DEFAULT_SERVER_PORT: u16 = 62092;
pub static MAX_TIMEOUT: Duration = Duration::new(3, 0);
