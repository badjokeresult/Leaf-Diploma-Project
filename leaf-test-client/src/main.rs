use std::fs;
use leaf_client::*;

#[tokio::main]
async fn main() {
    let content = fs::read("test.txt").unwrap();
    println!("LEN CONTENT : {}", content.len());
    let hashes = send_file(content).await;
    println!("HASHES COUNT : {}", hashes.len());
}
