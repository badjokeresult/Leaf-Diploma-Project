use sharks::{Sharks, Share};

pub trait SecretSharer {
    fn split_into_chunks(&self, secret: Vec<u8>) -> Vec<Vec<u8>>;
    fn recover_from_chunks(&self, chunks: Vec<Vec<u8>>) -> Vec<u8>;
}

pub struct ShamirSecretSharer {
    sharks: Sharks,
}

impl ShamirSecretSharer {
    pub fn new(parts_to_split: u8) -> ShamirSecretSharer {
        let sharks = Sharks(parts_to_split);

        ShamirSecretSharer {
            sharks,
        }
    }
}

impl SecretSharer for ShamirSecretSharer {
    fn split_into_chunks(&self, secret: Vec<u8>) -> Vec<Vec<u8>> {
        let dealer = self.sharks.dealer(&secret);
        let chunks: Vec<Share> = dealer.take(5).collect();

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

        let secret = self.sharks.recover(chunks_as_shares.iter().as_slice()).unwrap();
        secret
    }
}

pub(crate) mod errors {
    use std::fmt;
    use std::fmt::Formatter;

    #[derive(Debug, Clone)]
    pub struct DataSplittingError;

    impl fmt::Display for DataSplittingError {
        fn fmt(&self, f: &mut Formatter) -> fmt::Result {
            write!(f, "Error attempting to split a data into chunks")
        }
    }

    #[derive(Debug, Clone)]
    pub struct DataRecoveringError;

    impl fmt::Display for DataRecoveringError {
        fn fmt(&self, f: &mut Formatter) -> fmt::Result {
            write!(f, "Error recovering data from chunks")
        }
    }
}