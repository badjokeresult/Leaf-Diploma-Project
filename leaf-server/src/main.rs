mod peer;
mod storage;

use std::future::Future;
use peer::{BroadcastServerPeer, ServerPeer};

#[tokio::main]
async fn main() {
    let server = match BroadcastServerPeer::new().await {
        Ok(s) => s,
        Err(_) => {
            eprintln!("Error init server");
            return;
        }
    };
    server.listen().await;
}
