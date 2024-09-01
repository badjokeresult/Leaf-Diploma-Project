mod fs;

use std::path::Path;
use leafcore::{store_file, receive_file, init};

#[cfg(windows)]
const IFILE_PATH: &str = "C:\\Users\\glomo\\test.txt";

#[cfg(not(windows))]
const IFILE_PATH: &str = "/home/glomo/test.txt";

#[cfg(windows)]
const OFILE_PATH: &str = "C:\\Users\\glomo\\test1.txt";

#[cfg(not(windows))]
const OFILE_PATH: &str = "/home/glomo/test1.txt";


#[tokio::main]
async fn main() {
    init().await;
    let binding = std::fs::read_to_string(Path::new(IFILE_PATH)).unwrap();
    let content = binding.as_bytes();
    let content = content.to_vec();
    let hashes = store_file(content).await;
    let content = receive_file(hashes).await;
    let content_as_string = std::str::from_utf8(&content).unwrap();
    std::fs::write(Path::new(OFILE_PATH), content_as_string).unwrap();
}
