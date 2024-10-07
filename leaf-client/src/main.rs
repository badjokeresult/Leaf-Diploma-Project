mod lib;

use std::fs;
use lib::send_file;

#[tokio::main]
async fn main() {
    let content = fs::read("test.txt").unwrap();
    println!("LEN CONTENT : {}", content.len());
    let hashes = send_file(content).await;
    println!("HASHES COUNT : {}", hashes.len());
}
