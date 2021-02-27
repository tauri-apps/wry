//! Re-export module that provides window creation and event handling based on each platform.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::*;

#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "windows")]
pub(crate) use win::*;

#[cfg(target_os = "linux")]
pub use gtk::*;
#[cfg(not(target_os = "linux"))]
pub use winit::*;

use crate::{Dispatcher, Result, RpcHandler};

use std::{collections::HashMap, sync::Mutex};

use once_cell::sync::Lazy;
use serde_json::Value;

pub(crate) const RPC_CALLBACK_NAME: &str = "__rpc__";

pub(crate) static CALLBACKS: Lazy<
    Mutex<
        HashMap<
            (i64, String),
            (
                std::boxed::Box<dyn FnMut(&Dispatcher, i32, Vec<Value>) -> Result<()> + Send>,
                Dispatcher,
            ),
        >,
    >,
> = Lazy::new(|| {
    let m = HashMap::new();
    Mutex::new(m)
});

#[deprecated]
#[derive(Debug, Serialize, Deserialize)]
struct RPC {
    id: i32,
    method: String,
    params: Vec<Value>,
}

/// Function call from Javascript.
///
/// If the callback name matches the name for an RPC handler
/// the payload should be passed to the handler transparently.
///
/// Otherwise attempt to find a `Callback` with the same name
/// and pass it the payload `params`.
#[derive(Debug, Serialize, Deserialize)]
pub struct FuncCall {
    callback: String,
    payload: RpcRequest,
}

/// RPC request message.
#[derive(Debug, Serialize, Deserialize)]
pub struct RpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

/// RPC response message.
#[derive(Debug, Serialize, Deserialize)]
pub struct RpcResponse {
    jsonrpc: String,
    id: Option<Value>,
    result: Option<Value>,
    error: Option<Value>,
}
