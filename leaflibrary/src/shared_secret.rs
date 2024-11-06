use std::cmp::{max, min};
use std::io::{Error, ErrorKind};

use reed_solomon_erasure::{galois_8, ReedSolomon};
use rayon::prelude::*;

use errors::*;
use consts::*;

pub trait SecretSharer {
    fn split_into_chunks(&self, secret: &[u8]) -> Result<Vec<Vec<Option<Vec<u8>>>>, DataSplittingError>;
    fn recover_from_chunks(&self, chunks: Vec<Option<Vec<u8>>>) -> Result<Vec<u8>, DataRecoveringError>;
}

mod consts {
    pub const MIN_BLOCK_SIZE: usize = 64;
    pub const MAX_BLOCK_SIZE: usize = 2 * 1024 * 1024 * 1024;
    pub const GROWTH_FACTOR: f64 = 0.5;
    pub const ALIGNMENT: usize = 64;
}

#[repr(usize)]
#[derive(Clone, Copy)]
pub enum RecoveringLevel {
    Unknown = 2,
    Minimal = 0,
    Maximal = 1,
}

impl From<usize> for RecoveringLevel {
    fn from(value: usize) -> Self {
        match value {
            0 => RecoveringLevel::Minimal,
            1 => RecoveringLevel::Maximal,
            _ => RecoveringLevel::Unknown,
        }
    }
}

impl Into<usize> for RecoveringLevel {
    fn into(self) -> usize {
        match self {
            RecoveringLevel::Maximal => 1,
            RecoveringLevel::Minimal => 0,
            RecoveringLevel::Unknown => 2,
        }
    }
}

pub struct ReedSolomonSecretSharer {
    recovering_level: RecoveringLevel,
}

impl ReedSolomonSecretSharer {
    pub fn new(recovering_level: usize) -> Result<ReedSolomonSecretSharer, Error> {
        let recovering_level = RecoveringLevel::from(recovering_level);
        if let RecoveringLevel::Unknown = recovering_level {
            return Err(Error::new(ErrorKind::InvalidData, "Invalid"));
        }
        Ok(ReedSolomonSecretSharer {
            recovering_level,
        })
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
    fn split_into_chunks(&self, secret: &[u8]) -> Result<Vec<Vec<Option<Vec<u8>>>>, DataSplittingError> {
        let block_size = Self::calc_block_size(secret.len());
        let amount_of_blocks = Self::calc_amount_of_blocks(secret.len(), block_size);
        let mut buf = vec![0u8; block_size * amount_of_blocks];
        for i in 0..secret.len() {
            buf[i] = secret[i];
        }

        let amount_of_recovers = match self.recovering_level {
            RecoveringLevel::Minimal => amount_of_blocks,
            RecoveringLevel::Maximal => amount_of_blocks * 2,
            RecoveringLevel::Unknown => return Err(DataSplittingError("Unknown".to_string())),
        };
        let encoder: ReedSolomon<galois_8::Field> = match ReedSolomon::new(amount_of_blocks, amount_of_recovers) {
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

        blocks.append(&mut vec![vec![0u8; block_size]; amount_of_recovers]);
        if blocks.len() < amount_of_blocks * 2 {
            eprintln!("ERROR BLOCKS_LEN < amount_of_blocks * 2");
            panic!();
        }

        encoder.encode(&mut blocks).unwrap();
        let chunks = blocks.par_iter().cloned().map(Some).collect::<Vec<_>>();

        let chunks = match self.recovering_level {
            RecoveringLevel::Minimal => {
                let (data, rec) = chunks.split_at(chunks.len() / 2);
                vec![data.to_vec(), rec.to_vec()]
            },
            RecoveringLevel::Maximal => {
                let (data, rec) = chunks.split_at(chunks.len() / 3);
                vec![data.to_vec(), rec.to_vec()]
            },
            RecoveringLevel::Unknown => return Err(DataSplittingError("Error".to_string())),
        };

        Ok(chunks)
    }

    fn recover_from_chunks(&self, chunks: Vec<Option<Vec<u8>>>) -> Result<Vec<u8>, DataRecoveringError> {
        let mut full_data = chunks.par_iter().cloned().map(|x| {
            if let Some(d) = x {
                Some(d)
            } else {
                None
            }
        }).collect::<Vec<_>>();
        let (data_len, recovery_len) = match self.recovering_level {
            RecoveringLevel::Minimal => (full_data.len() / 2, full_data.len() / 2),
            RecoveringLevel::Maximal => (full_data.len() / 3, (full_data.len() / 3) * 2),
            RecoveringLevel::Unknown => return Err(DataRecoveringError("Error!".to_string())),
        };

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
}

#[cfg(test)]
mod tests {

}
