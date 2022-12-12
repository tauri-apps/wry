// Copyright 2020-2022 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::Result;

use crate::webview::web_context::WebContextData;
use http::{Request, Response};
use std::borrow::Cow;

#[derive(Debug)]
pub struct WebContextImpl {
  protocols: Vec<*mut Box<dyn Fn(&Request<Vec<u8>>) -> Result<Response<Cow<'static, [u8]>>>>>,
}

impl WebContextImpl {
  pub fn new(_data: &WebContextData) -> Self {
    Self {
      protocols: Vec::new(),
    }
  }

  pub fn set_allows_automation(&mut self, _flag: bool) {}

  pub fn registered_protocols(
    &mut self,
    handler: *mut Box<dyn Fn(&Request<Vec<u8>>) -> Result<Response<Cow<'static, [u8]>>>>,
  ) {
    self.protocols.push(handler);
  }
}

impl Drop for WebContextImpl {
  fn drop(&mut self) {
    // We need to drop handler closures here
    unsafe {
      for ptr in self.protocols.iter() {
        if !ptr.is_null() {
          let _ = Box::from_raw(*ptr);
        }
      }
    }
  }
}
