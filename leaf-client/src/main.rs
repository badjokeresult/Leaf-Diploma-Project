use std::fs;
use leafclient;

fn main() {
    let content = fs::read("test.txt").unwrap();
    let hashes = leafclient::send_file(content).unwrap();
    println!("File `test.txt` was sent! Len of `hashes`: {}", hashes.len());
    let recv_content = leafclient::recv_file(hashes).unwrap();
    fs::write("new_test.txt", &recv_content).unwrap();
}