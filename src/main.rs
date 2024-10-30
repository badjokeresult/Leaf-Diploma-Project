use tokio::fs;

mod lib;

use lib::*;

#[tokio::main]
async fn main() {
    let local_addr = "0.0.0.0:62092";
    let broadcast_addr = "192.168.122.255:62092";
    let num_threads: usize = 8;
    let (mut rx, server) = init(local_addr, broadcast_addr, num_threads).await;

    let content = fs::read("text.txt").await.unwrap();
    let hashes = send_file(content, &server, &mut rx).await.unwrap();
    let new_content = recv_content(hashes, &server, &mut rx).await.unwrap();
    fs::write("text1.txt", new_content).await.unwrap();
}