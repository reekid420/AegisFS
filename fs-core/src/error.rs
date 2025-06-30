use std::fmt;
use std::io;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    InvalidSuperblock,
    InvalidInode,
    InvalidPath,
    NotADirectory,
    AlreadyExists,
    NotFound,
    NotEmpty,
    InvalidArgument,
    Unsupported,
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O error: {}", e),
            Error::InvalidSuperblock => write!(f, "Invalid superblock"),
            Error::InvalidInode => write!(f, "Invalid inode"),
            Error::InvalidPath => write!(f, "Invalid path"),
            Error::NotADirectory => write!(f, "Not a directory"),
            Error::AlreadyExists => write!(f, "File or directory already exists"),
            Error::NotFound => write!(f, "File or directory not found"),
            Error::NotEmpty => write!(f, "Directory not empty"),
            Error::InvalidArgument => write!(f, "Invalid argument"),
            Error::Unsupported => write!(f, "Operation not supported"),
            Error::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<crate::FileSystemError> for Error {
    fn from(err: crate::FileSystemError) -> Self {
        match err {
            crate::FileSystemError::NotSupported => Error::Unsupported,
            crate::FileSystemError::NotFound(msg) => Error::Other(format!("Not found: {}", msg)),
            crate::FileSystemError::Fs(msg) => Error::Other(format!("Filesystem error: {}", msg)),
            crate::FileSystemError::NotImplemented(msg) => Error::Other(format!("Not implemented: {}", msg)),
            crate::FileSystemError::InvalidArgument(msg) => Error::Other(format!("Invalid argument: {}", msg)),
            crate::FileSystemError::NotADirectory => Error::NotADirectory,
            crate::FileSystemError::AlreadyExists => Error::AlreadyExists,
            crate::FileSystemError::InvalidName => Error::InvalidPath,
            crate::FileSystemError::PermissionDenied => Error::Other("Permission denied".to_string()),
            crate::FileSystemError::Io(e) => Error::Io(e),
            crate::FileSystemError::Layout(e) => Error::Other(format!("Layout error: {:?}", e)),
        }
    }
}

impl From<crate::BlockDeviceError> for Error {
    fn from(err: crate::BlockDeviceError) -> Self {
        match err {
            crate::BlockDeviceError::Io(e) => Error::Io(e),
            crate::BlockDeviceError::InvalidBlockNumber(n) => Error::Other(format!("Invalid block number: {}", n)),
            crate::BlockDeviceError::InvalidBlockSize(s) => Error::Other(format!("Invalid block size: {}", s)),
            crate::BlockDeviceError::ReadOnly => Error::Other("Device is read-only".to_string()),
            crate::BlockDeviceError::DeviceNotOpen => Error::Other("Device is not open".to_string()),
            crate::BlockDeviceError::DeviceClosed => Error::Other("Device is already closed".to_string()),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;