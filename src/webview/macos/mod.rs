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

use crate::{
  application::window::Window,
  webview::{mimetype::MimeType, FileDropEvent, RpcRequest, RpcResponse},
  Error, Result,
};

mod file_drop;

pub struct InnerWebView {
  webview: Id<Object>,
  manager: id,
  rpc_handler_ptr: *mut (
    Box<dyn Fn(&Window, RpcRequest) -> Option<RpcResponse>>,
    Rc<Window>,
  ),
  file_drop_ptr: *mut (Box<dyn Fn(&Window, FileDropEvent) -> bool>, Rc<Window>),
  protocol_ptrs: Vec<*mut (Box<dyn Fn(&Window, &str) -> Result<Vec<u8>>>, Rc<Window>)>,
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
      return Ok(InnerWebView { window });
    }
    extern "C" fn stop_task(_: &Object, _: Sel, _webview: id, _task: id) {}

    // Safety: objc runtime calls are unsafe
    unsafe {
      // Config and custom protocol
      let config: id = msg_send![class!(WKWebViewConfiguration), new];
      let mut protocol_ptrs = Vec::new();
      for (name, function) in custom_protocols {
        let scheme_name = format!("{}URLSchemeHandler", name);
        let cls = ClassDecl::new(&scheme_name, class!(NSObject));
        let cls = match cls {
          Some(mut cls) => {
            cls.add_ivar::<*mut c_void>("function");
            cls.add_method(
              sel!(webView:startURLSchemeTask:),
              start_task as extern "C" fn(&Object, Sel, id, id),
            );
            cls.add_method(
              sel!(webView:stopURLSchemeTask:),
              stop_task as extern "C" fn(&Object, Sel, id, id),
            );
            cls.register()
          }
          None => Class::get(&scheme_name).expect("Failed to get the class definition"),
        };
        let handler: id = msg_send![cls, new];
        let w = window.clone();
        let function = Box::into_raw(Box::new((function, w)));
        protocol_ptrs.push(function);

        (*handler).set_ivar("function", function as *mut _ as *mut c_void);
        let () = msg_send![config, setURLSchemeHandler:handler forURLScheme:NSString::new(&name)];
      }

      // Webview and manager
      let manager: id = msg_send![config, userContentController];
      let cls = match ClassDecl::new("WryWebView", class!(WKWebView)) {
        Some(mut decl) => {
          add_file_drop_methods(&mut decl);
          decl.register()
        }
        _ => class!(WryWebView),
      };
      let webview: id = msg_send![cls, alloc];
      let preference: id = msg_send![config, preferences];
      let yes: id = msg_send![class!(NSNumber), numberWithBool:1];
      let no: id = msg_send![class!(NSNumber), numberWithBool:0];

      debug_assert_eq!(
        {
          // Equivalent Obj-C:
          // [[config preferences] setValue:@YES forKey:@"developerExtrasEnabled"];
          let dev = NSString::new("developerExtrasEnabled");
          let _: id = msg_send![preference, setValue:yes forKey:dev];
        },
        ()
      );

      if transparent {
        // Equivalent Obj-C:
        // [config setValue:@NO forKey:@"drawsBackground"];
        let _: id = msg_send![config, setValue:no forKey:NSString::new("drawsBackground")];
      }

      // Resize
      let size = window.inner_size().to_logical(window.scale_factor());
      let rect = CGRect::new(&CGPoint::new(0., 0.), &CGSize::new(size.width, size.height));
      let _: () = msg_send![webview, initWithFrame:rect configuration:config];
      webview.setAutoresizingMask_(NSViewHeightSizable | NSViewWidthSizable);

      // Message handler
      let rpc_handler_ptr = if let Some(rpc_handler) = rpc_handler {
        let cls = ClassDecl::new("WebViewDelegate", class!(NSObject));
        let cls = match cls {
          Some(mut cls) => {
            cls.add_ivar::<*mut c_void>("function");
            cls.add_method(
              sel!(userContentController:didReceiveScriptMessage:),
              did_receive as extern "C" fn(&Object, Sel, id, id),
            );
            cls.register()
          }
          None => class!(WebViewDelegate),
        };
        let handler: id = msg_send![cls, new];
        let rpc_handler_ptr = Box::into_raw(Box::new((rpc_handler, window.clone())));

        (*handler).set_ivar("function", rpc_handler_ptr as *mut _ as *mut c_void);
        let external = NSString::new("external");
        let _: () = msg_send![manager, addScriptMessageHandler:handler name:external];
        rpc_handler_ptr
      } else {
        null_mut()
      };

      // File drop handling
      let file_drop_ptr = match file_drop_handler {
        // if we have a file_drop_handler defined, use the defined handler
        Some(file_drop_handler) => {
          set_file_drop_handler(webview, window.clone(), file_drop_handler)
        }
        // prevent panic by using a blank handler
        None => set_file_drop_handler(webview, window.clone(), Box::new(|_, _| false)),
      };

      let w = Self {
        webview: Id::from_ptr(webview),
        manager,
        rpc_handler_ptr,
        file_drop_ptr,
        protocol_ptrs,
      };

      // Initialize scripts
      w.init(
        r#"window.external = {
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
                }, true);"#,
      );
      for js in scripts {
        w.init(&js);
      }

      // Navigation
      if let Some(url) = url {
        if url.cannot_be_a_base() {
          let s = url.as_str();
          if let Some(pos) = s.find(',') {
            let (_, path) = s.split_at(pos + 1);
            w.navigate_to_string(path);
          }
        } else {
          w.navigate(url.as_str());
        }
      }

      let view = window.ns_view() as id;
      view.addSubview_(webview);

    // todo better handling
    return Err(Error::MessageSender);
  }

  pub fn eval(&self, js: &str) -> Result<()> {
    let window = &self.window.window;
    if let Some(delegate) = &window.delegate {
      delegate.load_url("google.com");
    }

  fn navigate(&self, url: &str) {
    // Safety: objc runtime calls are unsafe
    unsafe {
      let url: id = msg_send![class!(NSURL), URLWithString: NSString::new(url)];
      let request: id = msg_send![class!(NSURLRequest), requestWithURL: url];
      let () = msg_send![self.webview, loadRequest: request];
    }
  }

  fn navigate_to_string(&self, url: &str) {
    // Safety: objc runtime calls are unsafe
    unsafe {
      let empty: id = msg_send![class!(NSURL), URLWithString: NSString::new("")];
      let () = msg_send![self.webview, loadHTMLString:NSString::new(url) baseURL:empty];
    }
  }
}

impl Drop for InnerWebView {
  fn drop(&mut self) {
    // We need to drop handler closures here
    unsafe {
      if !self.rpc_handler_ptr.is_null() {
        let _ = Box::from_raw(self.rpc_handler_ptr);
      }

      if !self.file_drop_ptr.is_null() {
        let _ = Box::from_raw(self.file_drop_ptr);
      }

      for ptr in self.protocol_ptrs.iter() {
        if !ptr.is_null() {
          let _ = Box::from_raw(*ptr);
        }
      }
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
