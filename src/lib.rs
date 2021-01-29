#[macro_use]
extern crate serde;
#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

mod sys;

pub use crate::sys::*;

use std::fmt;
use std::ffi::c_void;
use std::os::raw::c_char;

#[cfg(target_os = "windows")]
extern {
    fn ivector(js: *const c_char) -> *mut c_void;
}

#[derive(Debug)]
pub enum Error {
    InitError,
    NulError(std::ffi::NulError),
    #[cfg(target_os = "windows")]
    WinrtError(windows::Error),
    #[cfg(target_os = "windows")]
    OsError(winit::error::OsError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InitError => "Fail to initialize instance".fmt(f),
            Error::NulError(e) => e.fmt(f),
            #[cfg(target_os = "windows")]
            Error::WinrtError(e) => format!("{:?}", e).fmt(f),
            #[cfg(target_os = "windows")]
            Error::OsError(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(target_os = "windows")]
impl From<windows::Error> for Error {
    fn from(error: windows::Error) -> Self {
        Error::WinrtError(error)
    }
}

impl From<std::ffi::NulError> for Error {
    fn from(error: std::ffi::NulError) -> Self {
        Error::NulError(error)
    }
}

#[cfg(target_os = "windows")]
impl From<winit::error::OsError> for Error {
    fn from(error: winit::error::OsError) -> Self {
        Error::OsError(error)
    }
}
