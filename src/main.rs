mod lib;

use std::fs;
use lib::*;

fn main() {
    let client_ptr = init();
    let content = fs::read("test.txt").unwrap();
    let hashes = send_file(content, client_ptr);
    let new_content = recv_content(hashes, client_ptr);
    fs::write("test1.txt", &new_content).unwrap();
}

