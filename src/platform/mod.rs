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

use crate::{Dispatcher, Error, Result, RpcHandler, application::{WindowProxy, FuncCall}};

use std::{collections::HashMap, sync::Mutex};

use once_cell::sync::Lazy;
use serde_json::Value;

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

pub(crate) fn rpc_proxy(js: String, proxy: &WindowProxy, handler: &RpcHandler) -> Result<Option<String>> {
    match serde_json::from_str::<FuncCall>(&js) {
        Ok(mut ev) => {
            let mut response = (handler)(proxy, ev.payload);
            if let Some(mut response) = response.take() {
                if let Some(id) = response.id {
                    let js = if let Some(error) = response.error.take() {
                        match serde_json::to_string(&error) {
                            Ok(retval) => {
                                format!("window.external.rpc._error({}, {})",
                                    id.to_string(), retval)
                            }
                            Err(_) => {
                                format!("window.external.rpc._error({}, null)",
                                    id.to_string())
                            }
                        }
                    } else if let Some(result) = response.result.take() {
                        match serde_json::to_string(&result) {
                            Ok(retval) => {
                                format!("window.external.rpc._result({}, {})",
                                    id.to_string(), retval)
                            }
                            Err(_) => {
                                format!("window.external.rpc._result({}, null)",
                                    id.to_string())
                            }
                        }
                    } else {
                        // No error or result, assume a positive response
                        // with empty result (ACK)
                        format!("window.external.rpc._result({}, null)",
                            id.to_string())
                    };
                    Ok(Some(js))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None) 
            }
        }
        Err(e) => {
            Err(Error::RpcScriptError(e.to_string(), js))
        }
    }
}
