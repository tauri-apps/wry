// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{
  ffi::{c_void, CStr},
  os::raw::c_char,
  path::PathBuf,
  ptr::null,
  rc::Rc,
  slice, str,
};

use cocoa::{
  appkit::{NSView, NSViewHeightSizable, NSViewWidthSizable},
  base::id,
};
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use objc::{
  declare::ClassDecl,
  runtime::{Class, Object, Sel},
};
use objc_id::Id;
use url::Url;

use file_drop::{add_file_drop_methods, set_file_drop_handler};

use crate::{Error, Result, application::window::Window, webview::{mimetype::MimeType, FileDropEvent, RpcRequest, RpcResponse}};



mod file_drop;

pub struct InnerWebView {
  window: Rc<Window>
}

impl InnerWebView {
  pub fn new(
    window: Rc<Window>,
    scripts: Vec<String>,
    url: Option<Url>,
    transparent: bool,
    custom_protocols: Vec<(
      String,
      Box<dyn Fn(&Window, &str) -> Result<Vec<u8>> + 'static>,
    )>,
    rpc_handler: Option<Box<dyn Fn(&Window, RpcRequest) -> Option<RpcResponse>>>,
    file_drop_handler: Option<Box<dyn Fn(&Window, FileDropEvent) -> bool>>,
    _data_directory: Option<PathBuf>,
  ) -> Result<Self> {
    let window = window.clone();
    //let window = &window.window;
    
    if let Some(delegate) = &window.window.delegate {
      delegate.load_url("https://google.com");

      let delegate = Rc::new(delegate);
      return Ok(InnerWebView {
        window,
      });

    }

    

    // todo better handling
    return Err(Error::MessageSender)
  }

  pub fn eval(&self, js: &str) -> Result<()> {
    let window = &self.window.window;
    if let Some(delegate) = &window.delegate {
      delegate.load_url("google.com");
    }

    Ok(())
  }
}
