#[cfg(target_os = "windows")]
#[cfg(feature = "win32")]
mod win32;
#[cfg(target_os = "windows")]
#[cfg(feature = "win32")]
pub use win32::*;
#[cfg(target_os = "windows")]
#[cfg(feature = "winrt")]
mod winrt;
#[cfg(target_os = "windows")]
#[cfg(feature = "winrt")]
pub use winrt::*;
