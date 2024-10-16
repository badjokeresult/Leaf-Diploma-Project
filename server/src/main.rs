use crate::peer::{BroadcastServerPeer, ServerPeer};

mod peer;

#[tokio::main]
async fn main() {
    let server = BroadcastServerPeer::new().await;
    server.listen().await;
}
