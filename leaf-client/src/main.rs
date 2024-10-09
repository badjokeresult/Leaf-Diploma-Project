use std::fs;
use leafclient;

use tokio;

#[tokio::main]
async fn main() {
    let content = tokio::fs::read("test.txt").await.unwrap();
    let hashes = leafclient::send_file(content).await.unwrap();
    println!("File `test.txt` was sent! Len of `hashes`: {}", hashes.len());
    let recv_content = leafclient::recv_file(hashes).await.unwrap();
    fs::write("new_test.txt", &recv_content).unwrap();
}