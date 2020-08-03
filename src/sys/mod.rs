#[cfg(target_os = "linux")]
mod gtk;
#[cfg(target_os = "linux")]
pub use gtk::*;

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "windows")]
pub use windows::*;
