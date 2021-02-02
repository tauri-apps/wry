#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "windows")]
pub use win::*;

use std::{
    collections::HashMap,
    sync::Mutex,
};

use once_cell::sync::Lazy;

static CALLBACKS: Lazy<
    Mutex<HashMap<String, Box<dyn FnMut(i8, Vec<String>) -> i32 + Sync + Send>>>,
> = Lazy::new(|| {
    let m = HashMap::new();
    Mutex::new(m)
});

#[derive(Debug, Serialize, Deserialize)]
struct RPC {
    id: i8,
    method: String,
    params: Vec<String>,
}