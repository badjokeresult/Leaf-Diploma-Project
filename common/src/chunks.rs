use std::cmp::{max, min};

use reed_solomon_erasure::{galois_8, ReedSolomon};
use rayon::prelude::*;

use errors::*;

pub struct FileParts {
    data_parts: Vec<Option<Vec<u8>>>,
    recovery_parts: Vec<Option<Vec<u8>>>,
}

impl FileParts {
    pub fn new(data_parts: Vec<Option<Vec<u8>>>, recovery_parts: Vec<Option<Vec<u8>>>) -> FileParts {
        FileParts {
            data_parts,
            recovery_parts,
        }
    }

    pub fn deconstruct(self) -> (Vec<Option<Vec<u8>>>, Vec<Option<Vec<u8>>>) {
        (self.data_parts, self.recovery_parts)
    }
}

pub trait SecretSharer {
    fn split_into_chunks(&self, secret: &[u8]) -> Result<FileParts, DataSplittingError>;
    fn recover_from_chunks(&self, chunks: FileParts) -> Result<Vec<u8>, DataRecoveringError>;
}

pub struct ReedSolomonSecretSharer {
    min_block_size: usize,
    max_block_size: usize,
    growth_factor: f64,
    alignment: usize,
}

impl ReedSolomonSecretSharer {
    pub fn new(min_block_size: Option<usize>, max_block_size: Option<usize>, growth_factor: Option<f64>) -> Result<ReedSolomonSecretSharer, InitializationError> {
        #[cfg(target_pointer_width = "64")]
        let alignment: usize = 64;

        #[cfg(target_pointer_width = "32") ]
        let alignment: usize = 32;

        let min_block_size = match min_block_size {
            Some(v) => {
                if v % 64 == 0 {
                    v
                } else {
                    return Err(InitializationError(format!("Invalid min block size value: {}", v)));
                }
            },
            None => 64
        };
        let max_block_size = match max_block_size {
            Some(v) => {
                if v % 64 == 0 {
                    v
                } else {
                    return Err(InitializationError(format!("Invalid max block size value: {}", v)));
                }
            },
            None => 2 * 1024 * 1024 * 1024,
        };
        let growth_factor = growth_factor.unwrap_or(0.5);

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
    fn split_into_chunks(&self, secret: &[u8]) -> Result<FileParts, DataSplittingError> {
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
        let chunks = blocks.par_iter().cloned().map(Some).collect::<Vec<_>>();

        let (data, rec) = chunks.split_at(amount_of_blocks);

        Ok(FileParts::new(data.to_vec(), rec.to_vec()))
    }

    fn recover_from_chunks(&self, chunks: FileParts) -> Result<Vec<u8>, DataRecoveringError> {
        let mut chunks = chunks.deconstruct();
        chunks.0.append(&mut chunks.1);
        let chunks = chunks.0;
        let mut full_data = chunks.par_iter().cloned().map(|x| {
            if let Some(d) = x {
                Some(d)
            } else {
                None
            }
        }).collect::<Vec<_>>();
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
