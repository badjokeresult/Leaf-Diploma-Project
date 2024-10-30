use std::fs;

mod lib;

fn main() {
    let local_addr = "0.0.0.0:62092";
    let broadcast_addr = "192.168.122.255:62092";
    let num_threads: usize = 8;
    let (rx, server) = lib::init(local_addr, broadcast_addr, num_threads);

    let content = fs::read("text.txt").unwrap();
    let hashes = lib::send_file(content, &server, &rx).unwrap();
    let new_content = lib::recv_content(hashes, &server, &rx).unwrap();
    fs::write("text1.txt", new_content).unwrap();
}