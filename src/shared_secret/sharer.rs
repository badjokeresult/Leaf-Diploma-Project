use std::cmp::{max, min};

use reed_solomon_erasure::{galois_8, ReedSolomon};
use sharks::{Share, Sharks};

pub trait SecretSharer {
    fn split_into_chunks(&self, secret: &[u8]) -> Vec<Vec<u8>>;
    fn recover_from_chunks(&self, chunks: Vec<Vec<u8>>) -> Vec<u8>;
}

pub struct ShamirSecretSharer {
    min_parts_amount: u8,
    total_parts_amount: usize,
}

impl ShamirSecretSharer {
    pub fn new() -> ShamirSecretSharer {
        ShamirSecretSharer {
            min_parts_amount: 0,
            total_parts_amount: 0,
        }
    }

    fn calc_parts_amounts_by_file_size(&mut self, file_size: usize) -> (u8, usize) {
        todo!()
    }
}

impl SecretSharer for ShamirSecretSharer {
    fn split_into_chunks(&mut self, secret: &[u8]) -> Vec<Vec<u8>> {
        self.calc_parts_amounts_by_file_size(secret.len());
        let sharer = Sharks(self.min_parts_amount);
        let dealer = sharer.dealer(secret);
        let chunks: Vec<Share> = dealer.take(self.total_parts_amount).collect();

        let mut chunks_as_bytes = vec![];
        for chunk in chunks {
            chunks_as_bytes.push(Vec::from(&chunk));
        }
        chunks_as_bytes
    }

    fn recover_from_chunks(&self, chunks: Vec<Vec<u8>>) -> Vec<u8> {
        let mut chunks_as_shares: Vec<Share> = Vec::new();
        for chunk in chunks {
            chunks_as_shares.push(Share::try_from(chunk.as_slice()).unwrap());
        }

        let sharer = Sharks(self.min_parts_amount);
        let secret = sharer.recover(chunks_as_shares.iter().as_slice()).unwrap();
        secret
    }
}

pub struct ReedSolomonSecretSharer {
    min_block_size: usize,
    max_block_size: usize,
    growth_factor: f64,
    alignment: usize,
}

impl ReedSolomonSecretSharer {
    pub fn new() -> ReedSolomonSecretSharer {
        ReedSolomonSecretSharer {
            min_block_size: 64,
            max_block_size: 512,
            growth_factor: 0.5,
            alignment: 64,
        }
    }

    fn calc_block_size(file_size: usize, min_block_size: usize, max_block_size: usize, growth_factor: f64, alignment: usize) -> usize {
        let bs = min_block_size as f64 * ((file_size as f64 / min_block_size as f64).powf(growth_factor));
        let bs = max(min_block_size, min(bs as usize, max_block_size));
        let bs = ((bs + alignment - 1) / alignment) * alignment;
        bs
    }

    fn calc_amount_of_blocks(file_size: usize, block_size: usize) -> usize {
        (file_size + block_size - 1) / block_size
    }
}

impl SecretSharer for ReedSolomonSecretSharer {
    fn split_into_chunks(&self, secret: &[u8]) -> Vec<Vec<u8>> {
        let block_size = Self::calc_block_size(secret.len(), self.min_block_size, self.max_block_size, self.growth_factor, self.alignment);
        let amount_of_blocks = Self::calc_amount_of_blocks(secret.len(), block_size);

        let mut buf = vec![0u8; block_size * amount_of_blocks];
        for i in 0..secret.len() {
            buf[i] = secret[i];
        }

        if secret.len() % block_size != 0 {
            panic!();
        }

        let encoder: ReedSolomon<galois_8::Field> = ReedSolomon::new(amount_of_blocks, amount_of_blocks).unwrap();
        let mut blocks = vec![];
        let blocks_chunks = buf.chunks(block_size).map(|x| Vec::from(x)).collect::<Vec<_>>();
        for chunk in blocks_chunks {
            blocks.push(chunk);
        }

        let mut shares = vec![];
        for _ in 0..amount_of_blocks {
            shares.push(vec![0u8; block_size]);
        }

        blocks.append(&mut shares.clone());
        if blocks.len() < amount_of_blocks * 2 {
            panic!();
        }

        encoder.encode(&mut blocks).unwrap();

        blocks[amount_of_blocks..].to_vec()
    }

    fn recover_from_chunks(&self, chunks: Vec<Vec<u8>>) -> Vec<u8> {
        let mut blocks = chunks.iter().cloned().map(Some).collect::<Vec<_>>();
        let blocks_len = blocks.len();
        let mut full_data = vec![None; blocks_len];
        full_data.append(&mut blocks);

        let decoder: ReedSolomon<galois_8::Field> = ReedSolomon::new(blocks_len, blocks_len).unwrap();
        decoder.reconstruct_data(&mut full_data).unwrap();

        let content = full_data[..blocks_len].iter().cloned().filter_map(|x| x).collect::<Vec<_>>();
        let mut secret = vec![];
        for i in 0..blocks_len {
            let mut value = content[i].clone();
            secret.append(&mut value);
        }

        let secret = secret.iter().cloned().filter(|x| x.clone() > 0u8).collect::<Vec<_>>();
        secret
    }
}
