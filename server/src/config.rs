pub const LOCAL_ADDR: &str = "0.0.0.0";
pub const LOCAL_PORT: u16 = 62092;
pub const CHAN_SIZE: usize = 100;
pub const BUF_SIZE: usize = 4096;
pub const MILLIS_TIMEOUT: u64 = 100;

#[cfg(not(target_os = "windows"))]
pub const STOR_PATH: &str = "/var/local/leaf/stor";

#[cfg(target_os = "windows")]
pub const STOR_PATH: &str = "C:\\Program Files\\Leaf\\Storage";
