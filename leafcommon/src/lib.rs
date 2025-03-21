pub mod crypto;
pub use crypto::{Encryptor, Hasher, KuznechikEncryptor, StreebogHasher};
pub mod message;
pub use message::Message;
pub mod shards;
pub use shards::{ReedSolomonSecretSharer, SecretSharer};

pub mod chunks;
pub use chunks::{
    Chunk, ChunkHash, Chunks, ChunksHashes, ReedSolomonChunk, ReedSolomonChunkHash,
    ReedSolomonChunks, ReedSolomonChunksHashes,
};

type ByteStream = Vec<Vec<u8>>;
