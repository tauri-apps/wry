pub mod sys;

pub use sys::*;

use std::fmt;

#[derive(Debug)]
pub enum Error {
    WinrtError(winrt::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::WinrtError(e) => format!("{:?}", e).fmt(f),
        }
    }
}

impl std::error::Error for Error {}

impl From<winrt::Error> for Error {
    fn from(error: winrt::Error) -> Self {
        Error::WinrtError(error)
    }
}
