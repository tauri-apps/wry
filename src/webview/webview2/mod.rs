#[cfg(feature = "win32")]
mod win32;
#[cfg(feature = "win32")]
pub use win32::*;
#[cfg(feature = "winrt")]
mod winrt;
#[cfg(feature = "winrt")]
pub use winrt::*;

use super::rpc_proxy;
