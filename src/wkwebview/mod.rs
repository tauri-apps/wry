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

use block2::Block;

use dpi::{LogicalPosition, LogicalSize};
use objc2::{
  class,
  declare::ClassBuilder,
  declare_class,
  ffi::YES,
  mutability::MainThreadOnly,
  rc::{Allocated, Retained},
  runtime::{AnyClass, AnyObject, Bool, NSObject, ProtocolObject},
  ClassType, DeclaredClass,
};
use objc2_app_kit::{
  NSApp, NSApplication, NSAutoresizingMaskOptions, NSDragOperation, NSDraggingInfo, NSEvent,
  NSModalResponse, NSModalResponseOK, NSPrintInfo, NSTitlebarSeparatorStyle, NSView,
};
use objc2_foundation::{
  ns_string, CGPoint, CGRect, CGSize, MainThreadMarker, NSArray, NSBundle, NSDate, NSError,
  NSHTTPURLResponse, NSJSONSerialization, NSKeyValueObservingOptions, NSMutableDictionary,
  NSMutableURLRequest, NSNumber, NSObjectNSKeyValueCoding, NSObjectNSKeyValueObserverRegistration,
  NSObjectProtocol, NSString, NSURL,
};
use objc2_web_kit::{
  WKAudiovisualMediaTypes, WKFrameInfo, WKMediaCaptureType, WKNavigationAction,
  WKNavigationActionPolicy, WKNavigationDelegate, WKNavigationResponse, WKNavigationResponsePolicy,
  WKOpenPanelParameters, WKPermissionDecision, WKScriptMessage, WKScriptMessageHandler,
  WKSecurityOrigin, WKUIDelegate, WKURLSchemeTask, WKUserContentController, WKUserScript,
  WKUserScriptInjectionTime, WKWebView, WKWebViewConfiguration, WKWebpagePreferences,
  WKWebsiteDataStore,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use std::{
  borrow::Cow,
  cell::RefCell,
  ffi::{c_void, CStr},
  os::raw::c_char,
  ptr::{null_mut, NonNull},
  rc::Rc,
  slice, str,
  sync::{Arc, Mutex},
};

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
  DragDropEvent, Error, PageLoadEvent, Rect, RequestAsyncResponder, Result, WebContext,
  WebViewAttributes, RGBA,
};

use http::{
  header::{CONTENT_LENGTH, CONTENT_TYPE},
  status::StatusCode,
  version::Version,
  Request, Response as HttpResponse,
};

const IPC_MESSAGE_HANDLER_NAME: &str = "ipc";

pub(crate) struct InnerWebView {
  pub webview: Retained<WryWebView>,
  pub manager: Retained<WKUserContentController>,
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
  download_delegate: *mut AnyObject,
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

    unsafe {
      Self::new_ns_view(
        &*(ns_view as *mut NSView),
        attributes,
        pl_attrs,
        _web_context,
        false,
      )
    }
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

    unsafe {
      Self::new_ns_view(
        &*(ns_view as *mut NSView),
        attributes,
        pl_attrs,
        _web_context,
        true,
      )
    }
  }

  fn new_ns_view(
    ns_view: &NSView,
    attributes: WebViewAttributes,
    _pl_attrs: super::PlatformSpecificWebViewAttributes,
    _web_context: Option<&mut WebContext>,
    is_child: bool,
  ) -> Result<Self> {
    let mtm = MainThreadMarker::new().ok_or(Error::NotMainThread)?;

    // Function for ipc handler
    extern "C" fn did_receive(
      this: &AnyObject,
      _: objc2::runtime::Sel,
      _: &AnyObject,
      msg: &WKScriptMessage,
    ) {
      // Safety: objc runtime calls are unsafe
      unsafe {
        #[cfg(feature = "tracing")]
        let _span = tracing::info_span!("wry::ipc::handle").entered();

        let function = this.get_ivar::<*mut c_void>("function");
        if !function.is_null() {
          let function = &mut *(*function as *mut Box<dyn Fn(Request<String>)>);
          let body = msg.body();
          let is_string = Retained::cast::<NSObject>(body.clone()).isKindOfClass(NSString::class());
          if is_string {
            let body = Retained::cast::<NSString>(body);
            let js_utf8 = body.UTF8String();

            let frame_info = msg.frameInfo();
            let request = frame_info.request();
            let url = request.URL().unwrap();
            let absolute_url = url.absoluteString().unwrap();
            let url_utf8 = absolute_url.UTF8String();

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
    extern "C" fn start_task(
      this: &AnyObject,
      _: objc2::runtime::Sel,
      _webview: &WKWebView,
      task: *mut ProtocolObject<dyn WKURLSchemeTask>, // FIXME: not sure if this work.
    ) {
      unsafe {
        #[cfg(feature = "tracing")]
        let span = tracing::info_span!("wry::custom_protocol::handle", uri = tracing::field::Empty)
          .entered();
        let function = this.get_ivar::<*mut c_void>("function");
        if !function.is_null() {
          let function =
            &mut *(*function as *mut Box<dyn Fn(Request<Vec<u8>>, RequestAsyncResponder)>);

          // Get url request
          let request = (*task).request();
          let url = request.URL().unwrap();

          let uri = url.absoluteString().unwrap().to_string();

          #[cfg(feature = "tracing")]
          span.record("uri", uri);

          // Get request method (GET, POST, PUT etc...)
          let method = request.HTTPMethod().unwrap().to_string();

          // Prepare our HttpRequest
          let mut http_request = Request::builder().uri(uri).method(method.as_str());

          // Get body
          let mut sent_form_body = Vec::new();
          let body = request.HTTPBody();
          let body_stream = request.HTTPBodyStream();
          if let Some(body) = body {
            let length = body.length();
            let data_bytes = body.bytes();
            sent_form_body = slice::from_raw_parts(data_bytes.as_ptr(), length).to_vec();
          } else if let Some(body_stream) = body_stream {
            body_stream.open();

            while body_stream.hasBytesAvailable() {
              sent_form_body.reserve(128);
              let p = sent_form_body.as_mut_ptr().add(sent_form_body.len());
              let read_length = sent_form_body.capacity() - sent_form_body.len();
              let count = body_stream.read_maxLength(NonNull::new(p).unwrap(), read_length);
              sent_form_body.set_len(sent_form_body.len() + count as usize);
            }

            body_stream.close();
          }

          // Extract all headers fields
          let all_headers = request.allHTTPHeaderFields();

          // get all our headers values and inject them in our request
          if let Some(all_headers) = all_headers {
            for current_header in all_headers.allKeys().to_vec() {
              let header_value = all_headers.valueForKey(current_header).unwrap();

              // inject the header into the request
              http_request =
                http_request.header(current_header.to_string(), header_value.to_string());
            }
          }

          let respond_with_404 = || {
            let urlresponse = NSHTTPURLResponse::alloc();
            let response = NSHTTPURLResponse::initWithURL_statusCode_HTTPVersion_headerFields(
              urlresponse,
              &url,
              StatusCode::NOT_FOUND.as_u16().try_into().unwrap(),
              Some(&NSString::from_str(
                format!("{:#?}", Version::HTTP_11).as_str(),
              )),
              None,
            )
            .unwrap();
            (*task).didReceiveResponse(&response);
            // Finish
            (*task).didFinish();
          };

          // send response
          match http_request.body(sent_form_body) {
            Ok(final_request) => {
              let responder: Box<dyn FnOnce(HttpResponse<Cow<'static, [u8]>>)> =
                Box::new(move |sent_response| {
                  let content = sent_response.body();
                  // default: application/octet-stream, but should be provided by the client
                  let wanted_mime = sent_response.headers().get(CONTENT_TYPE);
                  // default to 200
                  let wanted_status_code = sent_response.status().as_u16() as i32;
                  // default to HTTP/1.1
                  let wanted_version = format!("{:#?}", sent_response.version());

                  let mut headers = NSMutableDictionary::new();

                  if let Some(mime) = wanted_mime {
                    headers.insert_id(
                      NSString::from_str(mime.to_str().unwrap()).as_ref(),
                      NSString::from_str(CONTENT_TYPE.as_str()),
                    );
                  }
                  headers.insert_id(
                    NSString::from_str(&content.len().to_string()).as_ref(),
                    NSString::from_str(CONTENT_LENGTH.as_str()),
                  );

                  // add headers
                  for (name, value) in sent_response.headers().iter() {
                    if let Ok(value) = value.to_str() {
                      headers.insert_id(
                        NSString::from_str(name.as_str()).as_ref(),
                        NSString::from_str(value),
                      );
                    }
                  }

                  let urlresponse = NSHTTPURLResponse::alloc();
                  let response =
                    NSHTTPURLResponse::initWithURL_statusCode_HTTPVersion_headerFields(
                      urlresponse,
                      &url,
                      wanted_status_code.try_into().unwrap(),
                      Some(&NSString::from_str(&wanted_version)),
                      Some(&headers),
                    )
                    .unwrap();
                  (*task).didReceiveResponse(&response);

                  // Send data
                  let bytes = content.as_ptr() as *mut c_void;
                  let data = objc2_foundation::NSData::alloc();
                  // MIGRATE NOTE: we copied the content to the NSData because content will be freed
                  // when out of scope but NSData will also free the content when it's done and cause doube free.
                  let data =
                    objc2_foundation::NSData::initWithBytes_length(data, bytes, content.len());
                  (*task).didReceiveData(&data);
                  // Finish
                  (*task).didFinish();
                });

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
    extern "C" fn stop_task(
      _: &AnyObject,
      _: objc2::runtime::Sel,
      _webview: &WKWebView,
      _task: &ProtocolObject<dyn WKURLSchemeTask>,
    ) {
    }

    // Safety: objc runtime calls are unsafe
    unsafe {
      // Config and custom protocol
      let config = WKWebViewConfiguration::new();
      let mut protocol_ptrs = Vec::new();

      // Incognito mode
      let data_store = if attributes.incognito {
        WKWebsiteDataStore::nonPersistentDataStore()
      } else {
        WKWebsiteDataStore::defaultDataStore()
      };

      for (name, function) in attributes.custom_protocols {
        let scheme_name = format!("{}URLSchemeHandler", name);
        let cls = ClassBuilder::new(&scheme_name, NSObject::class());
        let cls = match cls {
          Some(mut cls) => {
            cls.add_ivar::<*mut c_void>("function");
            cls.add_method(
              objc2::sel!(webView:startURLSchemeTask:),
              start_task as extern "C" fn(_, _, _, _),
            );
            cls.add_method(
              objc2::sel!(webView:stopURLSchemeTask:),
              stop_task as extern "C" fn(_, _, _, _),
            );
            cls.register()
          }
          None => AnyClass::get(&scheme_name).expect("Failed to get the class definition"),
        };
        let handler: *mut AnyObject = objc2::msg_send![cls, new];
        let function = Box::into_raw(Box::new(function));
        protocol_ptrs.push(function);

        let ivar = (*handler).class().instance_variable("function").unwrap();
        let ivar_delegate = ivar.load_mut(&mut *handler);
        *ivar_delegate = function as *mut _ as *mut c_void;

        config.setURLSchemeHandler_forURLScheme(
          Some(&*(handler.cast::<ProtocolObject<dyn objc2_web_kit::WKURLSchemeHandler>>())),
          &NSString::from_str(&name),
        );
      }

      // WebView and manager
      let manager = config.userContentController();
      let webview = mtm.alloc::<WryWebView>().set_ivars(WryWebViewIvars {
        #[cfg(target_os = "macos")]
        drag_drop_handler: match attributes.drag_drop_handler {
          Some(handler) => RefCell::new(handler),
          None => RefCell::new(Box::new(|_| false)),
        },
        #[cfg(target_os = "macos")]
        accept_first_mouse: Bool::new(attributes.accept_first_mouse),
      });

      config.setWebsiteDataStore(&data_store);
      let _preference = config.preferences();
      let _yes = NSNumber::numberWithBool(YES);

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

      _preference.as_super().setValue_forKey(
        Some(&_yes),
        ns_string!("allowsPictureInPictureMediaPlayback"),
      );

      if attributes.autoplay {
        config.setMediaTypesRequiringUserActionForPlayback(
          WKAudiovisualMediaTypes::WKAudiovisualMediaTypeNone,
        );
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

      #[cfg(target_os = "macos")]
      let webview = {
        let window = ns_view.window().unwrap();
        let scale_factor = window.backingScaleFactor();
        let (x, y) = attributes
          .bounds
          .map(|b| b.position.to_logical::<f64>(scale_factor))
          .map(Into::into)
          .unwrap_or((0, 0));
        let (w, h) = if is_child {
          attributes
            .bounds
            .map(|b| b.size.to_logical::<u32>(scale_factor))
            .map(Into::into)
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
          origin: if is_child {
            window_position(ns_view, x, y, h as f64)
          } else {
            CGPoint::new(x as f64, (0 - y - h as i32) as f64)
          },
          size: CGSize::new(w as f64, h as f64),
        };
        let webview: Retained<WryWebView> =
          objc2::msg_send_id![super(webview), initWithFrame:frame configuration:&**config];
        webview
      };
      #[cfg(target_os = "ios")]
      let webview = {
        let frame = ns_view.frame();
        let webview = WKWebView::initWithFrame_configuration(webview, frame, &config);
        webview
      };

      #[cfg(target_os = "macos")]
      {
        if is_child {
          // fixed element
          webview.setAutoresizingMask(NSAutoresizingMaskOptions::NSViewMinYMargin);
        } else {
          // Auto-resize
          let options = NSAutoresizingMaskOptions(
            NSAutoresizingMaskOptions::NSViewHeightSizable.0
              | NSAutoresizingMaskOptions::NSViewWidthSizable.0,
          );
          webview.setAutoresizingMask(options);
        }

        // allowsBackForwardNavigation
        let value = attributes.back_forward_navigation_gestures;
        webview.setAllowsBackForwardNavigationGestures(value);

        // tabFocusesLinks
        _preference
          .as_super()
          .setValue_forKey(Some(&_yes), ns_string!("tabFocusesLinks"));
      }
      #[cfg(target_os = "ios")]
      {
        // set all autoresizingmasks
        webview.setAutoresizingMask(NSAutoresizingMaskOptions::from_bits(31).unwrap());
        // let () = msg_send![webview, setAutoresizingMask: 31];

        // disable scroll bounce by default
        let scroll: id = msg_send![webview, scrollView];
        let _: () = msg_send![scroll, setBounces: NO];
      }

      if !attributes.visible {
        webview.setHidden(true);
      }

      #[cfg(any(debug_assertions, feature = "devtools"))]
      if attributes.devtools {
        let has_inspectable_property: bool =
          NSObject::respondsToSelector(&webview, objc2::sel!(setInspectable:));
        if has_inspectable_property == true {
          webview.setInspectable(true);
        }
        // this cannot be on an `else` statement, it does not work on macOS :(
        let dev = NSString::from_str("developerExtrasEnabled");
        _preference.setValue_forKey(Some(&_yes), &dev);
      }

      // Message handler
      let ipc_handler_ptr = if let Some(ipc_handler) = attributes.ipc_handler {
        let cls = ClassBuilder::new("WebViewDelegate", NSObject::class());
        let cls = match cls {
          Some(mut cls) => {
            cls.add_ivar::<*mut c_void>("function");
            cls.add_method(
              objc2::sel!(userContentController:didReceiveScriptMessage:),
              did_receive as extern "C" fn(_, _, _, _),
            );
            cls.register()
          }
          None => class!(WebViewDelegate),
        };
        let handler: *mut AnyObject = objc2::msg_send![cls, new];
        let ipc_handler_ptr = Box::into_raw(Box::new(ipc_handler));

        let ivar = (*handler).class().instance_variable("function").unwrap();
        let ivar_delegate = ivar.load_mut(&mut *handler);
        *ivar_delegate = ipc_handler_ptr as *mut _ as *mut c_void;

        let ipc = NSString::from_str(IPC_MESSAGE_HANDLER_NAME);
        manager.addScriptMessageHandler_name(
          &*(handler.cast::<ProtocolObject<dyn WKScriptMessageHandler>>()),
          &ipc,
        );
        ipc_handler_ptr
      } else {
        null_mut()
      };

      // Document title changed handler
      let document_title_changed_handler =
        if let Some(document_title_changed_handler) = attributes.document_title_changed_handler {
          let cls = ClassBuilder::new("DocumentTitleChangedDelegate", NSObject::class());
          let cls = match cls {
            Some(mut cls) => {
              cls.add_ivar::<*mut c_void>("function");
              cls.add_method(
                objc2::sel!(observeValueForKeyPath:ofObject:change:context:),
                observe_value_for_key_path as extern "C" fn(_, _, _, _, _, _),
              );
              extern "C" fn observe_value_for_key_path(
                this: &AnyObject,
                _sel: objc2::runtime::Sel,
                key_path: &NSString,
                of_object: &AnyObject,
                _change: &AnyObject,
                _context: &AnyObject,
              ) {
                if key_path.to_string() == "title" {
                  unsafe {
                    let function = this.get_ivar::<*mut c_void>("function");
                    if !function.is_null() {
                      let function = &mut *(*function as *mut Box<dyn Fn(String)>);
                      let title: *const NSString = objc2::msg_send![of_object, title];
                      (function)((*title).to_string());
                    }
                  }
                }
              }
              cls.register()
            }
            None => class!(DocumentTitleChangedDelegate),
          };

          let handler: Retained<AnyObject> = objc2::msg_send_id![cls, new];
          let document_title_changed_handler =
            Box::into_raw(Box::new(document_title_changed_handler));

          let ivar = handler.class().instance_variable("function").unwrap();
          let ivar_delegate = ivar.load_mut(&mut *Retained::into_raw(handler.clone()));
          *ivar_delegate = document_title_changed_handler as *mut _ as *mut c_void;

          webview.addObserver_forKeyPath_options_context(
            &*(Retained::cast::<NSObject>(handler)),
            &NSString::from_str("title"),
            NSKeyValueObservingOptions::NSKeyValueObservingOptionNew,
            null_mut(),
          );

          document_title_changed_handler
        } else {
          null_mut()
        };

      // Navigation handler
      extern "C" fn navigation_policy(
        this: &AnyObject,
        _: objc2::runtime::Sel,
        _: &AnyObject,
        action: &WKNavigationAction,
        handler: &block2::Block<dyn Fn(WKNavigationActionPolicy)>,
      ) {
        unsafe {
          // shouldPerformDownload is only available on macOS 11.3+
          let can_download = action.respondsToSelector(objc2::sel!(shouldPerformDownload));
          let should_download: bool = if can_download {
            action.shouldPerformDownload()
          } else {
            false
          };
          let request = action.request();
          let url = request.URL().unwrap().absoluteString().unwrap();
          let target_frame = action.targetFrame().unwrap();
          let is_main_frame = target_frame.isMainFrame();

          if should_download {
            let has_download_handler = this.get_ivar::<*mut c_void>("HasDownloadHandler");
            if !has_download_handler.is_null() {
              let has_download_handler = &mut *(*has_download_handler as *mut Box<bool>);
              if **has_download_handler {
                (*handler).call((WKNavigationActionPolicy::Download,));
              } else {
                (*handler).call((WKNavigationActionPolicy::Cancel,));
              }
            } else {
              (*handler).call((WKNavigationActionPolicy::Cancel,));
            }
          } else {
            let function = this.get_ivar::<*mut c_void>("navigation_policy_function");
            if !function.is_null() {
              let function = &mut *(*function as *mut Box<dyn for<'s> Fn(String, bool) -> bool>);
              match (function)(url.to_string(), is_main_frame) {
                true => (*handler).call((WKNavigationActionPolicy::Allow,)),
                false => (*handler).call((WKNavigationActionPolicy::Cancel,)),
              };
            } else {
              (*handler).call((WKNavigationActionPolicy::Allow,));
            }
          }
        }
      }

      // Navigation handler
      extern "C" fn navigation_policy_response(
        this: &AnyObject,
        _: objc2::runtime::Sel,
        _: &AnyObject,
        response: &WKNavigationResponse,
        handler: &block2::Block<dyn Fn(WKNavigationResponsePolicy)>,
      ) {
        unsafe {
          let can_show_mime_type = response.canShowMIMEType();

          if !can_show_mime_type {
            let has_download_handler = this.get_ivar::<*mut c_void>("HasDownloadHandler");
            if !has_download_handler.is_null() {
              let has_download_handler = &mut *(*has_download_handler as *mut Box<bool>);
              if **has_download_handler {
                (*handler).call((WKNavigationResponsePolicy::Download,));
                return;
              }
            }
          }

          (*handler).call((WKNavigationResponsePolicy::Allow,));
        }
      }

      let pending_scripts = Arc::new(Mutex::new(Some(Vec::new())));

      let navigation_delegate_cls =
        match ClassBuilder::new("WryNavigationDelegate", NSObject::class()) {
          Some(mut cls) => {
            cls.add_ivar::<*mut c_void>("pending_scripts");
            cls.add_ivar::<*mut c_void>("HasDownloadHandler");
            cls.add_method(
              objc2::sel!(webView:decidePolicyForNavigationAction:decisionHandler:),
              navigation_policy as extern "C" fn(_, _, _, _, _),
            );
            cls.add_method(
              objc2::sel!(webView:decidePolicyForNavigationResponse:decisionHandler:),
              navigation_policy_response as extern "C" fn(_, _, _, _, _),
            );
            add_download_methods(&mut cls);
            add_navigation_mathods(&mut cls);
            cls.register()
          }
          None => objc2::class!(WryNavigationDelegate),
        };

      let navigation_policy_handler: Retained<AnyObject> =
        objc2::msg_send_id![navigation_delegate_cls, new];

      let ivar = (*navigation_policy_handler)
        .class()
        .instance_variable("pending_scripts")
        .unwrap();
      let ivar_delegate =
        ivar.load_mut(&mut *Retained::into_raw(navigation_policy_handler.clone()));
      *ivar_delegate = Box::into_raw(Box::new(pending_scripts.clone())) as *mut c_void;

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

        let ivar = navigation_policy_handler
          .class()
          .instance_variable("navigation_policy_function")
          .unwrap();
        let ivar_delegate =
          ivar.load_mut(&mut *Retained::into_raw(navigation_policy_handler.clone()));
        *ivar_delegate = function_ptr as *mut c_void;

        let has_download_handler = Box::into_raw(Box::new(Box::new(
          attributes.download_started_handler.is_some(),
        )));
        let ivar = navigation_policy_handler
          .class()
          .instance_variable("HasDownloadHandler")
          .unwrap();
        let ivar_delegate =
          ivar.load_mut(&mut *Retained::into_raw(navigation_policy_handler.clone()));
        *ivar_delegate = has_download_handler as *mut c_void;

        // Download handler
        let download_delegate = if attributes.download_started_handler.is_some()
          || attributes.download_completed_handler.is_some()
        {
          let cls = match ClassBuilder::new("WryDownloadDelegate", NSObject::class()) {
            Some(mut cls) => {
              cls.add_ivar::<*mut c_void>("started");
              cls.add_ivar::<*mut c_void>("completed");
              cls.add_method(
                objc2::sel!(download:decideDestinationUsingResponse:suggestedFilename:completionHandler:),
                download_policy as extern "C" fn(_, _, _, _, _, _),
              );
              cls.add_method(
                objc2::sel!(downloadDidFinish:),
                download_did_finish as extern "C" fn(_, _, _),
              );
              cls.add_method(
                objc2::sel!(download:didFailWithError:resumeData:),
                download_did_fail as extern "C" fn(_, _, _, _, _),
              );
              cls.register()
            }
            None => objc2::class!(WryDownloadDelegate),
          };

          let download_delegate: Retained<AnyObject> = objc2::msg_send_id![cls, new];
          if let Some(download_started_handler) = attributes.download_started_handler {
            let download_started_ptr = Box::into_raw(Box::new(download_started_handler));
            let ivar = download_delegate
              .class()
              .instance_variable("started")
              .unwrap();
            let ivar_delegate = ivar.load_mut(&mut *Retained::into_raw(download_delegate.clone()));
            *ivar_delegate = download_started_ptr as *mut _ as *mut c_void;
          }
          if let Some(download_completed_handler) = attributes.download_completed_handler {
            let download_completed_ptr = Box::into_raw(Box::new(download_completed_handler));
            let ivar = download_delegate
              .class()
              .instance_variable("completed")
              .unwrap();
            let ivar_delegate = ivar.load_mut(&mut *Retained::into_raw(download_delegate.clone()));
            *ivar_delegate = download_completed_ptr as *mut _ as *mut c_void;
          }

          set_download_delegate(navigation_policy_handler.clone(), download_delegate.clone());

          Retained::into_raw(navigation_policy_handler.clone())
        } else {
          null_mut()
        };

        (function_ptr, download_delegate)
      } else {
        (null_mut(), null_mut())
      };

      let page_load_handler = set_navigation_methods(
        Retained::into_raw(navigation_policy_handler.clone()),
        webview.clone(),
        attributes.on_page_load_handler,
      );

      webview.setNavigationDelegate(Some(
        &(Retained::cast::<ProtocolObject<dyn WKNavigationDelegate>>(navigation_policy_handler)),
      ));

      // File upload panel handler
      extern "C" fn run_file_upload_panel(
        _this: &ProtocolObject<dyn WKUIDelegate>,
        _: objc2::runtime::Sel,
        _webview: &WKWebView,
        open_panel_params: &WKOpenPanelParameters,
        _frame: &WKFrameInfo,
        handler: &block2::Block<dyn Fn(*const NSArray<NSURL>)>,
      ) {
        unsafe {
          if let Some(mtm) = MainThreadMarker::new() {
            let open_panel = objc2_app_kit::NSOpenPanel::openPanel(mtm);
            open_panel.setCanChooseFiles(true);
            let allow_multi = open_panel_params.allowsMultipleSelection();
            open_panel.setAllowsMultipleSelection(allow_multi);
            let allow_dir = open_panel_params.allowsDirectories();
            open_panel.setCanChooseDirectories(allow_dir);
            let ok: NSModalResponse = open_panel.runModal();
            if ok == NSModalResponseOK {
              let url = open_panel.URLs();
              (*handler).call((Retained::as_ptr(&url),));
            } else {
              (*handler).call((null_mut(),));
            }
          }
        }
      }

      extern "C" fn request_media_capture_permission(
        _this: &ProtocolObject<dyn WKUIDelegate>,
        _: objc2::runtime::Sel,
        _webview: &WKWebView,
        _origin: &WKSecurityOrigin,
        _frame: &WKFrameInfo,
        _type: WKMediaCaptureType,
        decision_handler: &Block<dyn Fn(WKPermissionDecision)>,
      ) {
        //https://developer.apple.com/documentation/webkit/wkpermissiondecision?language=objc
        (*decision_handler).call((WKPermissionDecision::Grant,));
      }

      let ui_delegate = match ClassBuilder::new("WebViewUIDelegate", NSObject::class()) {
        Some(mut ctl) => {
          ctl.add_method(
            objc2::sel!(webView:runOpenPanelWithParameters:initiatedByFrame:completionHandler:),
            run_file_upload_panel as extern "C" fn(_, _, _, _, _, _),
          );

          ctl.add_method(
            objc2::sel!(webView:requestMediaCapturePermissionForOrigin:initiatedByFrame:type:decisionHandler:),
            request_media_capture_permission as extern "C" fn(_, _, _, _, _, _, _),
          );

          ctl.register()
        }
        None => class!(WebViewUIDelegate),
      };
      let ui_delegate: Retained<ProtocolObject<dyn WKUIDelegate>> =
        objc2::msg_send_id![ui_delegate, new];
      webview.setUIDelegate(Some(&*ui_delegate));

      // File drop handling
      #[cfg(target_os = "macos")]
      let drag_drop_ptr = webview.ivars().drag_drop_handler.as_ptr();

      // ns window is required for the print operation
      #[cfg(target_os = "macos")]
      {
        let ns_window = ns_view.window().unwrap();

        let can_set_titlebar_style =
          ns_window.respondsToSelector(objc2::sel!(setTitlebarSeparatorStyle:));

        if can_set_titlebar_style == YES {
          ns_window.setTitlebarSeparatorStyle(NSTitlebarSeparatorStyle::None);
        }
      }

      let w = Self {
        webview: webview.clone(),
        manager: manager.clone(),
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
          ns_view.addSubview(&webview);
        } else {
          let parent_view_cls = match ClassBuilder::new("WryWebViewParent", NSView::class()) {
            Some(mut decl) => {
              decl.add_method(objc2::sel!(keyDown:), key_down as extern "C" fn(_, _, _));

              extern "C" fn key_down(_this: &NSView, _sel: objc2::runtime::Sel, event: &NSEvent) {
                unsafe {
                  let mtm = MainThreadMarker::new().unwrap();
                  let app = NSApp(mtm);
                  if let Some(menu) = app.mainMenu() {
                    menu.performKeyEquivalent(event);
                  }
                }
              }

              decl.register()
            }
            None => class!(NSView),
          };

          let parent_view: Allocated<NSView> = objc2::msg_send_id![parent_view_cls, alloc];
          let parent_view = NSView::init(parent_view);
          parent_view.setAutoresizingMask(
            NSAutoresizingMaskOptions::NSViewHeightSizable
              | NSAutoresizingMaskOptions::NSViewWidthSizable,
          );
          parent_view.addSubview(&webview.clone());

          // inject the webview into the window
          let ns_window = ns_view.window().unwrap();
          // Tell the webview receive keyboard events in the window.
          // See https://github.com/tauri-apps/wry/issues/739
          ns_window.setContentView(Some(&parent_view));
          ns_window.makeFirstResponder(Some(&webview));
        }

        // make sure the window is always on top when we create a new webview
        let mtm = MainThreadMarker::new().unwrap();
        let app = NSApplication::sharedApplication(mtm);
        NSApplication::activate(&app);
      }

      #[cfg(target_os = "ios")]
      {
        ns_view.addSubview(&webview);
      }

      Ok(w)
    }
  }

  pub fn url(&self) -> crate::Result<String> {
    url_from_webview(&self.webview)
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
          let handler = block2::RcBlock::new(move |val: *mut AnyObject, _err: *mut NSError| {
            #[cfg(feature = "tracing")]
            span.lock().unwrap().take();

            let mut result = String::new();

            if val != null_mut() {
              let json_ns_data = NSJSONSerialization::dataWithJSONObject_options_error(
                &*val,
                objc2_foundation::NSJSONWritingOptions::NSJSONWritingFragmentsAllowed,
              )
              .unwrap();
              let json_string = Retained::cast::<NSString>(json_ns_data);
              result = json_string.to_string();
            }

            callback(result);
          })
          .copy();

          self
            .webview
            .evaluateJavaScript_completionHandler(&NSString::from_str(js), Some(&handler));
        } else {
          #[cfg(feature = "tracing")]
          let handler = block2::RcBlock::new(move |val: *mut AnyObject, _err: *mut NSError| {
            span.lock().unwrap().take();
          })
          .copy();

          self
            .webview
            .evaluateJavaScript_completionHandler(&NSString::from_str(js), None);
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
      let userscript = WKUserScript::alloc();
      // FIXME: We allow subframe injection because webview2 does and cannot be disabled (currently).
      // once webview2 allows disabling all-frame script injection, forMainFrameOnly should be enabled
      // if it does not break anything. (originally added for isolation pattern).
      let script = WKUserScript::initWithSource_injectionTime_forMainFrameOnly(
        userscript,
        &NSString::from_str(js),
        WKUserScriptInjectionTime::AtDocumentStart,
        false,
      );
      self.manager.addUserScript(&script);
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
      let config = self.webview.configuration();
      let store = config.websiteDataStore();
      let all_data_types = WKWebsiteDataStore::allWebsiteDataTypes();
      let date = NSDate::dateWithTimeIntervalSince1970(0.0);
      let handler = block2::RcBlock::new(|| {});
      store.removeDataOfTypes_modifiedSince_completionHandler(&all_data_types, &date, &handler);
    }
    Ok(())
  }

  fn navigate_to_url(&self, url: &str, headers: Option<http::HeaderMap>) -> crate::Result<()> {
    // Safety: objc runtime calls are unsafe
    unsafe {
      let url = NSURL::URLWithString(&NSString::from_str(url)).unwrap();
      let mut request = NSMutableURLRequest::requestWithURL(&url);
      if let Some(headers) = headers {
        for (name, value) in headers.iter() {
          let key = NSString::from_str(name.as_str());
          let value = NSString::from_str(value.to_str().unwrap_or_default());
          request.addValue_forHTTPHeaderField(&value, &key);
        }
      }
      self.webview.loadRequest(&request);
    }

    Ok(())
  }

  fn navigate_to_string(&self, html: &str) {
    // Safety: objc runtime calls are unsafe
    unsafe {
      self
        .webview
        .loadHTMLString_baseURL(&NSString::from_str(html), None);
    }
  }

  fn set_user_agent(&self, user_agent: &str) {
    unsafe {
      self
        .webview
        .setCustomUserAgent(Some(&NSString::from_str(user_agent)));
    }
  }

  pub fn print(&self) -> crate::Result<()> {
    // Safety: objc runtime calls are unsafe
    #[cfg(target_os = "macos")]
    unsafe {
      let can_print = self
        .webview
        .respondsToSelector(objc2::sel!(printOperationWithPrintInfo:));
      if can_print {
        // Create a shared print info
        let print_info = NSPrintInfo::sharedPrintInfo();

        // Create new print operation from the webview content
        let print_operation = self.webview.printOperationWithPrintInfo(&print_info);

        // Allow the modal to detach from the current thread and be non-blocker
        print_operation.setCanSpawnSeparateThread(true);

        // Launch the modal
        let window = self.webview.window().unwrap();
        print_operation.runOperationModalForWindow_delegate_didRunSelector_contextInfo(
          &window,
          None,
          None,
          null_mut(),
        )
      }
    }

    Ok(())
  }

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn open_devtools(&self) {
    #[cfg(target_os = "macos")]
    unsafe {
      // taken from <https://github.com/WebKit/WebKit/blob/784f93cb80a386c29186c510bba910b67ce3adc1/Source/WebKit/UIProcess/API/Cocoa/WKWebView.mm#L1939>
      let tool: Retained<AnyObject> = objc2::msg_send_id![&self.webview, _inspector];
      let () = objc2::msg_send![&tool, show];
    }
  }

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn close_devtools(&self) {
    #[cfg(target_os = "macos")]
    unsafe {
      // taken from <https://github.com/WebKit/WebKit/blob/784f93cb80a386c29186c510bba910b67ce3adc1/Source/WebKit/UIProcess/API/Cocoa/WKWebView.mm#L1939>
      let tool: Retained<AnyObject> = objc2::msg_send_id![&self.webview, _inspector];
      let () = objc2::msg_send![&tool, close];
    }
  }

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn is_devtools_open(&self) -> bool {
    #[cfg(target_os = "macos")]
    unsafe {
      // taken from <https://github.com/WebKit/WebKit/blob/784f93cb80a386c29186c510bba910b67ce3adc1/Source/WebKit/UIProcess/API/Cocoa/WKWebView.mm#L1939>
      let tool: Retained<AnyObject> = objc2::msg_send_id![&self.webview, _inspector];
      let is_visible: bool = objc2::msg_send![&tool, isVisible];
      is_visible
    }
    #[cfg(not(target_os = "macos"))]
    false
  }

  pub fn zoom(&self, scale_factor: f64) -> crate::Result<()> {
    unsafe {
      self.webview.setPageZoom(scale_factor);
    }

    Ok(())
  }

  pub fn set_background_color(&self, _background_color: RGBA) -> Result<()> {
    Ok(())
  }

  pub fn bounds(&self) -> crate::Result<Rect> {
    unsafe {
      let parent = self.webview.superview().unwrap();
      let parent_frame = parent.frame();
      let webview_frame = self.webview.frame();

      Ok(Rect {
        position: LogicalPosition::new(
          webview_frame.origin.x,
          parent_frame.size.height - webview_frame.origin.y - webview_frame.size.height,
        )
        .into(),
        size: LogicalSize::new(webview_frame.size.width, webview_frame.size.height).into(),
      })
    }
  }

  pub fn set_bounds(&self, #[allow(unused)] bounds: Rect) -> crate::Result<()> {
    #[cfg(target_os = "macos")]
    if self.is_child {
      let window = self.webview.window().unwrap();
      let scale_factor = window.backingScaleFactor();
      let (x, y) = bounds.position.to_logical::<f64>(scale_factor).into();
      let (width, height) = bounds.size.to_logical::<i32>(scale_factor).into();

      unsafe {
        let parent_view = self.webview.superview().unwrap();
        let frame = CGRect {
          origin: window_position(&parent_view, x, y, height),
          size: CGSize::new(width, height),
        };
        self.webview.setFrame(frame);
      }
    }

    Ok(())
  }

  pub fn set_visible(&self, visible: bool) -> Result<()> {
    self.webview.setHidden(!visible);
    Ok(())
  }

  pub fn focus(&self) -> Result<()> {
    let window = self.webview.window().unwrap();
    window.makeFirstResponder(Some(&self.webview));
    Ok(())
  }

  #[cfg(target_os = "macos")]
  pub(crate) fn reparent(&self, window: Retained<objc2_app_kit::NSWindow>) -> crate::Result<()> {
    unsafe {
      let content_view = window.contentView().unwrap();
      content_view.addSubview(&self.webview);
    }

    Ok(())
  }
}

pub fn url_from_webview(webview: &WKWebView) -> Result<String> {
  let url_obj = unsafe { webview.URL().unwrap() };
  let absolute_url = unsafe { url_obj.absoluteString().unwrap() };

  let bytes = {
    let bytes: *const c_char = absolute_url.UTF8String();
    bytes as *const u8
  };

  // 4 represents utf8 encoding
  let len = absolute_url.lengthOfBytesUsingEncoding(4);
  let bytes = unsafe { std::slice::from_raw_parts(bytes, len) };

  std::str::from_utf8(bytes)
    .map(Into::into)
    .map_err(Into::into)
}

pub fn platform_webview_version() -> Result<String> {
  unsafe {
    let bundle = NSBundle::bundleWithIdentifier(&NSString::from_str("com.apple.WebKit")).unwrap();
    let dict = bundle.infoDictionary().unwrap();
    let webkit_version = dict
      .objectForKey(&NSString::from_str("CFBundleVersion"))
      .unwrap();
    let webkit_version = Retained::cast::<NSString>(webkit_version);

    bundle.unload();
    Ok(webkit_version.to_string())
  }
}

impl Drop for InnerWebView {
  fn drop(&mut self) {
    // We need to drop handler closures here
    unsafe {
      if !self.ipc_handler_ptr.is_null() {
        drop(Box::from_raw(self.ipc_handler_ptr));

        let ipc = NSString::from_str(IPC_MESSAGE_HANDLER_NAME);
        self.manager.removeScriptMessageHandlerForName(&ipc);
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
        drop(Rc::from_raw(self.drag_drop_ptr));
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
      self.webview.removeFromSuperview();
      self.webview.retain();
      self.manager.retain();
    }
  }
}

/// Converts from wry screen-coordinates to macOS screen-coordinates.
/// wry: top-left is (0, 0) and y increasing downwards
/// macOS: bottom-left is (0, 0) and y increasing upwards
unsafe fn window_position(view: &NSView, x: i32, y: i32, height: f64) -> CGPoint {
  let frame: CGRect = view.frame();
  CGPoint::new(x as f64, frame.size.height - y as f64 - height)
}

pub struct WryWebViewIvars {
  #[cfg(target_os = "macos")]
  drag_drop_handler: RefCell<Box<dyn Fn(DragDropEvent) -> bool>>,
  #[cfg(target_os = "macos")]
  accept_first_mouse: objc2::runtime::Bool,
}

declare_class!(
  pub struct WryWebView;

  unsafe impl ClassType for WryWebView {
    type Super = WKWebView;
    type Mutability = MainThreadOnly;
    const NAME: &'static str = "WryWebView";
  }

  impl DeclaredClass for WryWebView {
    type Ivars = WryWebViewIvars;
  }

  unsafe impl WryWebView {
    #[method(performKeyEquivalent:)]
    fn perform_key_equivalent(
      &self,
      _event: &NSEvent,
    ) -> objc2::runtime::Bool {
      objc2::runtime::Bool::NO
    }

    #[method(acceptsFirstMouse:)]
    fn accept_first_mouse(
      &self,
      _event: &NSEvent,
    ) -> objc2::runtime::Bool {
        self.ivars().accept_first_mouse
    }
  }

  // Drag & Drop
  #[cfg(target_os = "macos")]
  unsafe impl WryWebView {
    #[method(draggingEntered:)]
    unsafe fn dragging_entered(
      &self,
      drag_info: &ProtocolObject<dyn NSDraggingInfo>,
    ) -> NSDragOperation {
      drag_drop::dragging_entered(self, drag_info)
    }

    #[method(draggingUpdated:)]
    unsafe fn dragging_updated(
      &self,
      drag_info: &ProtocolObject<dyn NSDraggingInfo>,
    ) -> NSDragOperation {
      drag_drop::dragging_updated(self, drag_info)
    }

    #[method(performDragOperation:)]
    unsafe fn perform_drag_operation(
      &self,
      drag_info: &ProtocolObject<dyn NSDraggingInfo>,
    ) -> Bool {
      drag_drop::perform_drag_operation(self, drag_info)
    }

    #[method(draggingExited:)]
    unsafe fn dragging_exited(
      &self,
      drag_info: &ProtocolObject<dyn NSDraggingInfo>,
    ) {
      drag_drop::dragging_exited(self, drag_info)
    }
  }

  // Synthetic mouse events
  #[cfg(target_os = "macos")]
  unsafe impl WryWebView {
    #[method(otherMouseDown:)]
    unsafe fn other_mouse_down(
      &self,
      event: &NSEvent,
    ) {
      synthetic_mouse_events::other_mouse_down(self, event)
    }

    #[method(otherMouseUp:)]
    unsafe fn other_mouse_up(
      &self,
      event: &NSEvent,
    ) {
      synthetic_mouse_events::other_mouse_up(self, event)
    }
  }
);
