use openssl::hash::{hash, MessageDigest};

pub fn calc_hash_for_chunk(chunk: &[u8]) -> Vec<u8> {
    hash(MessageDigest::sha3_256(), chunk).unwrap().to_vec()
}

pub fn is_hash_equal_to_model(hash: &[u8], model: &[u8]) -> bool {
    hash.iter().eq(model.iter())
}
