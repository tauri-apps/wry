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

use crate::{Error, Result, RpcHandler, application::{WindowProxy, RpcRequest}};

pub(crate) fn rpc_proxy(js: String, proxy: &WindowProxy, handler: &RpcHandler) -> Result<Option<String>> {
    let req = serde_json::from_str::<RpcRequest>(&js).map_err(|e| {
        Error::RpcScriptError(e.to_string(), js)
    })?;
    let mut response = (handler)(proxy, req);
    if let Some(mut response) = response.take() {
        if let Some(id) = response.id {
            let js = if let Some(error) = response.error.take() {
                let retval = serde_json::to_string(&error)?;
                format!("window.external.rpc._error({}, {})",
                    id.to_string(), retval)
            } else if let Some(result) = response.result.take() {
                let retval = serde_json::to_string(&result)?;
                format!("window.external.rpc._result({}, {})",
                    id.to_string(), retval)
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
