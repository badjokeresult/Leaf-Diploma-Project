use std::cmp::{max, min};

use reed_solomon_erasure::{galois_8, ReedSolomon};
use rayon::prelude::*;

use errors::*;
use consts::*;

pub trait SecretSharer {
    fn split_into_chunks(&self, secret: &[u8]) -> Result<Vec<Vec<u8>>, DataSplittingError>;
    fn recover_from_chunks(&self, chunks: Vec<Vec<u8>>) -> Result<Vec<u8>, DataRecoveringError>;
}

mod consts {
    pub const MIN_BLOCK_SIZE: usize = 64;
    pub const MAX_BLOCK_SIZE: usize = 2 * 1024 * 1024 * 1024;
    pub const GROWTH_FACTOR: f64 = 0.5;
    pub const ALIGNMENT: usize = 64;
}

pub struct ReedSolomonSecretSharer;

impl ReedSolomonSecretSharer {
    pub fn new() -> ReedSolomonSecretSharer {
        ReedSolomonSecretSharer {

        }
    }

    fn calc_block_size(file_size: usize) -> usize {
        let bs = MIN_BLOCK_SIZE as f64 * ((file_size as f64 / MIN_BLOCK_SIZE as f64).powf(GROWTH_FACTOR));
        let bs = max(MIN_BLOCK_SIZE, min(bs as usize, MAX_BLOCK_SIZE));
        let bs = ((bs + ALIGNMENT - 1) / ALIGNMENT) * ALIGNMENT;
        bs
    }

    fn calc_amount_of_blocks(file_size: usize, block_size: usize) -> usize {
        (file_size + block_size - 1) / block_size
    }
}

impl SecretSharer for ReedSolomonSecretSharer {
    fn split_into_chunks(&self, secret: &[u8]) -> Result<Vec<Vec<u8>>, DataSplittingError> {
        let block_size = Self::calc_block_size(secret.len());
        let amount_of_blocks = Self::calc_amount_of_blocks(secret.len(), block_size);
        let mut buf = vec![0u8; block_size * amount_of_blocks];
        for i in 0..secret.len() {
            buf[i] = secret[i];
        }

        let encoder: ReedSolomon<galois_8::Field> = match ReedSolomon::new(amount_of_blocks, amount_of_blocks) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("ERROR INIT REED_SOLOMON");
                return Err(DataSplittingError(e.to_string()));
            },
        };
        let mut blocks = vec![];
        let blocks_chunks = buf
            .par_iter()
            .chunks(block_size)
            .map(|x| {
                let mut v = vec![];
                for i in x {
                    v.push(i.clone());
                }
                v
            })
            .collect::<Vec<_>>();
        for chunk in blocks_chunks {
            blocks.push(chunk);
        }

        blocks.append(&mut vec![vec![0u8; block_size]; amount_of_blocks]);
        if blocks.len() < amount_of_blocks * 2 {
            eprintln!("ERROR BLOCKS_LEN < amount_of_blocks * 2");
            panic!();
        }

        encoder.encode(&mut blocks).unwrap();

        Ok(blocks)
    }

    fn recover_from_chunks(&self, chunks: Vec<Vec<u8>>) -> Result<Vec<u8>, DataRecoveringError> {
        let mut blocks = chunks.par_iter().cloned().map(Some).collect::<Vec<_>>();
        let blocks_len = blocks.len();
        let mut full_data = vec![None; blocks_len];
        full_data.append(&mut blocks);

        let decoder: ReedSolomon<galois_8::Field> = ReedSolomon::new(blocks_len, blocks_len).unwrap();
        decoder.reconstruct_data(&mut full_data).unwrap();

        let content = full_data[..blocks_len].par_iter().cloned().filter_map(|x| x).collect::<Vec<_>>();
        let mut secret = vec![];
        for i in 0..blocks_len {
            let mut value = content[i].clone();
            secret.append(&mut value);
        }

        let secret = secret.par_iter().cloned().filter(|x| x.clone() > 0u8).collect::<Vec<_>>();
        Ok(secret)
    }
}

mod errors {
    use std::fmt;
    use std::fmt::Formatter;

    #[derive(Debug, Clone)]
    pub struct DataSplittingError(pub String);

    impl fmt::Display for DataSplittingError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error attempting to split a data into chunks: {}", self.0)
        }
    }

    #[derive(Debug, Clone)]
    pub struct DataRecoveringError(pub String);

    impl fmt::Display for DataRecoveringError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error recovering data from chunks: {}", self.0)
        }
    }
}

#[cfg(test)]
mod tests {

}