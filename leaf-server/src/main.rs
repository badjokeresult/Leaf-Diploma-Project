mod peer;
mod storage;

use std::future::Future;
use peer::{BroadcastServerPeer, ServerPeer};

#[tokio::main]
async fn main() {
    let server = match BroadcastServerPeer::new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error init server: {}", e.to_string());
            return;
        }
    };
    server.listen();
}
