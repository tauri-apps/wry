use std::{
  collections::HashSet, ffi::c_void, os::unix::prelude::RawFd, ptr::null_mut, rc::Rc, sync::RwLock,
};

use crate::{application::window::Window, Result};

use super::{WebContext, WebViewAttributes};

use crossbeam_channel::*;
use jni::{
  objects::{JClass, JObject},
  sys::jobject,
  JNIEnv,
};
use once_cell::sync::{Lazy, OnceCell};
use tao::platform::android::ndk_glue::*;

pub struct InnerWebView {
  pub window: Rc<Window>,
}

impl InnerWebView {
  pub fn new(
    window: Rc<Window>,
    attributes: WebViewAttributes,
    _web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    let sender = &CHANNEL.0;
    let WebViewAttributes {
      url,
      initialization_scripts,
      ipc_handler,
      devtools,
      ..
    } = attributes;

    if devtools {
      #[cfg(any(debug_assertions, feature = "devtools"))]
      sender.send(WebViewMessage::Devtools).unwrap_or({
        log::warn!("Error when sending WebViewMessage::Devtools");
      });
    }

    if let Some(u) = url {
      let mut url_string = String::from(u.as_str());
      let name = u.scheme();
      // TODO: Expands custom protocols with real configurations
      let schemes = vec!["assets", "res"];
      if schemes.contains(&name) {
        url_string = u
          .as_str()
          .replace(&format!("{}://", name), "https://tauri.mobile/")
      }
      sender.send(WebViewMessage::LoadUrl(url_string)).unwrap_or({
        log::warn!("Error when sending WebViewMessage::LoadUrl");
      });
    }

    if !initialization_scripts.is_empty() {
      sender
        .send(WebViewMessage::Scripts(initialization_scripts))
        .unwrap_or({
          log::warn!("Error when sending WebViewMessage::Scripts");
        });
    }

    sender.send(WebViewMessage::Done).unwrap_or({
      log::warn!("Error when sending WebViewMessage::Done");
    });

    let w = window.clone();
    if let Some(i) = ipc_handler {
      IPC.get_or_init(move || UnsafeIpc::new(Box::into_raw(Box::new(i)) as *mut _, w));
    }

    Ok(Self { window })
  }

  pub fn print(&self) {}

  pub fn eval(&self, _js: &str) -> Result<()> {
    Ok(())
  }

  pub fn focus(&self) {}

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn open_devtools(&self) {}

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn close_devtools(&self) {}

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn is_devtools_open(&self) -> bool {
    false
  }

  pub fn zoom(&self, scale_factor: f64) {}
}

pub fn platform_webview_version() -> Result<String> {
  todo!()
}
