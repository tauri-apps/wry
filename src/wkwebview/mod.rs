// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

mod download;
#[cfg(target_os = "macos")]
mod drag_drop;
mod navigation;
#[cfg(feature = "mac-proxy")]
mod proxy;
#[cfg(target_os = "macos")]
mod synthetic_mouse_events;

#[cfg(target_os = "macos")]
use cocoa::appkit::{NSView, NSViewHeightSizable, NSViewMinYMargin, NSViewWidthSizable};
use cocoa::{
  base::{id, nil, NO, YES},
  foundation::{NSDictionary, NSFastEnumeration, NSInteger},
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use std::{
  borrow::Cow,
  ffi::{c_void, CStr},
  os::raw::c_char,
  ptr::{null, null_mut},
  slice, str,
  sync::{Arc, Mutex},
};

use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use objc::{
  declare::ClassDecl,
  runtime::{Class, Object, Sel, BOOL},
};
use objc_id::Id;

#[cfg(target_os = "macos")]
use drag_drop::{add_drag_drop_methods, set_drag_drop_handler};

#[cfg(feature = "mac-proxy")]
use crate::{
  proxy::ProxyConfig,
  wkwebview::proxy::{
    nw_endpoint_t, nw_proxy_config_create_http_connect, nw_proxy_config_create_socksv5,
  },
};

use crate::{
  wkwebview::{
    download::{
      add_download_methods, download_did_fail, download_did_finish, download_policy,
      set_download_delegate,
    },
    navigation::{add_navigation_mathods, drop_navigation_methods, set_navigation_methods},
  },
  Error, PageLoadEvent, Rect, RequestAsyncResponder, Result, WebContext, WebViewAttributes, RGBA,
};

use http::{
  header::{CONTENT_LENGTH, CONTENT_TYPE},
  status::StatusCode,
  version::Version,
  Request, Response as HttpResponse,
};

const IPC_MESSAGE_HANDLER_NAME: &str = "ipc";
#[cfg(target_os = "macos")]
const ACCEPT_FIRST_MOUSE: &str = "accept_first_mouse";

const NS_JSON_WRITING_FRAGMENTS_ALLOWED: u64 = 4;

pub(crate) struct InnerWebView {
  pub webview: id,
  pub manager: id,
  is_child: bool,
  pending_scripts: Arc<Mutex<Option<Vec<String>>>>,
  // Note that if following functions signatures are changed in the future,
  // all functions pointer declarations in objc callbacks below all need to get updated.
  ipc_handler_ptr: *mut Box<dyn Fn(Request<String>)>,
  document_title_changed_handler: *mut Box<dyn Fn(String)>,
  navigation_decide_policy_ptr: *mut Box<dyn Fn(String, bool) -> bool>,
  page_load_handler: *mut Box<dyn Fn(PageLoadEvent)>,
  #[cfg(target_os = "macos")]
  drag_drop_ptr: *mut Box<dyn Fn(crate::DragDropEvent) -> bool>,
  download_delegate: id,
  protocol_ptrs: Vec<*mut Box<dyn Fn(Request<Vec<u8>>, RequestAsyncResponder)>>,
}

impl InnerWebView {
  pub fn new(
    window: &impl HasWindowHandle,
    attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    _web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    let ns_view = match window.window_handle()?.as_raw() {
      #[cfg(target_os = "macos")]
      RawWindowHandle::AppKit(w) => w.ns_view.as_ptr(),
      #[cfg(target_os = "ios")]
      RawWindowHandle::UiKit(w) => w.ui_view.as_ptr(),
      _ => return Err(Error::UnsupportedWindowHandle),
    };

    Self::new_ns_view(ns_view as _, attributes, pl_attrs, _web_context, false)
  }

  pub fn new_as_child(
    window: &impl HasWindowHandle,
    attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    _web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    let ns_view = match window.window_handle()?.as_raw() {
      #[cfg(target_os = "macos")]
      RawWindowHandle::AppKit(w) => w.ns_view.as_ptr(),
      #[cfg(target_os = "ios")]
      RawWindowHandle::UiKit(w) => w.ui_view.as_ptr(),
      _ => return Err(Error::UnsupportedWindowHandle),
    };

    Self::new_ns_view(ns_view as _, attributes, pl_attrs, _web_context, true)
  }

  fn new_ns_view(
    ns_view: id,
    attributes: WebViewAttributes,
    _pl_attrs: super::PlatformSpecificWebViewAttributes,
    _web_context: Option<&mut WebContext>,
    is_child: bool,
  ) -> Result<Self> {
    // Function for ipc handler
    extern "C" fn did_receive(this: &Object, _: Sel, _: id, msg: id) {
      // Safety: objc runtime calls are unsafe
      unsafe {
        #[cfg(feature = "tracing")]
        let _span = tracing::info_span!("wry::ipc::handle").entered();

        let function = this.get_ivar::<*mut c_void>("function");
        if !function.is_null() {
          let function = &mut *(*function as *mut Box<dyn Fn(Request<String>)>);
          let body: id = msg_send![msg, body];
          let is_string: bool = msg_send![body, isKindOfClass: class!(NSString)];
          if is_string {
            let js_utf8: *const c_char = msg_send![body, UTF8String];

            let frame_info: id = msg_send![msg, frameInfo];
            let request: id = msg_send![frame_info, request];
            let url: id = msg_send![request, URL];
            let absolute_url: id = msg_send![url, absoluteString];
            let url_utf8: *const c_char = msg_send![absolute_url, UTF8String];

            if let (Ok(url), Ok(js)) = (
              CStr::from_ptr(url_utf8).to_str(),
              CStr::from_ptr(js_utf8).to_str(),
            ) {
              (function)(Request::builder().uri(url).body(js.to_string()).unwrap());
              return;
            }
          }
        }

        #[cfg(feature = "tracing")]
        tracing::warn!("WebView received invalid IPC call.");
      }
    }

    // Task handler for custom protocol
    extern "C" fn start_task(this: &Object, _: Sel, _webview: id, task: id) {
      unsafe {
        #[cfg(feature = "tracing")]
        let span = tracing::info_span!("wry::custom_protocol::handle", uri = tracing::field::Empty)
          .entered();
        let function = this.get_ivar::<*mut c_void>("function");
        if !function.is_null() {
          let function =
            &mut *(*function as *mut Box<dyn Fn(Request<Vec<u8>>, RequestAsyncResponder)>);

          // Get url request
          let request: id = msg_send![task, request];
          let url: id = msg_send![request, URL];

          let uri_nsstring = {
            let s: id = msg_send![url, absoluteString];
            NSString(s)
          };
          let uri = uri_nsstring.to_str();

          #[cfg(feature = "tracing")]
          span.record("uri", uri);

          // Get request method (GET, POST, PUT etc...)
          let method = {
            let s: id = msg_send![request, HTTPMethod];
            NSString(s)
          };

          // Prepare our HttpRequest
          let mut http_request = Request::builder().uri(uri).method(method.to_str());

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
            let header_field = NSString(current_header_ptr);
            let header_value = NSString(all_headers.valueForKey_(current_header_ptr));

            // inject the header into the request
            http_request = http_request.header(header_field.to_str(), header_value.to_str());
          }

          let respond_with_404 = || {
            let urlresponse: id = msg_send![class!(NSHTTPURLResponse), alloc];
            let response: id = msg_send![urlresponse, initWithURL:url statusCode:StatusCode::NOT_FOUND HTTPVersion:NSString::new(format!("{:#?}", Version::HTTP_11).as_str()) headerFields:null::<c_void>()];
            let () = msg_send![task, didReceiveResponse: response];
            // Finish
            let () = msg_send![task, didFinish];
          };

          // send response
          match http_request.body(sent_form_body) {
            Ok(final_request) => {
              let responder: Box<dyn FnOnce(HttpResponse<Cow<'static, [u8]>>)> = Box::new(
                move |sent_response| {
                  let content = sent_response.body();
                  // default: application/octet-stream, but should be provided by the client
                  let wanted_mime = sent_response.headers().get(CONTENT_TYPE);
                  // default to 200
                  let wanted_status_code = sent_response.status().as_u16() as i32;
                  // default to HTTP/1.1
                  let wanted_version = format!("{:#?}", sent_response.version());

                  let dictionary: id = msg_send![class!(NSMutableDictionary), alloc];
                  let headers: id = msg_send![dictionary, initWithCapacity:1];
                  if let Some(mime) = wanted_mime {
                    let () = msg_send![headers, setObject:NSString::new(mime.to_str().unwrap()) forKey: NSString::new(CONTENT_TYPE.as_str())];
                  }
                  let () = msg_send![headers, setObject:NSString::new(&content.len().to_string()) forKey: NSString::new(CONTENT_LENGTH.as_str())];

                  // add headers
                  for (name, value) in sent_response.headers().iter() {
                    let header_key = name.as_str();
                    if let Ok(value) = value.to_str() {
                      let () = msg_send![headers, setObject:NSString::new(value) forKey: NSString::new(header_key)];
                    }
                  }

                  let urlresponse: id = msg_send![class!(NSHTTPURLResponse), alloc];
                  let response: id = msg_send![urlresponse, initWithURL:url statusCode: wanted_status_code HTTPVersion:NSString::new(&wanted_version) headerFields:headers];
                  let () = msg_send![task, didReceiveResponse: response];

                  // Send data
                  let bytes = content.as_ptr() as *mut c_void;
                  let data: id = msg_send![class!(NSData), alloc];
                  let data: id = msg_send![data, initWithBytesNoCopy:bytes length:content.len() freeWhenDone: if content.len() == 0 { NO } else { YES }];
                  let () = msg_send![task, didReceiveData: data];
                  // Finish
                  let () = msg_send![task, didFinish];
                },
              );

              #[cfg(feature = "tracing")]
              let _span = tracing::info_span!("wry::custom_protocol::call_handler").entered();
              function(final_request, RequestAsyncResponder { responder });
            }
            Err(_) => respond_with_404(),
          };
        } else {
          #[cfg(feature = "tracing")]
          tracing::warn!(
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

      // Incognito mode
      let data_store: id = if attributes.incognito {
        msg_send![class!(WKWebsiteDataStore), nonPersistentDataStore]
      } else {
        msg_send![class!(WKWebsiteDataStore), defaultDataStore]
      };

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
        protocol_ptrs.push(function);

        (*handler).set_ivar("function", function as *mut _ as *mut c_void);
        let () = msg_send![config, setURLSchemeHandler:handler forURLScheme:NSString::new(&name)];
      }

      // WebView and manager
      let manager: id = msg_send![config, userContentController];
      let cls = match ClassDecl::new("WryWebView", class!(WKWebView)) {
        #[allow(unused_mut)]
        Some(mut decl) => {
          #[cfg(target_os = "macos")]
          {
            add_drag_drop_methods(&mut decl);
            synthetic_mouse_events::setup(&mut decl);
            decl.add_ivar::<bool>(ACCEPT_FIRST_MOUSE);
            decl.add_method(
              sel!(acceptsFirstMouse:),
              accept_first_mouse as extern "C" fn(&Object, Sel, id) -> BOOL,
            );

            extern "C" fn accept_first_mouse(this: &Object, _sel: Sel, _event: id) -> BOOL {
              unsafe {
                let accept: bool = *this.get_ivar(ACCEPT_FIRST_MOUSE);
                if accept {
                  YES
                } else {
                  NO
                }
              }
            }
          }
          decl.register()
        }
        _ => class!(WryWebView),
      };
      let webview: id = msg_send![cls, alloc];

      let () = msg_send![config, setWebsiteDataStore: data_store];
      let _preference: id = msg_send![config, preferences];
      let _yes: id = msg_send![class!(NSNumber), numberWithBool:1];

      #[cfg(feature = "mac-proxy")]
      if let Some(proxy_config) = attributes.proxy_config {
        let proxy_config = match proxy_config {
          ProxyConfig::Http(endpoint) => {
            let nw_endpoint = nw_endpoint_t::try_from(endpoint).unwrap();
            nw_proxy_config_create_http_connect(nw_endpoint, nil)
          }
          ProxyConfig::Socks5(endpoint) => {
            let nw_endpoint = nw_endpoint_t::try_from(endpoint).unwrap();
            nw_proxy_config_create_socksv5(nw_endpoint)
          }
        };

        let proxies: id = msg_send![class!(NSArray), arrayWithObject: proxy_config];
        let () = msg_send![data_store, setProxyConfigurations: proxies];
      }

      #[cfg(target_os = "macos")]
      (*webview).set_ivar(ACCEPT_FIRST_MOUSE, attributes.accept_first_mouse);

      let _: id = msg_send![_preference, setValue:_yes forKey:NSString::new("allowsPictureInPictureMediaPlayback")];

      if attributes.autoplay {
        let _: id = msg_send![config, setMediaTypesRequiringUserActionForPlayback:0];
      }

      #[cfg(target_os = "macos")]
      let _: id = msg_send![_preference, setValue:_yes forKey:NSString::new("tabFocusesLinks")];

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

      #[cfg(target_os = "macos")]
      {
        let (x, y) = attributes.bounds.map(|b| (b.x, b.y)).unwrap_or((0, 0));
        let (w, h) = if is_child {
          attributes.bounds.map(|b| (b.width, b.height))
        } else {
          None
        }
        .unwrap_or_else(|| {
          if is_child {
            let frame = NSView::frame(ns_view);
            (frame.size.width as u32, frame.size.height as u32)
          } else {
            (0, 0)
          }
        });

        let frame = CGRect {
          origin: window_position(if is_child { ns_view } else { webview }, x, y, h as f64),
          size: CGSize::new(w as f64, h as f64),
        };

        let _: () = msg_send![webview, initWithFrame:frame configuration:config];
        if is_child {
          // fixed element
          webview.setAutoresizingMask_(NSViewMinYMargin);
        } else {
          // Auto-resize
          webview.setAutoresizingMask_(NSViewHeightSizable | NSViewWidthSizable);
        }
      }

      #[cfg(target_os = "ios")]
      {
        let frame: CGRect = msg_send![ns_view, frame];
        // set all autoresizingmasks
        let () = msg_send![webview, setAutoresizingMask: 31];
        let _: () = msg_send![webview, initWithFrame:frame configuration:config];

        // disable scroll bounce by default
        let scroll: id = msg_send![webview, scrollView];
        let _: () = msg_send![scroll, setBounces: NO];
      }

      if !attributes.visible {
        let () = msg_send![webview, setHidden: YES];
      }

      #[cfg(any(debug_assertions, feature = "devtools"))]
      if attributes.devtools {
        let has_inspectable_property: BOOL =
          msg_send![webview, respondsToSelector: sel!(setInspectable:)];
        if has_inspectable_property == YES {
          let _: () = msg_send![webview, setInspectable: YES];
        }
        // this cannot be on an `else` statement, it does not work on macOS :(
        let dev = NSString::new("developerExtrasEnabled");
        let _: id = msg_send![_preference, setValue:_yes forKey:dev];
      }

      // allowsBackForwardNavigation
      #[cfg(target_os = "macos")]
      {
        let value = attributes.back_forward_navigation_gestures;
        let _: () = msg_send![webview, setAllowsBackForwardNavigationGestures: value];
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
        let ipc_handler_ptr = Box::into_raw(Box::new(ipc_handler));

        (*handler).set_ivar("function", ipc_handler_ptr as *mut _ as *mut c_void);
        let ipc = NSString::new(IPC_MESSAGE_HANDLER_NAME);
        let _: () = msg_send![manager, addScriptMessageHandler:handler name:ipc];
        ipc_handler_ptr
      } else {
        null_mut()
      };

      // Document title changed handler
      let document_title_changed_handler = if let Some(document_title_changed_handler) =
        attributes.document_title_changed_handler
      {
        let cls = ClassDecl::new("DocumentTitleChangedDelegate", class!(NSObject));
        let cls = match cls {
          Some(mut cls) => {
            cls.add_ivar::<*mut c_void>("function");
            cls.add_method(
              sel!(observeValueForKeyPath:ofObject:change:context:),
              observe_value_for_key_path as extern "C" fn(&Object, Sel, id, id, id, id),
            );
            extern "C" fn observe_value_for_key_path(
              this: &Object,
              _sel: Sel,
              key_path: id,
              of_object: id,
              _change: id,
              _context: id,
            ) {
              let key = NSString(key_path);
              if key.to_str() == "title" {
                unsafe {
                  let function = this.get_ivar::<*mut c_void>("function");
                  if !function.is_null() {
                    let function = &mut *(*function as *mut Box<dyn Fn(String)>);
                    let title: id = msg_send![of_object, title];
                    (function)(NSString(title).to_str().to_string());
                  }
                }
              }
            }
            cls.register()
          }
          None => class!(DocumentTitleChangedDelegate),
        };

        let handler: id = msg_send![cls, new];
        let document_title_changed_handler =
          Box::into_raw(Box::new(document_title_changed_handler));

        (*handler).set_ivar(
          "function",
          document_title_changed_handler as *mut _ as *mut c_void,
        );

        let _: () = msg_send![webview, addObserver:handler forKeyPath:NSString::new("title") options:0x01 context:nil ];

        document_title_changed_handler
      } else {
        null_mut()
      };

      // Navigation handler
      extern "C" fn navigation_policy(this: &Object, _: Sel, _: id, action: id, handler: id) {
        unsafe {
          // shouldPerformDownload is only available on macOS 11.3+
          let can_download: BOOL =
            msg_send![action, respondsToSelector: sel!(shouldPerformDownload)];
          let should_download: BOOL = if can_download == YES {
            msg_send![action, shouldPerformDownload]
          } else {
            NO
          };
          let request: id = msg_send![action, request];
          let url: id = msg_send![request, URL];
          let url: id = msg_send![url, absoluteString];
          let url = NSString(url);
          let target_frame: id = msg_send![action, targetFrame];
          let is_main_frame: bool = msg_send![target_frame, isMainFrame];

          let handler = handler as *mut block::Block<(NSInteger,), c_void>;

          if should_download == YES {
            let has_download_handler = this.get_ivar::<*mut c_void>("HasDownloadHandler");
            if !has_download_handler.is_null() {
              let has_download_handler = &mut *(*has_download_handler as *mut Box<bool>);
              if **has_download_handler {
                (*handler).call((2,));
              } else {
                (*handler).call((0,));
              }
            } else {
              (*handler).call((0,));
            }
          } else {
            let function = this.get_ivar::<*mut c_void>("navigation_policy_function");
            if !function.is_null() {
              let function = &mut *(*function as *mut Box<dyn for<'s> Fn(String, bool) -> bool>);
              match (function)(url.to_str().to_string(), is_main_frame) {
                true => (*handler).call((1,)),
                false => (*handler).call((0,)),
              };
            } else {
              (*handler).call((1,));
            }
          }
        }
      }

      // Navigation handler
      extern "C" fn navigation_policy_response(
        this: &Object,
        _: Sel,
        _: id,
        response: id,
        handler: id,
      ) {
        unsafe {
          let handler = handler as *mut block::Block<(NSInteger,), c_void>;
          let can_show_mime_type: bool = msg_send![response, canShowMIMEType];

          if !can_show_mime_type {
            let has_download_handler = this.get_ivar::<*mut c_void>("HasDownloadHandler");
            if !has_download_handler.is_null() {
              let has_download_handler = &mut *(*has_download_handler as *mut Box<bool>);
              if **has_download_handler {
                (*handler).call((2,));
                return;
              }
            }
          }

          (*handler).call((1,));
        }
      }

      let pending_scripts = Arc::new(Mutex::new(Some(Vec::new())));

      let navigation_delegate_cls = match ClassDecl::new("WryNavigationDelegate", class!(NSObject))
      {
        Some(mut cls) => {
          cls.add_ivar::<*mut c_void>("pending_scripts");
          cls.add_ivar::<*mut c_void>("HasDownloadHandler");
          cls.add_method(
            sel!(webView:decidePolicyForNavigationAction:decisionHandler:),
            navigation_policy as extern "C" fn(&Object, Sel, id, id, id),
          );
          cls.add_method(
            sel!(webView:decidePolicyForNavigationResponse:decisionHandler:),
            navigation_policy_response as extern "C" fn(&Object, Sel, id, id, id),
          );
          add_download_methods(&mut cls);
          add_navigation_mathods(&mut cls);
          cls.register()
        }
        None => class!(WryNavigationDelegate),
      };

      let navigation_policy_handler: id = msg_send![navigation_delegate_cls, new];

      (*navigation_policy_handler).set_ivar(
        "pending_scripts",
        Box::into_raw(Box::new(pending_scripts.clone())) as *mut c_void,
      );

      let (navigation_decide_policy_ptr, download_delegate) = if attributes
        .navigation_handler
        .is_some()
        || attributes.new_window_req_handler.is_some()
        || attributes.download_started_handler.is_some()
      {
        let function_ptr = {
          let navigation_handler = attributes.navigation_handler;
          let new_window_req_handler = attributes.new_window_req_handler;
          Box::into_raw(Box::new(
            Box::new(move |url: String, is_main_frame: bool| -> bool {
              if is_main_frame {
                navigation_handler
                  .as_ref()
                  .map_or(true, |navigation_handler| (navigation_handler)(url))
              } else {
                new_window_req_handler
                  .as_ref()
                  .map_or(true, |new_window_req_handler| (new_window_req_handler)(url))
              }
            }) as Box<dyn Fn(String, bool) -> bool>,
          ))
        };
        (*navigation_policy_handler).set_ivar(
          "navigation_policy_function",
          function_ptr as *mut _ as *mut c_void,
        );

        let has_download_handler = Box::into_raw(Box::new(Box::new(
          attributes.download_started_handler.is_some(),
        )));
        (*navigation_policy_handler).set_ivar(
          "HasDownloadHandler",
          has_download_handler as *mut _ as *mut c_void,
        );

        // Download handler
        let download_delegate = if attributes.download_started_handler.is_some()
          || attributes.download_completed_handler.is_some()
        {
          let cls = match ClassDecl::new("WryDownloadDelegate", class!(NSObject)) {
            Some(mut cls) => {
              cls.add_ivar::<*mut c_void>("started");
              cls.add_ivar::<*mut c_void>("completed");
              cls.add_method(
                sel!(download:decideDestinationUsingResponse:suggestedFilename:completionHandler:),
                download_policy as extern "C" fn(&Object, Sel, id, id, id, id),
              );
              cls.add_method(
                sel!(downloadDidFinish:),
                download_did_finish as extern "C" fn(&Object, Sel, id),
              );
              cls.add_method(
                sel!(download:didFailWithError:resumeData:),
                download_did_fail as extern "C" fn(&Object, Sel, id, id, id),
              );
              cls.register()
            }
            None => class!(WryDownloadDelegate),
          };

          let download_delegate: id = msg_send![cls, new];
          if let Some(download_started_handler) = attributes.download_started_handler {
            let download_started_ptr = Box::into_raw(Box::new(download_started_handler));
            (*download_delegate).set_ivar("started", download_started_ptr as *mut _ as *mut c_void);
          }
          if let Some(download_completed_handler) = attributes.download_completed_handler {
            let download_completed_ptr = Box::into_raw(Box::new(download_completed_handler));
            (*download_delegate)
              .set_ivar("completed", download_completed_ptr as *mut _ as *mut c_void);
          }

          set_download_delegate(navigation_policy_handler, download_delegate);

          navigation_policy_handler
        } else {
          null_mut()
        };

        (function_ptr, download_delegate)
      } else {
        (null_mut(), null_mut())
      };

      let page_load_handler = set_navigation_methods(
        navigation_policy_handler,
        webview,
        attributes.on_page_load_handler,
      );

      let _: () = msg_send![webview, setNavigationDelegate: navigation_policy_handler];

      // File upload panel handler
      extern "C" fn run_file_upload_panel(
        _this: &Object,
        _: Sel,
        _webview: id,
        open_panel_params: id,
        _frame: id,
        handler: id,
      ) {
        unsafe {
          let handler = handler as *mut block::Block<(id,), c_void>;
          let cls = class!(NSOpenPanel);
          let open_panel: id = msg_send![cls, openPanel];
          let _: () = msg_send![open_panel, setCanChooseFiles: YES];
          let allow_multi: BOOL = msg_send![open_panel_params, allowsMultipleSelection];
          let _: () = msg_send![open_panel, setAllowsMultipleSelection: allow_multi];
          let allow_dir: BOOL = msg_send![open_panel_params, allowsDirectories];
          let _: () = msg_send![open_panel, setCanChooseDirectories: allow_dir];
          let ok: NSInteger = msg_send![open_panel, runModal];
          if ok == 1 {
            let url: id = msg_send![open_panel, URLs];
            (*handler).call((url,));
          } else {
            (*handler).call((nil,));
          }
        }
      }

      extern "C" fn request_media_capture_permission(
        _this: &Object,
        _: Sel,
        _webview: id,
        _origin: id,
        _frame: id,
        _type: id,
        decision_handler: id,
      ) {
        unsafe {
          let decision_handler = decision_handler as *mut block::Block<(NSInteger,), c_void>;
          //https://developer.apple.com/documentation/webkit/wkpermissiondecision?language=objc
          (*decision_handler).call((1,));
        }
      }

      let ui_delegate = match ClassDecl::new("WebViewUIDelegate", class!(NSObject)) {
        Some(mut ctl) => {
          ctl.add_method(
            sel!(webView:runOpenPanelWithParameters:initiatedByFrame:completionHandler:),
            run_file_upload_panel as extern "C" fn(&Object, Sel, id, id, id, id),
          );

          ctl.add_method(
            sel!(webView:requestMediaCapturePermissionForOrigin:initiatedByFrame:type:decisionHandler:),
            request_media_capture_permission as extern "C" fn(&Object, Sel, id, id, id, id, id),
          );

          ctl.register()
        }
        None => class!(WebViewUIDelegate),
      };
      let ui_delegate: id = msg_send![ui_delegate, new];
      let _: () = msg_send![webview, setUIDelegate: ui_delegate];

      // File drop handling
      #[cfg(target_os = "macos")]
      let drag_drop_ptr = match attributes.drag_drop_handler {
        // if we have a drag_drop_handler defined, use the defined handler
        Some(drag_drop_handler) => set_drag_drop_handler(webview, drag_drop_handler),
        // prevent panic by using a blank handler
        None => set_drag_drop_handler(webview, Box::new(|_| false)),
      };

      // ns window is required for the print operation
      #[cfg(target_os = "macos")]
      {
        let ns_window: id = msg_send![ns_view, window];

        let can_set_titlebar_style: BOOL = msg_send![
          ns_window,
          respondsToSelector: sel!(setTitlebarSeparatorStyle:)
        ];
        if can_set_titlebar_style == YES {
          // `1` means `none`, see https://developer.apple.com/documentation/appkit/nstitlebarseparatorstyle/none
          let () = msg_send![ns_window, setTitlebarSeparatorStyle: 1];
        }
      }

      let w = Self {
        webview,
        manager,
        pending_scripts,
        ipc_handler_ptr,
        document_title_changed_handler,
        navigation_decide_policy_ptr,
        #[cfg(target_os = "macos")]
        drag_drop_ptr,
        page_load_handler,
        download_delegate,
        protocol_ptrs,
        is_child,
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
        w.navigate_to_url(url.as_str(), attributes.headers)?;
      } else if let Some(html) = attributes.html {
        w.navigate_to_string(&html);
      }

      // Inject the web view into the window as main content
      #[cfg(target_os = "macos")]
      {
        if is_child {
          let _: () = msg_send![ns_view, addSubview: webview];
        } else {
          let parent_view_cls = match ClassDecl::new("WryWebViewParent", class!(NSView)) {
            Some(mut decl) => {
              decl.add_method(
                sel!(keyDown:),
                key_down as extern "C" fn(&mut Object, Sel, id),
              );

              extern "C" fn key_down(_this: &mut Object, _sel: Sel, event: id) {
                unsafe {
                  let app = cocoa::appkit::NSApp();
                  let menu: id = msg_send![app, mainMenu];
                  let () = msg_send![menu, performKeyEquivalent: event];
                }
              }

              decl.register()
            }
            None => class!(NSView),
          };

          let parent_view: id = msg_send![parent_view_cls, alloc];
          let _: () = msg_send![parent_view, init];
          parent_view.setAutoresizingMask_(NSViewHeightSizable | NSViewWidthSizable);
          let _: () = msg_send![parent_view, addSubview: webview];

          // inject the webview into the window
          let ns_window: id = msg_send![ns_view, window];
          // Tell the webview receive keyboard events in the window.
          // See https://github.com/tauri-apps/wry/issues/739
          let _: () = msg_send![ns_window, setContentView: parent_view];
          let _: () = msg_send![ns_window, makeFirstResponder: webview];
        }

        // make sure the window is always on top when we create a new webview
        let app_class = class!(NSApplication);
        let app: id = msg_send![app_class, sharedApplication];
        let _: () = msg_send![app, activateIgnoringOtherApps: YES];
      }

      #[cfg(target_os = "ios")]
      {
        let _: () = msg_send![ns_view, addSubview: webview];
      }

      Ok(w)
    }
  }

  pub fn url(&self) -> crate::Result<String> {
    url_from_webview(self.webview)
  }

  pub fn eval(&self, js: &str, callback: Option<impl Fn(String) + Send + 'static>) -> Result<()> {
    if let Some(scripts) = &mut *self.pending_scripts.lock().unwrap() {
      scripts.push(js.into());
    } else {
      // Safety: objc runtime calls are unsafe
      unsafe {
        #[cfg(feature = "tracing")]
        let span = Mutex::new(Some(tracing::debug_span!("wry::eval").entered()));

        // we need to check if the callback exists outside the handler otherwise it's a segfault
        if let Some(callback) = callback {
          let handler = block::ConcreteBlock::new(move |val: id, _err: id| {
            #[cfg(feature = "tracing")]
            span.lock().unwrap().take();

            let mut result = String::new();

            if val != nil {
              let serializer = class!(NSJSONSerialization);
              let json_ns_data: NSData = msg_send![serializer, dataWithJSONObject:val options:NS_JSON_WRITING_FRAGMENTS_ALLOWED error:nil];
              let json_string = NSString::from(json_ns_data);

              result = json_string.to_str().to_string();
            }

            callback(result);
          }).copy();

          let _: () =
            msg_send![self.webview, evaluateJavaScript:NSString::new(js) completionHandler:handler];
        } else {
          #[cfg(feature = "tracing")]
          let handler = block::ConcreteBlock::new(move |_val: id, _err: id| {
            span.lock().unwrap().take();
          })
          .copy();
          #[cfg(not(feature = "tracing"))]
          let handler = null::<*const c_void>();

          let _: () =
            msg_send![self.webview, evaluateJavaScript:NSString::new(js) completionHandler:handler];
        }
      }
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

  pub fn load_url(&self, url: &str) -> crate::Result<()> {
    self.navigate_to_url(url, None)
  }

  pub fn load_url_with_headers(&self, url: &str, headers: http::HeaderMap) -> crate::Result<()> {
    self.navigate_to_url(url, Some(headers))
  }

  pub fn clear_all_browsing_data(&self) -> Result<()> {
    unsafe {
      let config: id = msg_send![self.webview, configuration];
      let store: id = msg_send![config, websiteDataStore];
      let all_data_types: id = msg_send![class!(WKWebsiteDataStore), allWebsiteDataTypes];
      let date: id = msg_send![class!(NSDate), dateWithTimeIntervalSince1970: 0.0];
      let handler = null::<*const c_void>();
      let _: () = msg_send![store, removeDataOfTypes:all_data_types modifiedSince:date completionHandler:handler];
    }
    Ok(())
  }

  fn navigate_to_url(&self, url: &str, headers: Option<http::HeaderMap>) -> crate::Result<()> {
    // Safety: objc runtime calls are unsafe
    unsafe {
      let url: id = msg_send![class!(NSURL), URLWithString: NSString::new(url)];
      let request: id = msg_send![class!(NSMutableURLRequest), requestWithURL: url];
      if let Some(headers) = headers {
        for (name, value) in headers.iter() {
          let key = NSString::new(name.as_str());
          let value = NSString::new(value.to_str().unwrap_or_default());
          let _: () = msg_send![request, addValue:value.as_ptr() forHTTPHeaderField:key.as_ptr()];
        }
      }
      let () = msg_send![self.webview, loadRequest: request];
    }

    Ok(())
  }

  fn navigate_to_string(&self, html: &str) {
    // Safety: objc runtime calls are unsafe
    unsafe {
      let () = msg_send![self.webview, loadHTMLString:NSString::new(html) baseURL:nil];
    }
  }

  fn set_user_agent(&self, user_agent: &str) {
    unsafe {
      let () = msg_send![self.webview, setCustomUserAgent: NSString::new(user_agent)];
    }
  }

  pub fn print(&self) -> crate::Result<()> {
    // Safety: objc runtime calls are unsafe
    #[cfg(target_os = "macos")]
    unsafe {
      let can_print: BOOL = msg_send![
        self.webview,
        respondsToSelector: sel!(printOperationWithPrintInfo:)
      ];
      if can_print == YES {
        // Create a shared print info
        let print_info: id = msg_send![class!(NSPrintInfo), sharedPrintInfo];
        let print_info: id = msg_send![print_info, init];
        // Create new print operation from the webview content
        let print_operation: id = msg_send![self.webview, printOperationWithPrintInfo: print_info];
        // Allow the modal to detach from the current thread and be non-blocker
        let () = msg_send![print_operation, setCanSpawnSeparateThread: YES];
        // Launch the modal
        let window: id = msg_send![self.webview, window];
        let () = msg_send![print_operation, runOperationModalForWindow: window delegate: null::<*const c_void>() didRunSelector: null::<*const c_void>() contextInfo: null::<*const c_void>()];
      }
    }

    Ok(())
  }

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn open_devtools(&self) {
    #[cfg(target_os = "macos")]
    unsafe {
      // taken from <https://github.com/WebKit/WebKit/blob/784f93cb80a386c29186c510bba910b67ce3adc1/Source/WebKit/UIProcess/API/Cocoa/WKWebView.mm#L1939>
      let tool: id = msg_send![self.webview, _inspector];
      let _: id = msg_send![tool, show];
    }
  }

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn close_devtools(&self) {
    #[cfg(target_os = "macos")]
    unsafe {
      // taken from <https://github.com/WebKit/WebKit/blob/784f93cb80a386c29186c510bba910b67ce3adc1/Source/WebKit/UIProcess/API/Cocoa/WKWebView.mm#L1939>
      let tool: id = msg_send![self.webview, _inspector];
      let _: id = msg_send![tool, close];
    }
  }

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn is_devtools_open(&self) -> bool {
    #[cfg(target_os = "macos")]
    unsafe {
      // taken from <https://github.com/WebKit/WebKit/blob/784f93cb80a386c29186c510bba910b67ce3adc1/Source/WebKit/UIProcess/API/Cocoa/WKWebView.mm#L1939>
      let tool: id = msg_send![self.webview, _inspector];
      let is_visible: objc::runtime::BOOL = msg_send![tool, isVisible];
      is_visible == objc::runtime::YES
    }
    #[cfg(not(target_os = "macos"))]
    false
  }

  pub fn zoom(&self, scale_factor: f64) -> crate::Result<()> {
    unsafe {
      let _: () = msg_send![self.webview, setPageZoom: scale_factor];
    }

    Ok(())
  }

  pub fn set_background_color(&self, _background_color: RGBA) -> Result<()> {
    Ok(())
  }

  pub fn bounds(&self) -> crate::Result<Rect> {
    unsafe {
      let parent: id = msg_send![self.webview, superview];
      let parent_frame: CGRect = msg_send![parent, frame];
      let webview_frame: CGRect = msg_send![self.webview, frame];

      Ok(Rect {
        x: webview_frame.origin.x as i32,
        y: (parent_frame.size.height - webview_frame.origin.y - webview_frame.size.height) as i32,
        width: webview_frame.size.width as u32,
        height: webview_frame.size.height as u32,
      })
    }
  }

  pub fn set_bounds(&self, bounds: Rect) -> crate::Result<()> {
    if self.is_child {
      unsafe {
        let frame = CGRect {
          origin: window_position(
            msg_send![self.webview, superview],
            bounds.x,
            bounds.y,
            bounds.height as f64,
          ),
          size: CGSize::new(bounds.width as f64, bounds.height as f64),
        };
        let () = msg_send![self.webview, setFrame: frame];
      }
    }

    Ok(())
  }

  pub fn set_visible(&self, visible: bool) -> Result<()> {
    unsafe {
      let () = msg_send![self.webview, setHidden: !visible];
    }

    Ok(())
  }

  pub fn focus(&self) -> Result<()> {
    unsafe {
      let window: id = msg_send![self.webview, window];
      let _: () = msg_send![window, makeFirstResponder: self.webview];
    }

    Ok(())
  }

  #[cfg(target_os = "macos")]
  pub(crate) fn reparent(&self, window: id) -> crate::Result<()> {
    unsafe {
      let content_view: id = msg_send![window, contentView];
      let _: () = msg_send![content_view, addSubview: self.webview];
    }

    Ok(())
  }
}

pub fn url_from_webview(webview: id) -> Result<String> {
  let url_obj: *mut Object = unsafe { msg_send![webview, URL] };
  let absolute_url: *mut Object = unsafe { msg_send![url_obj, absoluteString] };

  let bytes = {
    let bytes: *const c_char = unsafe { msg_send![absolute_url, UTF8String] };
    bytes as *const u8
  };

  // 4 represents utf8 encoding
  let len = unsafe { msg_send![absolute_url, lengthOfBytesUsingEncoding: 4] };
  let bytes = unsafe { std::slice::from_raw_parts(bytes, len) };

  std::str::from_utf8(bytes)
    .map(Into::into)
    .map_err(Into::into)
}

pub fn platform_webview_version() -> Result<String> {
  unsafe {
    let bundle: id =
      msg_send![class!(NSBundle), bundleWithIdentifier: NSString::new("com.apple.WebKit")];
    let dict: id = msg_send![bundle, infoDictionary];
    let webkit_version: id = msg_send![dict, objectForKey: NSString::new("CFBundleVersion")];

    let nsstring = NSString(webkit_version);

    let () = msg_send![bundle, unload];
    Ok(nsstring.to_str().to_string())
  }
}

impl Drop for InnerWebView {
  fn drop(&mut self) {
    // We need to drop handler closures here
    unsafe {
      if !self.ipc_handler_ptr.is_null() {
        drop(Box::from_raw(self.ipc_handler_ptr));

        let ipc = NSString::new(IPC_MESSAGE_HANDLER_NAME);
        let _: () = msg_send![self.manager, removeScriptMessageHandlerForName: ipc];
      }

      if !self.document_title_changed_handler.is_null() {
        drop(Box::from_raw(self.document_title_changed_handler));
      }

      if !self.navigation_decide_policy_ptr.is_null() {
        drop(Box::from_raw(self.navigation_decide_policy_ptr));
      }

      drop_navigation_methods(self);

      #[cfg(target_os = "macos")]
      if !self.drag_drop_ptr.is_null() {
        drop(Box::from_raw(self.drag_drop_ptr));
      }

      if !self.download_delegate.is_null() {
        self.download_delegate.drop_in_place();
      }

      for ptr in self.protocol_ptrs.iter() {
        if !ptr.is_null() {
          drop(Box::from_raw(*ptr));
        }
      }

      // Remove webview from window's NSView before dropping.
      let () = msg_send![self.webview, removeFromSuperview];
      let _: Id<_> = Id::from_retained_ptr(self.webview);
      let _: Id<_> = Id::from_retained_ptr(self.manager);
    }
  }
}

const UTF8_ENCODING: usize = 4;

struct NSString(id);

impl NSString {
  fn new(s: &str) -> Self {
    // Safety: objc runtime calls are unsafe
    NSString(unsafe {
      let ns_string: id = msg_send![class!(NSString), alloc];
      let ns_string: id = msg_send![ns_string,
                            initWithBytes:s.as_ptr()
                            length:s.len()
                            encoding:UTF8_ENCODING];

      // The thing is allocated in rust, the thing must be set to autorelease in rust to relinquish control
      // or it can not be released correctly in OC runtime
      let _: () = msg_send![ns_string, autorelease];

      ns_string
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

  #[allow(dead_code)] // only used when `mac-proxy` feature is enabled
  fn to_cstr(&self) -> *const c_char {
    unsafe {
      let utf_8_string = msg_send![self.0, UTF8String];
      utf_8_string
    }
  }

  fn as_ptr(&self) -> id {
    self.0
  }
}

impl From<NSData> for NSString {
  fn from(value: NSData) -> Self {
    Self(unsafe {
      let ns_string: id = msg_send![class!(NSString), alloc];
      let ns_string: id = msg_send![ns_string, initWithData:value encoding:UTF8_ENCODING];
      let _: () = msg_send![ns_string, autorelease];

      ns_string
    })
  }
}

struct NSData(id);

/// Converts from wry screen-coordinates to macOS screen-coordinates.
/// wry: top-left is (0, 0) and y increasing downwards
/// macOS: bottom-left is (0, 0) and y increasing upwards
unsafe fn window_position(view: id, x: i32, y: i32, height: f64) -> CGPoint {
  let frame: CGRect = msg_send![view, frame];
  CGPoint::new(x as f64, frame.size.height - y as f64 - height)
}
