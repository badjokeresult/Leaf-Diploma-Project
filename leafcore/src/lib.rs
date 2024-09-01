use lazy_static::lazy_static;

mod codec;
mod crypto;
mod hash;
mod messages;
mod peer;
mod shared_secret;
mod storage;

use async_once::AsyncOnce;
use crate::peer::server::BroadcastServerPeer;

lazy_static! {
    static ref PEER_SERVER: AsyncOnce<BroadcastServerPeer> = AsyncOnce::new(async {
        BroadcastServerPeer::new(62092).await
    });
}

