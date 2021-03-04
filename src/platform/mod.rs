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

use std::rc::Rc;
use serde_json::Value;

use crate::{Error, Result, RpcHandler, application::{WindowProxy, RpcRequest, RpcResponse}};

// Helper so all platforms handle RPC messages consistently.
pub(crate) fn rpc_proxy(js: String, proxy: Rc<WindowProxy>, handler: &RpcHandler) -> Result<Option<String>> {
    let req = serde_json::from_str::<RpcRequest>(&js).map_err(|e| {
        Error::RpcScriptError(e.to_string(), js)
    })?;

    let mut response = (handler)(proxy, req);
    // Got a synchronous response so convert it to a script to be evaluated
    if let Some(mut response) = response.take() {
        if let Some(id) = response.id {
            let js = if let Some(error) = response.error.take() {
                RpcResponse::into_error_script(id, error)?
            } else if let Some(result) = response.result.take() {
                RpcResponse::into_result_script(id, result)?
            } else {
                // No error or result, assume a positive response
                // with empty result (ACK)
                RpcResponse::into_result_script(id, Value::Null)?
            };
            Ok(Some(js))
        } else {
            Ok(None)
        }
    } else {
        Ok(None) 
    }
}
