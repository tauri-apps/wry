// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{
  ffi::{c_void, CStr},
  os::raw::c_char,
  path::PathBuf,
  ptr::{null, null_mut},
  rc::Rc,
  slice, str,
};
use cacao::webview::{WebView, WebViewConfig, WebViewDelegate};
use cocoa::{appkit::{NSOpenGLContext, NSTabView, NSView, NSViewHeightSizable, NSViewWidthSizable}, base::id};
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use objc::{
  declare::ClassDecl,
  runtime::{Class, Object, Sel},
};
use objc_id::Id;
use url::Url;

use file_drop::{add_file_drop_methods, set_file_drop_handler};

use crate::{Error, Result, application::{platform::macos::WindowExtMacOS, window::Window}, webview::{mimetype::MimeType, FileDropEvent, RpcRequest, RpcResponse}};

mod file_drop;
pub struct WebViewInstance {
  window: Rc<Window>,
  rpc_handler: Option<Box<dyn Fn(&Window, RpcRequest) -> Option<RpcResponse>>>,
  custom_protocols: Vec<(
    String,
    Box<dyn Fn(&Window, &str) -> Result<Vec<u8>> + 'static>,
  )>,
}

impl WebViewInstance {
  fn new(window: Rc<Window>, rpc_handler: Option<Box<dyn Fn(&Window, RpcRequest) -> Option<RpcResponse>>>, custom_protocols: Vec<(
    String,
    Box<dyn Fn(&Window, &str) -> Result<Vec<u8>> + 'static>,
  )>) -> Self {
    Self {
      window,
      rpc_handler,
      custom_protocols,
    }
  }

  fn execute_script() {}
}

impl WebViewDelegate for WebViewInstance {


  fn on_message(&self, name: &str, body: &str) {
      match name {
        // RPC
        "external" => {
          if let Some(handler) = &self.rpc_handler {
            match super::rpc_proxy(&self.window.clone(), body.to_string(), &handler) {
              Ok(result) => {
                if let Some(ref script) = result {
                  // TODO: ALLOW SEND SCRIPT BACK TO WEBVIEW
                  //let wv: id = msg_send![msg, webView];
                  //let js = NSString::new(script);
                  //let _: id =
                    //msg_send![wv, evaluateJavaScript:js completionHandler:null::<*const c_void>()];
                }
              }
              Err(e) => {
                eprintln!("{}", e);
              }
            }
          }
        },
        _ => {

        }
      }
  }

  fn on_custom_protocol_request(&self, path: &str) -> Option<Vec<u8>> {
    for (protocol, callback) in &self.custom_protocols {
      let protocol_len = protocol.len();
      let matched_path = path.chars().into_iter().take(protocol_len).collect::<String>();
      if matched_path.as_str() == protocol {
        if let Ok(result) = callback(&self.window, path) {
          return Some(result);
        }
      }
    }

    None
  }
}

pub struct InnerWebView {
  webview: WebView<WebViewInstance>
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
   
    let mut webview_config = WebViewConfig::default();

    // intialize scripts
    let script =  r#"window.external = {
          invoke: function(s) {
              window.webkit.messageHandlers.external.postMessage(s);
          },
      };
      window.addEventListener("keydown", function(e) {
          if (e.defaultPrevented) {
              return;
          }
        if (e.metaKey) {
              switch(e.key) {
                  case "x":
                      document.execCommand("cut");
                      e.preventDefault();
                      break;
                  case "c":
                      document.execCommand("copy");
                      e.preventDefault();
                      break;
                  case "v":
                      document.execCommand("paste");
                      e.preventDefault();
                      break;
                  default:
                      return;
              }
          }
      }, true);"#;
    webview_config.add_user_script(script, cacao::webview::InjectAt::Start, true);

    // add user scripts
    for js in scripts {
      webview_config.add_user_script(&js, cacao::webview::InjectAt::Start, false);
    }

    // register custom protocols
    for (protocol,_) in &custom_protocols  {
      webview_config.add_custom_protocol(&protocol);
    }

    // regiser `external` (rpc handler)
    webview_config.add_handler("external");

    let webview_instance = WebViewInstance::new(window.clone(), rpc_handler, custom_protocols);

    let webview = WebView::with(webview_config, webview_instance);
    
    // Safety: objc runtime calls are unsafe
      webview.objc.with_mut(move |webview_obj| {
        unsafe {
          // this is a weird workaround
          // grabbing winit objc reference
          let view = window.ns_window() as id;
          // inject cacao webview objc reference as main window
          let _: () = msg_send![view, setContentView:webview_obj];
        }
      });

      if let Some(url) = url {
        webview.load_url(url.as_str());
      }

      let final_webview = Self { 
        webview,
      };
  
 
    Ok(final_webview)
  }

  pub fn eval(&self, js: &str) -> Result<()> {
    // Safety: objc runtime calls are unsafe
    unsafe {
      //let _: id = msg_send![self.webview, evaluateJavaScript:NSString::new(js) completionHandler:null::<*const c_void>()];
    }
    Ok(())
  }

  fn init(&self, js: &str) {
    // Safety: objc runtime calls are unsafe
    // Equivalent Obj-C:
    // [manager addUserScript:[[WKUserScript alloc] initWithSource:[NSString stringWithUTF8String:js.c_str()] injectionTime:WKUserScriptInjectionTimeAtDocumentStart forMainFrameOnly:YES]]
    unsafe {
      let userscript: id = msg_send![class!(WKUserScript), alloc];
      let script: id =
        msg_send![userscript, initWithSource:NSString::new(js) injectionTime:0 forMainFrameOnly:1];
      //let _: () = msg_send![self.manager, addUserScript: script];
    }
  }

  fn navigate(&self, url: &str) {
    // Safety: objc runtime calls are unsafe
    unsafe {
      let url: id = msg_send![class!(NSURL), URLWithString: NSString::new(url)];
      let request: id = msg_send![class!(NSURLRequest), requestWithURL: url];
      //let () = msg_send![self.webview, loadRequest: request];
    }
  }

  fn navigate_to_string(&self, url: &str) {
    // Safety: objc runtime calls are unsafe
    unsafe {
      let empty: id = msg_send![class!(NSURL), URLWithString: NSString::new("")];
      //let () = msg_send![self.webview, loadHTMLString:NSString::new(url) baseURL:empty];
    }
  }
}



const UTF8_ENCODING: usize = 4;

struct NSString(Id<Object>);

impl NSString {
  fn new(s: &str) -> Self {
    // Safety: objc runtime calls are unsafe
    NSString(unsafe {
      let nsstring: id = msg_send![class!(NSString), alloc];
      Id::from_ptr(
        msg_send![nsstring, initWithBytes:s.as_ptr() length:s.len() encoding:UTF8_ENCODING],
      )
    })
  }

  fn to_str(&self) -> &str {
    unsafe {
      let bytes: *const c_char = msg_send![self.0, UTF8String];
      let len = msg_send![self.0, lengthOfBytesUsingEncoding: UTF8_ENCODING];
      let bytes = slice::from_raw_parts(bytes as *const u8, len);
      str::from_utf8_unchecked(bytes)
    }
  }
}
