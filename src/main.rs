use std::fs;

mod lib;

use lib::*;

#[tokio::main]
async fn main() {
    let local_addr = "0.0.0.0:62092";
    let broadcast_addr = "192.168.122.255:62092";
    let num_threads: usize = 8;
    let (mut rx, server) = init(local_addr, broadcast_addr, num_threads);

    let content = fs::read("text.txt").unwrap();
    let hashes = send_file(content, &server, &mut rx).unwrap();
    let new_content = recv_content(hashes, &server, &rx).unwrap();
    fs::write("text1.txt", new_content).unwrap();

    shutdown(server).unwrap();
}