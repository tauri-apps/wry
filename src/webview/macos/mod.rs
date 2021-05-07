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
  base::{id, YES},
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
  application::{platform::macos::WindowExtMacOS, window::Window},
  webview::{mimetype::MimeType, FileDropEvent, RpcRequest, RpcResponse},
  Result,
};

mod file_drop;

pub struct InnerWebView {
  webview: Id<Object>,
  ns_window: id,
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
    // Function for rpc handler
    extern "C" fn did_receive(this: &Object, _: Sel, _: id, msg: id) {
      // Safety: objc runtime calls are unsafe
      unsafe {
        let function = this.get_ivar::<*mut c_void>("function");
        let function = &mut *(*function
          as *mut (
            Box<dyn for<'r> Fn(&'r Window, RpcRequest) -> Option<RpcResponse>>,
            Rc<Window>,
          ));
        let body: id = msg_send![msg, body];
        let utf8: *const c_char = msg_send![body, UTF8String];
        let js = CStr::from_ptr(utf8).to_str().expect("Invalid UTF8 string");

        match super::rpc_proxy(&function.1, js.to_string(), &function.0) {
          Ok(result) => {
            if let Some(ref script) = result {
              let wv: id = msg_send![msg, webView];
              let js = NSString::new(script);
              let _: id =
                msg_send![wv, evaluateJavaScript:js completionHandler:null::<*const c_void>()];
            }
          }
          Err(e) => {
            eprintln!("{}", e);
          }
        }
      }
    }

    // Task handler for custom protocol
    extern "C" fn start_task(this: &Object, _: Sel, _webview: id, task: id) {
      unsafe {
        let function = this.get_ivar::<*mut c_void>("function");
        let function = &mut *(*function
          as *mut (
            Box<dyn for<'r, 's> Fn(&'r Window, &'s str) -> Result<Vec<u8>>>,
            Rc<Window>,
          ));

        // Get url request
        let request: id = msg_send![task, request];
        let url: id = msg_send![request, URL];
        let nsstring = {
          let s: id = msg_send![url, absoluteString];
          NSString(Id::from_ptr(s))
        };
        let uri = nsstring.to_str();

        // Send response
        if let Ok(content) = function.0(&function.1, uri) {
          let mime = MimeType::parse(&content, uri);
          let nsurlresponse: id = msg_send![class!(NSURLResponse), alloc];
          let response: id = msg_send![nsurlresponse, initWithURL:url MIMEType:NSString::new(&mime)
                        expectedContentLength:content.len() textEncodingName:null::<c_void>()];
          let () = msg_send![task, didReceiveResponse: response];

          // Send data
          let bytes = content.as_ptr() as *mut c_void;
          let data: id = msg_send![class!(NSData), alloc];
          let data: id = msg_send![data, initWithBytes:bytes length:content.len()];
          let () = msg_send![task, didReceiveData: data];

          // Finish
          let () = msg_send![task, didFinish];
        }
      }
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
      let zero = CGRect::new(&CGPoint::new(0., 0.), &CGSize::new(0., 0.));
      let _: () = msg_send![webview, initWithFrame:zero configuration:config];
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

      // ns window is required for the print operation
      let ns_window = window.ns_window() as id;

      let w = Self {
        webview: Id::from_ptr(webview),
        ns_window,
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
      // Tell the webview we use layers
      let _: () = msg_send![webview, setWantsLayer: YES];
      // Inject the web view into the window as main content
      let _: () = msg_send![ns_window, setContentView: webview];
      // make sure the window is always on top when we create a new webview
      let app_class = class!(NSApplication);
      let app: id = msg_send![app_class, sharedApplication];
      let _: () = msg_send![app, activateIgnoringOtherApps: YES];

      Ok(w)
    }
  }

  pub fn eval(&self, js: &str) -> Result<()> {
    // Safety: objc runtime calls are unsafe
    unsafe {
      let _: id = msg_send![self.webview, evaluateJavaScript:NSString::new(js) completionHandler:null::<*const c_void>()];
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
      let _: () = msg_send![self.manager, addUserScript: script];
    }
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

  pub fn print(&self) {
    // Safety: objc runtime calls are unsafe
    unsafe {
      // Create a shared print info
      let print_info: id = msg_send![class!(NSPrintInfo), sharedPrintInfo];
      let print_info: id = msg_send![print_info, init];
      // Create new print operation from the webview content
      let print_operation: id = msg_send![self.webview, printOperationWithPrintInfo: print_info];
      // Allow the modal to detach from the current thread and be non-blocker
      let () = msg_send![print_operation, setCanSpawnSeparateThread: YES];
      // Launch the modal
      let () = msg_send![print_operation, runOperationModalForWindow: self.ns_window delegate: null::<*const c_void>() didRunSelector: null::<*const c_void>() contextInfo: null::<*const c_void>()];
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
