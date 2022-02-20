// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#[cfg(target_os = "macos")]
mod file_drop;
mod web_context;

pub use web_context::WebContextImpl;

#[cfg(target_os = "macos")]
use cocoa::{
  appkit::{NSView, NSViewHeightSizable, NSViewWidthSizable},
  base::YES,
};
use cocoa::{
  base::id,
  foundation::{NSDictionary, NSFastEnumeration},
};

use std::{
  ffi::{c_void, CStr},
  os::raw::c_char,
  ptr::{null, null_mut},
  rc::Rc,
  slice, str,
};

use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use objc::{
  declare::ClassDecl,
  runtime::{Class, Object, Sel},
};
use objc_id::Id;

#[cfg(target_os = "macos")]
use crate::application::platform::macos::WindowExtMacOS;
#[cfg(target_os = "macos")]
use file_drop::{add_file_drop_methods, set_file_drop_handler};

#[cfg(target_os = "ios")]
use crate::application::platform::ios::WindowExtIOS;

use crate::{
  application::{
    dpi::{LogicalSize, PhysicalSize},
    window::Window,
  },
  webview::{FileDropEvent, WebContext, WebViewAttributes},
  Result,
};

use crate::http::{
  Request as HttpRequest, RequestBuilder as HttpRequestBuilder, Response as HttpResponse,
};

pub struct InnerWebView {
  webview: id,
  #[cfg(target_os = "macos")]
  ns_window: id,
  manager: id,
  // Note that if following functions signatures are changed in the future,
  // all fucntions pointer declarations in objc callbacks below all need to get updated.
  ipc_handler_ptr: *mut (Box<dyn Fn(&Window, String)>, Rc<Window>),
  #[cfg(target_os = "macos")]
  file_drop_ptr: *mut (Box<dyn Fn(&Window, FileDropEvent) -> bool>, Rc<Window>),
  protocol_ptrs: Vec<*mut Box<dyn Fn(&HttpRequest) -> Result<HttpResponse>>>,
}

impl InnerWebView {
  pub fn new(
    window: Rc<Window>,
    attributes: WebViewAttributes,
    mut web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    // Function for ipc handler
    extern "C" fn did_receive(this: &Object, _: Sel, _: id, msg: id) {
      // Safety: objc runtime calls are unsafe
      unsafe {
        let function = this.get_ivar::<*mut c_void>("function");
        if !function.is_null() {
          let function =
            &mut *(*function as *mut (Box<dyn for<'r> Fn(&'r Window, String)>, Rc<Window>));
          let body: id = msg_send![msg, body];
          let utf8: *const c_char = msg_send![body, UTF8String];
          let js = CStr::from_ptr(utf8).to_str().expect("Invalid UTF8 string");

          (function.0)(&function.1, js.to_string());
        } else {
          log::warn!("WebView instance is dropped! This handler shouldn't be called.");
        }
      }
    }

    // Task handler for custom protocol
    extern "C" fn start_task(this: &Object, _: Sel, _webview: id, task: id) {
      unsafe {
        let function = this.get_ivar::<*mut c_void>("function");
        if !function.is_null() {
          let function =
            &mut *(*function as *mut Box<dyn for<'s> Fn(&'s HttpRequest) -> Result<HttpResponse>>);

          // Get url request
          let request: id = msg_send![task, request];
          let url: id = msg_send![request, URL];
          let nsstring = {
            let s: id = msg_send![url, absoluteString];
            NSString(Id::from_ptr(s))
          };

          // Get request method (GET, POST, PUT etc...)
          let method = {
            let s: id = msg_send![request, HTTPMethod];
            NSString(Id::from_ptr(s))
          };

          // Prepare our HttpRequest
          let mut http_request = HttpRequestBuilder::new()
            .uri(nsstring.to_str())
            .method(method.to_str());

          // Get body
          let mut sent_form_body = Vec::new();
          let body: id = msg_send![request, HTTPBody];
          let body_stream: id = msg_send![request, HTTPBodyStream];
          if !body.is_null() {
            let length = msg_send![body, length];
            let data_bytes: id = msg_send![body, bytes];
            sent_form_body = slice::from_raw_parts(data_bytes as *const u8, length).to_vec();
          } else if !body_stream.is_null() {
            let _: () = msg_send![body_stream, open];

            while msg_send![body_stream, hasBytesAvailable] {
              sent_form_body.reserve(128);
              let p = sent_form_body.as_mut_ptr().add(sent_form_body.len());
              let read_length = sent_form_body.capacity() - sent_form_body.len();
              let count: usize = msg_send![body_stream, read: p maxLength: read_length];
              sent_form_body.set_len(sent_form_body.len() + count);
            }

            let _: () = msg_send![body_stream, close];
          }

          // Extract all headers fields
          let all_headers: id = msg_send![request, allHTTPHeaderFields];

          // get all our headers values and inject them in our request
          for current_header_ptr in all_headers.iter() {
            let header_field = NSString(Id::from_ptr(current_header_ptr));
            let header_value = NSString(Id::from_ptr(all_headers.valueForKey_(current_header_ptr)));
            // inject the header into the request
            http_request = http_request.header(header_field.to_str(), header_value.to_str());
          }

          // send response
          let final_request = http_request.body(sent_form_body).unwrap();
          if let Ok(sent_response) = function(&final_request) {
            let content = sent_response.body();
            // default: application/octet-stream, but should be provided by the client
            let wanted_mime = sent_response.mimetype();
            // default to 200
            let wanted_status_code = sent_response.status().as_u16() as i32;
            // default to HTTP/1.1
            let wanted_version = format!("{:#?}", sent_response.version());

            let dictionary: id = msg_send![class!(NSMutableDictionary), alloc];
            let headers: id = msg_send![dictionary, initWithCapacity:1];
            if let Some(mime) = wanted_mime {
              let () = msg_send![headers, setObject:NSString::new(mime) forKey: NSString::new("content-type")];
            }
            let () = msg_send![headers, setObject:NSString::new(&content.len().to_string()) forKey: NSString::new("content-length")];

            // add headers
            for (name, value) in sent_response.headers().iter() {
              let header_key = name.to_string();
              if let Ok(value) = value.to_str() {
                let () = msg_send![headers, setObject:NSString::new(value) forKey: NSString::new(&header_key)];
              }
            }

            let urlresponse: id = msg_send![class!(NSHTTPURLResponse), alloc];
            let response: id = msg_send![urlresponse, initWithURL:url statusCode: wanted_status_code HTTPVersion:NSString::new(&wanted_version) headerFields:headers];
            let () = msg_send![task, didReceiveResponse: response];

            // Send data
            let bytes = content.as_ptr() as *mut c_void;
            let data: id = msg_send![class!(NSData), alloc];
            let data: id = msg_send![data, initWithBytes:bytes length:content.len()];
            let () = msg_send![task, didReceiveData: data];
          } else {
            let urlresponse: id = msg_send![class!(NSHTTPURLResponse), alloc];
            let response: id = msg_send![urlresponse, initWithURL:url statusCode:404 HTTPVersion:NSString::new("HTTP/1.1") headerFields:null::<c_void>()];
            let () = msg_send![task, didReceiveResponse: response];
          }
          // Finish
          let () = msg_send![task, didFinish];
        } else {
          log::warn!(
            "Either WebView or WebContext instance is dropped! This handler shouldn't be called."
          );
        }
      }
    }
    extern "C" fn stop_task(_: &Object, _: Sel, _webview: id, _task: id) {}

    // Safety: objc runtime calls are unsafe
    unsafe {
      // Config and custom protocol
      let config: id = msg_send![class!(WKWebViewConfiguration), new];
      let mut protocol_ptrs = Vec::new();
      for (name, function) in attributes.custom_protocols {
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
        let function = Box::into_raw(Box::new(function));
        if let Some(context) = &mut web_context {
          context.os.registered_protocols(function);
        } else {
          protocol_ptrs.push(function);
        }

        (*handler).set_ivar("function", function as *mut _ as *mut c_void);
        let () = msg_send![config, setURLSchemeHandler:handler forURLScheme:NSString::new(&name)];
      }

      // Webview and manager
      let manager: id = msg_send![config, userContentController];
      let cls = match ClassDecl::new("WryWebView", class!(WKWebView)) {
        #[allow(unused_mut)]
        Some(mut decl) => {
          #[cfg(target_os = "macos")]
          add_file_drop_methods(&mut decl);
          decl.register()
        }
        _ => class!(WryWebView),
      };
      let webview: id = msg_send![cls, alloc];
      let _preference: id = msg_send![config, preferences];
      let _yes: id = msg_send![class!(NSNumber), numberWithBool:1];

      #[cfg(any(debug_assertions, feature = "devtool"))]
      if attributes.devtool {
        // Equivalent Obj-C:
        // [[config preferences] setValue:@YES forKey:@"developerExtrasEnabled"];
        let dev = NSString::new("developerExtrasEnabled");
        let _: id = msg_send![_preference, setValue:_yes forKey:dev];
      }

      #[cfg(feature = "transparent")]
      if attributes.transparent {
        let no: id = msg_send![class!(NSNumber), numberWithBool:0];
        // Equivalent Obj-C:
        // [config setValue:@NO forKey:@"drawsBackground"];
        let _: id = msg_send![config, setValue:no forKey:NSString::new("drawsBackground")];
      }

      #[cfg(feature = "fullscreen")]
      // Equivalent Obj-C:
      // [preference setValue:@YES forKey:@"fullScreenEnabled"];
      let _: id = msg_send![_preference, setValue:_yes forKey:NSString::new("fullScreenEnabled")];

      // Initialize webview with zero point
      let zero = CGRect::new(&CGPoint::new(0., 0.), &CGSize::new(0., 0.));
      let _: () = msg_send![webview, initWithFrame:zero configuration:config];

      // Auto-resize on macOS
      #[cfg(target_os = "macos")]
      {
        webview.setAutoresizingMask_(NSViewHeightSizable | NSViewWidthSizable);
      }

      // Message handler
      let ipc_handler_ptr = if let Some(ipc_handler) = attributes.ipc_handler {
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
        let ipc_handler_ptr = Box::into_raw(Box::new((ipc_handler, window.clone())));

        (*handler).set_ivar("function", ipc_handler_ptr as *mut _ as *mut c_void);
        let ipc = NSString::new("ipc");
        let _: () = msg_send![manager, addScriptMessageHandler:handler name:ipc];
        ipc_handler_ptr
      } else {
        null_mut()
      };

      // File drop handling
      #[cfg(target_os = "macos")]
      let file_drop_ptr = match attributes.file_drop_handler {
        // if we have a file_drop_handler defined, use the defined handler
        Some(file_drop_handler) => {
          set_file_drop_handler(webview, window.clone(), file_drop_handler)
        }
        // prevent panic by using a blank handler
        None => set_file_drop_handler(webview, window.clone(), Box::new(|_, _| false)),
      };

      // ns window is required for the print operation
      #[cfg(target_os = "macos")]
      let ns_window = window.ns_window() as id;

      let w = Self {
        webview,
        #[cfg(target_os = "macos")]
        ns_window,
        manager,
        ipc_handler_ptr,
        #[cfg(target_os = "macos")]
        file_drop_ptr,
        protocol_ptrs,
      };

      // Initialize scripts
      w.init(
r#"Object.defineProperty(window, 'ipc', {
  value: Object.freeze({postMessage: function(s) {window.webkit.messageHandlers.ipc.postMessage(s);}})
});"#,
      );
      for js in attributes.initialization_scripts {
        w.init(&js);
      }

      // Set user agent
      if let Some(user_agent) = attributes.user_agent {
        w.set_user_agent(user_agent.as_str())
      }

      // Navigation
      if let Some(url) = attributes.url {
        if url.cannot_be_a_base() {
          let s = url.as_str();
          if let Some(pos) = s.find(',') {
            let (_, path) = s.split_at(pos + 1);
            w.navigate_to_string(path);
          }
        } else {
          w.navigate(url.as_str());
        }
      } else if let Some(html) = attributes.html {
        w.navigate_to_string(&html);
      }

      // Inject the web view into the window as main content
      #[cfg(target_os = "macos")]
      {
        // Tell the webview we use layers (macOS only)
        let _: () = msg_send![webview, setWantsLayer: YES];
        // inject the webview into the window
        let ns_window = window.ns_window() as id;
        let _: () = msg_send![ns_window, setContentView: webview];

        // make sure the window is always on top when we create a new webview
        let app_class = class!(NSApplication);
        let app: id = msg_send![app_class, sharedApplication];
        let _: () = msg_send![app, activateIgnoringOtherApps: YES];
      }

      #[cfg(target_os = "ios")]
      {
        let ui_window = window.ui_window() as id;
        let _: () = msg_send![ui_window, setContentView: webview];
      }

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
      // FIXME: We allow subframe injection because webview2 does and cannot be disabled (currently).
      // once webview2 allows disabling all-frame script injection, forMainFrameOnly should be enabled
      // if it does not break anything. (originally added for isolation pattern).
        msg_send![userscript, initWithSource:NSString::new(js) injectionTime:0 forMainFrameOnly:0];
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

  fn navigate_to_string(&self, html: &str) {
    // Safety: objc runtime calls are unsafe
    unsafe {
      let url: id = msg_send![class!(NSURL), URLWithString: NSString::new("http://localhost")];
      let () = msg_send![self.webview, loadHTMLString:NSString::new(html) baseURL:url];
    }
  }

  fn set_user_agent(&self, user_agent: &str) {
    unsafe {
      let () = msg_send![self.webview, setCustomUserAgent: NSString::new(user_agent)];
    }
  }

  pub fn print(&self) {
    // Safety: objc runtime calls are unsafe
    #[cfg(target_os = "macos")]
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

  pub fn focus(&self) {}

  /// Open the web inspector which is usually called dev tool.
  ///
  /// ## Platform-specific
  ///
  /// - **iOS:** Not implemented.
  pub fn devtool(&self) {
    #[cfg(target_os = "macos")]
    #[cfg(any(debug_assertions, feature = "devtool"))]
    unsafe {
      // taken from <https://github.com/WebKit/WebKit/blob/784f93cb80a386c29186c510bba910b67ce3adc1/Source/WebKit/UIProcess/API/Cocoa/WKWebView.mm#L1939>
      let tool: id = msg_send![self.webview, _inspector];
      let _: id = msg_send![tool, show];
    }
  }

  #[cfg(target_os = "macos")]
  pub fn inner_size(&self, scale_factor: f64) -> PhysicalSize<u32> {
    let view_frame = unsafe { NSView::frame(self.webview) };
    let logical: LogicalSize<f64> =
      (view_frame.size.width as f64, view_frame.size.height as f64).into();
    logical.to_physical(scale_factor)
  }
}

pub fn platform_webview_version() -> Result<String> {
  unsafe {
    let bundle: id =
      msg_send![class!(NSBundle), bundleWithIdentifier: NSString::new("com.apple.WebKit")];
    let dict: id = msg_send![bundle, infoDictionary];
    let webkit_version: id = msg_send![dict, objectForKey: NSString::new("CFBundleVersion")];
    let nsstring = NSString(Id::from_ptr(webkit_version));
    let () = msg_send![bundle, unload];
    Ok(nsstring.to_str().to_string())
  }
}

impl Drop for InnerWebView {
  fn drop(&mut self) {
    // We need to drop handler closures here
    unsafe {
      if !self.ipc_handler_ptr.is_null() {
        let _ = Box::from_raw(self.ipc_handler_ptr);
      }

      #[cfg(target_os = "macos")]
      if !self.file_drop_ptr.is_null() {
        let _ = Box::from_raw(self.file_drop_ptr);
      }

      for ptr in self.protocol_ptrs.iter() {
        if !ptr.is_null() {
          let _ = Box::from_raw(*ptr);
        }
      }

      let _: Id<_> = Id::from_ptr(self.webview);
      #[cfg(target_os = "macos")]
      let _: Id<_> = Id::from_ptr(self.ns_window);
      let _: Id<_> = Id::from_ptr(self.manager);
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
