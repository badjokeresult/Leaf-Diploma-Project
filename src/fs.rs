use std::path::PathBuf;

use tokio::fs;

use errors::*;

type Result<T> = std::result::Result<T, Box<dyn FilesystemError>>;

pub trait FilesystemWorker {
    async fn get_content(&self, path: &PathBuf) -> Result<Vec<u8>>;
    async fn write_content(&self, path: &PathBuf, content: &[u8]) -> Result<()>;
}

pub struct LocalFilesystemWorker;

impl LocalFilesystemWorker {
    pub fn new() -> LocalFilesystemWorker {
        LocalFilesystemWorker {}
    }
}

impl FilesystemWorker for LocalFilesystemWorker {
    async fn get_content(&self, path: &PathBuf) -> Result<Vec<u8>> {
        match fs::read(path).await {
            Ok(d) => Ok(d),
            Err(e) => return Err(Box::new(ReadingFileContentError(e.to_string()))),
        }
    }

    async fn write_content(&self, path: &PathBuf, content: &[u8]) -> Result<()> {
        match fs::write(path, content).await {
            Ok(_) => Ok(()),
            Err(e) => return Err(Box::new(WritingToFileError(e.to_string()))),
        }
    }
}

mod errors {
    use std::fmt;
    use std::fmt::Formatter;

    pub trait FilesystemError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result;
    }

    #[derive(Debug, Clone)]
    pub struct ReadingFileContentError(pub String);

    impl FilesystemError for ReadingFileContentError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error reading file content: {}", self.0)
        }
    }

    impl fmt::Display for ReadingFileContentError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            FilesystemError::fmt(self, f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct WritingToFileError(pub String);

    impl FilesystemError for WritingToFileError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(f, "Error writing data to a file: {}", self.0)
        }
    }

    impl fmt::Display for WritingToFileError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            FilesystemError::fmt(self, f)
        }
    }
}

mod tests {

}