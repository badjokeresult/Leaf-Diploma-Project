use std::cmp::{max, min};

use reed_solomon_erasure::{galois_8, ReedSolomon};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use errors::*;

pub trait SecretSharer {
    fn split_into_chunks(&self, secret: &[u8]) -> Result<(Vec<Vec<u8>>, Vec<Vec<u8>>), DataSplittingError>;
    fn recover_from_chunks(&self, data: Vec<Vec<u8>>, rec: Vec<Vec<u8>>) -> Result<Vec<u8>, DataRecoveringError>;
}

pub struct ReedSolomonSecretSharer {
    min_block_size: usize,
    max_block_size: usize,
    growth_factor: f64,
    alignment: usize,
}

impl ReedSolomonSecretSharer {
    pub fn new() -> Result<ReedSolomonSecretSharer, InitializationError> {
        #[cfg(target_pointer_width = "64")]
        let alignment: usize = 64;

        #[cfg(target_pointer_width = "32") ]
        let alignment: usize = 32;

        let min_block_size = 64;
        let max_block_size = 4 * 1024 * 1024 * 1024;
        let growth_factor = 0.5_f64;

        Ok(ReedSolomonSecretSharer{
            min_block_size,
            max_block_size,
            growth_factor,
            alignment
        })
    }

    fn calc_block_size(&self, file_size: usize) -> usize {
        let bs = self.min_block_size as f64 * ((file_size as f64 / self.min_block_size as f64).powf(self.growth_factor));
        let bs = max(self.min_block_size, min(bs as usize, self.max_block_size));
        let bs = ((bs + self.alignment - 1) / self.alignment) * self.alignment;
        bs
    }

    fn calc_amount_of_blocks(file_size: usize, block_size: usize) -> usize {
        (file_size + block_size - 1) / block_size
    }
}

impl SecretSharer for ReedSolomonSecretSharer {
    fn split_into_chunks(&self, secret: &[u8]) -> Result<(Vec<Vec<u8>>, Vec<Vec<u8>>), DataSplittingError> {
        let block_size = self.calc_block_size(secret.len());
        let amount_of_blocks = Self::calc_amount_of_blocks(secret.len(), block_size);
        let mut buf = vec![0u8; block_size * amount_of_blocks];
        for i in 0..secret.len() {
            buf[i] = secret[i];
        }

        let amount_of_recovers = amount_of_blocks;
        let encoder: ReedSolomon<galois_8::Field> = match ReedSolomon::new(amount_of_blocks, amount_of_recovers) {
            Ok(e) => e,
            Err(e) => {
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

        blocks.append(&mut vec![vec![0u8; block_size]; amount_of_recovers]);
        if blocks.len() < amount_of_blocks * 2 {
            eprintln!("ERROR BLOCKS_LEN < amount_of_blocks * 2");
            panic!();
        }

        encoder.encode(&mut blocks).unwrap();
        let (data, rec) = blocks.split_at(amount_of_blocks);
        Ok((data.to_vec(), rec.to_vec()))
    }

    fn recover_from_chunks(&self, data: Vec<Vec<u8>>, mut rec: Vec<Vec<u8>>) -> Result<Vec<u8>, DataRecoveringError> {
        let mut chunks = data;
        chunks.append(&mut rec);

        let mut full_data = chunks.par_iter().cloned().map(Some).collect::<Vec<_>>();
        let (data_len, recovery_len) = (full_data.len() / 2, full_data.len() / 2);

        let decoder: ReedSolomon<galois_8::Field> = ReedSolomon::new(data_len, recovery_len).unwrap();
        decoder.reconstruct_data(&mut full_data).unwrap();

        let content = full_data[..data_len].par_iter().cloned().filter_map(|x| x).collect::<Vec<_>>();
        let mut secret = vec![];
        for i in 0..data_len {
            let mut value = content[i].clone();
            secret.append(&mut value);
        }

        let secret = match secret.iter().position(|x| 0u8.eq(x)) {
            Some(p) => secret.split_at(p).0.to_vec(),
            None => secret,
        };

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

    #[derive(Debug, Clone)]
    pub struct InitializationError(pub String);

    impl fmt::Display for InitializationError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error initializing data: {}", self.0)
        }
    }
}

#[cfg(test)]
mod tests {

}
