// Copyright 2020-2024 Tauri Programme within The Commons Conservancy
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
mod util;

use block2::Block;

use download::{navigation_download_action, navigation_download_response};
use dpi::{LogicalPosition, LogicalSize};
use navigation::{
  did_commit_navigation, did_finish_navigation, navigation_policy, navigation_policy_response,
};
use objc2::{
  class,
  declare::ClassBuilder,
  declare_class, msg_send, msg_send_id,
  mutability::{InteriorMutable, MainThreadOnly},
  rc::{Allocated, Retained},
  runtime::{AnyClass, AnyObject, Bool, MessageReceiver, NSObject, ProtocolObject},
  ClassType, DeclaredClass,
};
use objc2_app_kit::{
  NSApp, NSApplication, NSAutoresizingMaskOptions, NSDragOperation, NSDraggingInfo, NSEvent,
  NSModalResponse, NSModalResponseOK, NSPrintInfo, NSTitlebarSeparatorStyle, NSView,
};
use objc2_foundation::{
  ns_string, CGPoint, CGRect, CGSize, MainThreadMarker, NSArray, NSBundle, NSData, NSDate,
  NSDictionary, NSError, NSHTTPURLResponse, NSJSONSerialization, NSKeyValueChangeKey,
  NSKeyValueObservingOptions, NSMutableDictionary, NSMutableURLRequest, NSNumber,
  NSObjectNSKeyValueCoding, NSObjectNSKeyValueObserverRegistration, NSObjectProtocol, NSString,
  NSURLResponse, NSUTF8StringEncoding, NSURL, NSUUID,
};
#[cfg(target_os = "ios")]
use objc2_ui_kit::UIScrollView;
use objc2_web_kit::{
  WKAudiovisualMediaTypes, WKDownload, WKDownloadDelegate, WKFrameInfo, WKMediaCaptureType,
  WKNavigation, WKNavigationAction, WKNavigationActionPolicy, WKNavigationDelegate,
  WKNavigationResponse, WKNavigationResponsePolicy, WKOpenPanelParameters, WKPermissionDecision,
  WKScriptMessage, WKScriptMessageHandler, WKSecurityOrigin, WKUIDelegate, WKURLSchemeHandler,
  WKURLSchemeTask, WKUserContentController, WKUserScript, WKUserScriptInjectionTime, WKWebView,
  WKWebViewConfiguration, WKWebsiteDataStore,
};
use once_cell::sync::Lazy;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use std::{
  borrow::Cow,
  collections::{HashMap, HashSet},
  ffi::{c_void, CStr},
  os::raw::c_char,
  panic::{catch_unwind, AssertUnwindSafe},
  path::PathBuf,
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
  wkwebview::download::{download_did_fail, download_did_finish, download_policy},
  DragDropEvent, Error, PageLoadEvent, Rect, RequestAsyncResponder, Result, WebContext,
  WebViewAttributes, RGBA,
};

use http::{
  header::{CONTENT_LENGTH, CONTENT_TYPE},
  status::StatusCode,
  version::Version,
  Request, Response as HttpResponse,
};

use self::util::Counter;

const IPC_MESSAGE_HANDLER_NAME: &str = "ipc";

static COUNTER: Counter = Counter::new();
static WEBVIEW_IDS: Lazy<Mutex<HashSet<u32>>> = Lazy::new(Default::default);

#[derive(Debug, Default, Copy, Clone)]
pub struct PrintMargin {
  pub top: f32,
  pub right: f32,
  pub bottom: f32,
  pub left: f32,
}

#[derive(Debug, Default, Clone)]
pub struct PrintOptions {
  pub margins: PrintMargin,
}

pub(crate) struct InnerWebView {
  pub webview: Retained<WryWebView>,
  pub manager: Retained<WKUserContentController>,
  is_child: bool,
  pending_scripts: Arc<Mutex<Option<Vec<String>>>>,
  // Note that if following functions signatures are changed in the future,
  // all functions pointer declarations in objc callbacks below all need to get updated.
  ipc_handler_delegate: Option<Retained<WryWebViewDelegate>>,
  #[allow(dead_code)]
  // We need this the keep the reference count
  document_title_changed_observer: Option<Retained<DocumentTitleChangedObserver>>,
  #[allow(dead_code)]
  // We need this the keep the reference count
  navigation_policy_delegate: Retained<WryNavigationDelegate>,
  #[allow(dead_code)]
  // We need this the keep the reference count
  download_delegate: Option<Retained<WryDownloadDelegate>>,
  protocol_ptrs: Vec<*mut Box<dyn Fn(Request<Vec<u8>>, RequestAsyncResponder)>>,
  webview_id: u32,
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
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    _web_context: Option<&mut WebContext>,
    is_child: bool,
  ) -> Result<Self> {
    let mtm = MainThreadMarker::new().ok_or(Error::NotMainThread)?;

    // Task handler for custom protocol
    extern "C" fn start_task<'a>(
      this: &AnyObject,
      _sel: objc2::runtime::Sel,
      webview: *mut WryWebView,
      task: *mut ProtocolObject<dyn WKURLSchemeTask>,
    ) {
      unsafe {
        #[cfg(feature = "tracing")]
        tracing::info_span!(parent: None, "wry::custom_protocol::handle", uri = tracing::field::Empty).entered();

        let task_key = (*task).hash(); // hash by task object address
        let task_uuid = (*webview).add_custom_task_key(task_key);

        let ivar = this.class().instance_variable("webview_id").unwrap();
        let webview_id: u32 = ivar.load::<u32>(this).clone();
        let ivar = this.class().instance_variable("function").unwrap();
        let function: &*mut c_void = ivar.load(this);
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
                  fn check_webview_id_valid(webview_id: u32) -> crate::Result<()> {
                    if !WEBVIEW_IDS.lock().unwrap().contains(&webview_id) {
                      return Err(crate::Error::CustomProtocolTaskInvalid);
                    }
                    Ok(())
                  }
                  /// Task may not live longer than async custom protocol handler.
                  ///
                  /// There are roughly 2 ways to cause segfault:
                  /// 1. Task has stopped. pointer of the task not valid anymore.
                  /// 2. Task had stopped, but the pointer of the task has allocated to a new task.
                  ///    Outdated custom handler may call to the new task instance and cause segfault.
                  fn check_task_is_valud(
                    webview: &WryWebView,
                    task_key: usize,
                    current_uuid: Retained<NSUUID>,
                  ) -> crate::Result<()> {
                    let latest_task_uuid = webview.get_custom_task_uuid(task_key);
                    if let Some(latest_uuid) = latest_task_uuid {
                      if latest_uuid != current_uuid {
                        return Err(crate::Error::CustomProtocolTaskInvalid);
                      }
                    } else {
                      return Err(crate::Error::CustomProtocolTaskInvalid);
                    }
                    Ok(())
                  }

                  // FIXME: This is 10000% unsafe. `task` and `webview` are not guaranteed to be valid.
                  // We should consider use sync command only.
                  unsafe fn response(
                    task: *mut ProtocolObject<dyn WKURLSchemeTask>,
                    webview: *mut WryWebView,
                    task_key: usize,
                    task_uuid: Retained<NSUUID>,
                    webview_id: u32,
                    url: Retained<NSURL>,
                    sent_response: HttpResponse<Cow<'_, [u8]>>,
                  ) -> crate::Result<()> {
                    check_task_is_valud(&*webview, task_key, task_uuid.clone())?;

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

                    check_webview_id_valid(webview_id)?;
                    check_task_is_valud(&*webview, task_key, task_uuid.clone())?;
                    (*task).didReceiveResponse(&response);

                    // Send data
                    let bytes = content.as_ptr() as *mut c_void;
                    let data = NSData::alloc();
                    // MIGRATE NOTE: we copied the content to the NSData because content will be freed
                    // when out of scope but NSData will also free the content when it's done and cause doube free.
                    let data = NSData::initWithBytes_length(data, bytes, content.len());
                    check_webview_id_valid(webview_id)?;
                    check_task_is_valud(&*webview, task_key, task_uuid.clone())?;
                    (*task).didReceiveData(&data);

                    // Finish
                    check_webview_id_valid(webview_id)?;
                    check_task_is_valud(&*webview, task_key, task_uuid.clone())?;
                    (*task).didFinish();

                    (*webview).remove_custom_task_key(task_key);
                    Ok(())
                  }

                  let _ = response(
                    task,
                    webview,
                    task_key,
                    task_uuid,
                    webview_id,
                    url.clone(),
                    sent_response,
                  );
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
      _this: &ProtocolObject<dyn WKURLSchemeHandler>,
      _sel: objc2::runtime::Sel,
      webview: &mut WryWebView,
      task: &ProtocolObject<dyn WKURLSchemeTask>,
    ) {
      webview.remove_custom_task_key(task.hash());
    }

    let mut wv_ids = WEBVIEW_IDS.lock().unwrap();
    let webview_id = COUNTER.next();
    wv_ids.insert(webview_id);
    drop(wv_ids);

    // Safety: objc runtime calls are unsafe
    unsafe {
      // Config and custom protocol
      let config = WKWebViewConfiguration::new();
      let mut protocol_ptrs = Vec::new();

      let os_version = util::operating_system_version();

      #[cfg(target_os = "macos")]
      let custom_data_store_available = os_version.0 >= 14;

      #[cfg(target_os = "ios")]
      let custom_data_store_available = os_version.0 >= 17;

      // Incognito mode
      let data_store = match (
        attributes.incognito,
        custom_data_store_available,
        pl_attrs.data_store_identifier,
      ) {
        (true, _, _) => WKWebsiteDataStore::nonPersistentDataStore(),
        // if data_store_identifier is given and custom data stores are available, use custom store
        (false, true, Some(data_store)) => {
          let identifier = NSUUID::from_bytes(data_store);
          WKWebsiteDataStore::dataStoreForIdentifier(&identifier)
        }
        // default data store
        _ => WKWebsiteDataStore::defaultDataStore(),
      };

      for (name, function) in attributes.custom_protocols {
        let scheme_name = format!("{}URLSchemeHandler", name);
        let cls = ClassBuilder::new(&scheme_name, NSObject::class());
        let cls = match cls {
          Some(mut cls) => {
            cls.add_ivar::<*mut c_void>("function");
            cls.add_ivar::<u32>("webview_id");
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

        let ivar = (*handler).class().instance_variable("webview_id").unwrap();
        let ivar_delegate = ivar.load_mut(&mut *handler);
        *ivar_delegate = webview_id;

        let config_unwind_safe = AssertUnwindSafe(&config);
        let handler_unwind_safe = AssertUnwindSafe(handler);
        if catch_unwind(|| {
          config_unwind_safe.setURLSchemeHandler_forURLScheme(
            Some(&*(handler_unwind_safe.cast::<ProtocolObject<dyn WKURLSchemeHandler>>())),
            &NSString::from_str(&name),
          );
        })
        .is_err()
        {
          return Err(Error::UrlSchemeRegisterError(name));
        }
      }

      // WebView and manager
      let manager = config.userContentController();
      let webview = mtm.alloc::<WryWebView>().set_ivars(WryWebViewIvars {
        is_child,
        #[cfg(target_os = "macos")]
        drag_drop_handler: match attributes.drag_drop_handler {
          Some(handler) => handler,
          None => Box::new(|_| false),
        },
        #[cfg(target_os = "macos")]
        accept_first_mouse: Bool::new(attributes.accept_first_mouse),
        custom_protocol_task_ids: HashMap::new(),
      });

      config.setWebsiteDataStore(&data_store);
      let _preference = config.preferences();
      let _yes = NSNumber::numberWithBool(true);

      #[cfg(feature = "mac-proxy")]
      if let Some(proxy_config) = attributes.proxy_config {
        let proxy_config = match proxy_config {
          ProxyConfig::Http(endpoint) => {
            let nw_endpoint = nw_endpoint_t::try_from(endpoint).unwrap();
            nw_proxy_config_create_http_connect(nw_endpoint, null_mut())
          }
          ProxyConfig::Socks5(endpoint) => {
            let nw_endpoint = nw_endpoint_t::try_from(endpoint).unwrap();
            nw_proxy_config_create_socksv5(nw_endpoint)
          }
        };

        let proxies: Retained<NSArray<NSObject>> = NSArray::arrayWithObject(&*proxy_config);
        data_store.setValue_forKey(Some(&proxies), ns_string!("proxyConfigurations"));
      }

      _preference.setValue_forKey(
        Some(&_yes),
        ns_string!("allowsPictureInPictureMediaPlayback"),
      );

      #[cfg(target_os = "ios")]
      config.setValue_forKey(Some(&_yes), ns_string!("allowsInlineMediaPlayback"));

      if attributes.autoplay {
        config.setMediaTypesRequiringUserActionForPlayback(
          WKAudiovisualMediaTypes::WKAudiovisualMediaTypeNone,
        );
      }

      #[cfg(feature = "transparent")]
      if attributes.transparent {
        let no = NSNumber::numberWithBool(false);
        // Equivalent Obj-C:
        config.setValue_forKey(Some(&no), ns_string!("drawsBackground"));
      }

      #[cfg(feature = "fullscreen")]
      // Equivalent Obj-C:
      _preference.setValue_forKey(Some(&_yes), ns_string!("fullScreenEnabled"));

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
        webview.setAllowsBackForwardNavigationGestures(attributes.back_forward_navigation_gestures);

        // tabFocusesLinks
        _preference.setValue_forKey(Some(&_yes), ns_string!("tabFocusesLinks"));
      }
      #[cfg(target_os = "ios")]
      {
        // set all autoresizingmasks
        webview.setAutoresizingMask(NSAutoresizingMaskOptions::from_bits(31).unwrap());
        // let () = msg_send![webview, setAutoresizingMask: 31];

        // disable scroll bounce by default
        let scroll_view: UIScrollView = webview.scrollView(); // FIXME: not test yet
        scroll_view.setBounces(false)
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
      let ipc_handler_delegate = if let Some(ipc_handler) = attributes.ipc_handler {
        let delegate = WryWebViewDelegate::new(manager.clone(), ipc_handler, mtm);
        Some(delegate)
      } else {
        None
      };

      // Document title changed handler
      let document_title_changed_observer =
        if let Some(handler) = attributes.document_title_changed_handler {
          let delegate = DocumentTitleChangedObserver::new(webview.clone(), handler);
          Some(delegate)
        } else {
          None
        };

      let pending_scripts = Arc::new(Mutex::new(Some(Vec::new())));
      let has_download_handler = attributes.download_started_handler.is_some();
      // Download handler
      let download_delegate = if attributes.download_started_handler.is_some()
        || attributes.download_completed_handler.is_some()
      {
        let delegate = WryDownloadDelegate::new(
          attributes.download_started_handler,
          attributes.download_completed_handler,
          mtm,
        );
        Some(delegate)
      } else {
        None
      };

      let navigation_policy_delegate = WryNavigationDelegate::new(
        webview.clone(),
        pending_scripts.clone(),
        has_download_handler,
        attributes.navigation_handler,
        attributes.new_window_req_handler,
        download_delegate.clone(),
        attributes.on_page_load_handler,
        mtm,
      );

      let proto_navigation_policy_delegate =
        ProtocolObject::from_ref(navigation_policy_delegate.as_ref());
      webview.setNavigationDelegate(Some(proto_navigation_policy_delegate));

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

      // ns window is required for the print operation
      #[cfg(target_os = "macos")]
      {
        let ns_window = ns_view.window().unwrap();
        let can_set_titlebar_style =
          ns_window.respondsToSelector(objc2::sel!(setTitlebarSeparatorStyle:));
        if can_set_titlebar_style {
          ns_window.setTitlebarSeparatorStyle(NSTitlebarSeparatorStyle::None);
        }
      }

      let w = Self {
        webview: webview.clone(),
        manager: manager.clone(),
        pending_scripts,
        ipc_handler_delegate,
        document_title_changed_observer,
        navigation_policy_delegate,
        download_delegate,
        protocol_ptrs,
        is_child,
        webview_id,
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
              let json_string = NSString::alloc();
              let json_string =
                NSString::initWithData_encoding(json_string, &json_ns_data, NSUTF8StringEncoding)
                  .unwrap();

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
    unsafe {
      let userscript = WKUserScript::alloc();
      // TODO: feature to allow injecting into subframes
      let script = WKUserScript::initWithSource_injectionTime_forMainFrameOnly(
        userscript,
        &NSString::from_str(js),
        WKUserScriptInjectionTime::AtDocumentStart,
        true,
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
    self.print_with_options(&PrintOptions::default())
  }

  pub fn print_with_options(&self, options: &PrintOptions) -> crate::Result<()> {
    // Safety: objc runtime calls are unsafe
    #[cfg(target_os = "macos")]
    unsafe {
      let can_print = self
        .webview
        .respondsToSelector(objc2::sel!(printOperationWithPrintInfo:));
      if can_print {
        // Create a shared print info
        let print_info = NSPrintInfo::sharedPrintInfo();
        // let print_info: id = msg_send![print_info, init];
        print_info.setTopMargin(options.margins.top.into());
        print_info.setRightMargin(options.margins.right.into());
        print_info.setBottomMargin(options.margins.bottom.into());
        print_info.setLeftMargin(options.margins.left.into());

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
    WEBVIEW_IDS.lock().unwrap().remove(&self.webview_id);

    // We need to drop handler closures here
    unsafe {
      if let Some(ipc_handler) = self.ipc_handler_delegate.take() {
        let ipc = NSString::from_str(IPC_MESSAGE_HANDLER_NAME);
        // this will decrease the retain count of the ipc handler and trigger the drop
        ipc_handler
          .ivars()
          .controller
          .removeScriptMessageHandlerForName(&ipc);
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
  is_child: bool,
  #[cfg(target_os = "macos")]
  drag_drop_handler: Box<dyn Fn(DragDropEvent) -> bool>,
  #[cfg(target_os = "macos")]
  accept_first_mouse: objc2::runtime::Bool,
  custom_protocol_task_ids: HashMap<usize, Retained<NSUUID>>,
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
    ) -> bool {
      // This is a temporary workaround for https://github.com/tauri-apps/tauri/issues/9426
      // FIXME: When the webview is a child webview, performKeyEquivalent always return YES
      // and stop propagating the event to the window, hence the menu shortcut won't be
      // triggered. However, overriding this method also means the cmd+key event won't be
      // handled in webview, which means the key cannot be listened by JavaScript.
      if self.ivars().is_child {
        false
      } else {
        unsafe {
          let _: Bool = self.send_super_message(
            WKWebView::class(),
            objc2::sel!(performKeyEquivalent:),
            (_event,),
          );
        };
        true
      }
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
    fn dragging_entered(
      &self,
      drag_info: &ProtocolObject<dyn NSDraggingInfo>,
    ) -> NSDragOperation {
      drag_drop::dragging_entered(self, drag_info)
    }

    #[method(draggingUpdated:)]
    fn dragging_updated(
      &self,
      drag_info: &ProtocolObject<dyn NSDraggingInfo>,
    ) -> NSDragOperation {
      drag_drop::dragging_updated(self, drag_info)
    }

    #[method(performDragOperation:)]
    fn perform_drag_operation(
      &self,
      drag_info: &ProtocolObject<dyn NSDraggingInfo>,
    ) -> Bool {
      drag_drop::perform_drag_operation(self, drag_info)
    }

    #[method(draggingExited:)]
    fn dragging_exited(
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
    fn other_mouse_down(
      &self,
      event: &NSEvent,
    ) {
      synthetic_mouse_events::other_mouse_down(self, event)
    }

    #[method(otherMouseUp:)]
    fn other_mouse_up(
      &self,
      event: &NSEvent,
    ) {
      synthetic_mouse_events::other_mouse_up(self, event)
    }
  }
);

// Custom Protocol Task Checker
impl WryWebView {
  fn add_custom_task_key(&mut self, task_id: usize) -> Retained<NSUUID> {
    let task_uuid = NSUUID::new();
    self
      .ivars_mut()
      .custom_protocol_task_ids
      .insert(task_id, task_uuid.clone());
    task_uuid
  }
  fn remove_custom_task_key(&mut self, task_id: usize) {
    self.ivars_mut().custom_protocol_task_ids.remove(&task_id);
  }
  fn get_custom_task_uuid(&self, task_id: usize) -> Option<Retained<NSUUID>> {
    self.ivars().custom_protocol_task_ids.get(&task_id).cloned()
  }
}

struct WryWebViewDelegateIvars {
  controller: Retained<WKUserContentController>,
  ipc_handler: Box<dyn Fn(Request<String>)>,
}

declare_class!(
  struct WryWebViewDelegate;

  unsafe impl ClassType for WryWebViewDelegate {
    type Super = NSObject;
    type Mutability = MainThreadOnly;
    const NAME: &'static str = "WryWebViewDelegate";
  }

  impl DeclaredClass for WryWebViewDelegate {
    type Ivars = WryWebViewDelegateIvars;
  }

  unsafe impl NSObjectProtocol for WryWebViewDelegate {}

  unsafe impl WKScriptMessageHandler for WryWebViewDelegate {
    // Function for ipc handler
    #[method(userContentController:didReceiveScriptMessage:)]
    fn did_receive(
      this: &WryWebViewDelegate,
      _controller: &WKUserContentController,
      msg: &WKScriptMessage,
    ) {
      // Safety: objc runtime calls are unsafe
      unsafe {
        #[cfg(feature = "tracing")]
        let _span = tracing::info_span!(parent: None, "wry::ipc::handle").entered();

        let ipc_handler = &this.ivars().ipc_handler;
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
            ipc_handler(Request::builder().uri(url).body(js.to_string()).unwrap());
            return;
          }
        }

        #[cfg(feature = "tracing")]
        tracing::warn!("WebView received invalid IPC call.");
      }
    }
  }
);

impl WryWebViewDelegate {
  fn new(
    controller: Retained<WKUserContentController>,
    ipc_handler: Box<dyn Fn(Request<String>)>,
    mtm: MainThreadMarker,
  ) -> Retained<Self> {
    let delegate = mtm
      .alloc::<WryWebViewDelegate>()
      .set_ivars(WryWebViewDelegateIvars {
        ipc_handler,
        controller,
      });

    let delegate: Retained<Self> = unsafe { msg_send_id![super(delegate), init] };

    let proto_delegate = ProtocolObject::from_ref(delegate.as_ref());
    unsafe {
      // this will increate the retain count of the delegate
      delegate.ivars().controller.addScriptMessageHandler_name(
        proto_delegate,
        &NSString::from_str(IPC_MESSAGE_HANDLER_NAME),
      );
    }

    delegate
  }
}

struct DocumentTitleChangedObserverIvars {
  object: Retained<WryWebView>,
  handler: Box<dyn Fn(String)>,
}

declare_class!(
  struct DocumentTitleChangedObserver;

  unsafe impl ClassType for DocumentTitleChangedObserver {
    type Super = NSObject;
    type Mutability = InteriorMutable;
    const NAME: &'static str = "DocumentTitleChangedObserver";
  }

  impl DeclaredClass for DocumentTitleChangedObserver {
    type Ivars = DocumentTitleChangedObserverIvars;
  }

  unsafe impl DocumentTitleChangedObserver {
    #[method(observeValueForKeyPath:ofObject:change:context:)]
    fn observe_value_for_key_path(
      &self,
      key_path: Option<&NSString>,
      of_object: Option<&AnyObject>,
      _change: Option<&NSDictionary<NSKeyValueChangeKey, AnyObject>>,
      _context: *mut c_void,
    ) {
      if let (Some(key_path), Some(object)) = (key_path, of_object) {
        if key_path.to_string() == "title" {
          unsafe {
            let handler = &self.ivars().handler;
            // if !handler.is_null() {
              let title: *const NSString = msg_send![object, title];
              handler((*title).to_string());
            // }
          }
        }
      }
    }
  }

  unsafe impl NSObjectProtocol for DocumentTitleChangedObserver {}
);

impl DocumentTitleChangedObserver {
  fn new(webview: Retained<WryWebView>, handler: Box<dyn Fn(String)>) -> Retained<Self> {
    let observer = Self::alloc().set_ivars(DocumentTitleChangedObserverIvars {
      object: webview,
      handler,
    });

    let observer: Retained<Self> = unsafe { msg_send_id![super(observer), init] };

    unsafe {
      observer
        .ivars()
        .object
        .addObserver_forKeyPath_options_context(
          &observer,
          &NSString::from_str("title"),
          NSKeyValueObservingOptions::NSKeyValueObservingOptionNew,
          null_mut(),
        );
    }

    observer
  }
}

impl Drop for DocumentTitleChangedObserver {
  fn drop(&mut self) {
    unsafe {
      self
        .ivars()
        .object
        .removeObserver_forKeyPath(&self, &NSString::from_str("title"));
    }
  }
}

pub struct WryNavigationDelegateIvars {
  pending_scripts: Arc<Mutex<Option<Vec<String>>>>,
  has_download_handler: bool,
  navigation_policy_function: Box<dyn Fn(String, bool) -> bool>,
  download_delegate: Option<Retained<WryDownloadDelegate>>,
  on_page_load_handler: Option<Box<dyn Fn(PageLoadEvent)>>,
}

declare_class!(
  pub struct WryNavigationDelegate;

  unsafe impl ClassType for WryNavigationDelegate {
    type Super = NSObject;
    type Mutability = MainThreadOnly;
    const NAME: &'static str = "WryNavigationDelegate";
  }

  impl DeclaredClass for WryNavigationDelegate {
    type Ivars = WryNavigationDelegateIvars;
  }

  unsafe impl NSObjectProtocol for WryNavigationDelegate {}

  unsafe impl WKNavigationDelegate for WryNavigationDelegate {
    #[method(webView:decidePolicyForNavigationAction:decisionHandler:)]
    fn navigation_policy(
      &self,
      webview: &WKWebView,
      action: &WKNavigationAction,
      handler: &block2::Block<dyn Fn(WKNavigationActionPolicy)>,
    ) {
      navigation_policy(self, webview, action, handler);
    }

    #[method(webView:decidePolicyForNavigationResponse:decisionHandler:)]
    fn navigation_policy_response(
      &self,
      webview: &WKWebView,
      response: &WKNavigationResponse,
      handler: &block2::Block<dyn Fn(WKNavigationResponsePolicy)>,
    ) {
      navigation_policy_response(self, webview, response, handler);
    }

    #[method(webView:didFinishNavigation:)]
    fn did_finish_navigation(
      &self,
      webview: &WKWebView,
      navigation: &WKNavigation,
    ) {
      did_finish_navigation(self, webview, navigation);
    }

    #[method(webView:didCommitNavigation:)]
    fn did_commit_navigation(
      &self,
      webview: &WKWebView,
      navigation: &WKNavigation,
    ) {
      did_commit_navigation(self, webview, navigation);
    }

    #[method(webView:navigationAction:didBecomeDownload:)]
    fn navigation_download_action(
      &self,
      webview: &WKWebView,
      action: &WKNavigationAction,
      download: &WKDownload,
    ) {
      navigation_download_action(self, webview, action, download);
    }

    #[method(webView:navigationResponse:didBecomeDownload:)]
    fn navigation_download_response(
      &self,
      webview: &WKWebView,
      response: &WKNavigationResponse,
      download: &WKDownload,
    ) {
      navigation_download_response(self, webview, response, download);
    }
  }
);

impl WryNavigationDelegate {
  fn new(
    webview: Retained<WryWebView>,
    pending_scripts: Arc<Mutex<Option<Vec<String>>>>,
    has_download_handler: bool,
    navigation_handler: Option<Box<dyn Fn(String) -> bool>>,
    new_window_req_handler: Option<Box<dyn Fn(String) -> bool>>,
    download_delegate: Option<Retained<WryDownloadDelegate>>,
    on_page_load_handler: Option<Box<dyn Fn(PageLoadEvent, String)>>,
    mtm: MainThreadMarker,
  ) -> Retained<Self> {
    let navigation_policy_function = Box::new(move |url: String, is_main_frame: bool| -> bool {
      if is_main_frame {
        navigation_handler
          .as_ref()
          .map_or(true, |navigation_handler| (navigation_handler)(url))
      } else {
        new_window_req_handler
          .as_ref()
          .map_or(true, |new_window_req_handler| (new_window_req_handler)(url))
      }
    });

    let on_page_load_handler = if let Some(handler) = on_page_load_handler {
      let custom_handler = Box::new(move |event| {
        handler(event, url_from_webview(&webview).unwrap_or_default());
      }) as Box<dyn Fn(PageLoadEvent)>;
      Some(custom_handler)
    } else {
      None
    };

    let delegate = mtm
      .alloc::<WryNavigationDelegate>()
      .set_ivars(WryNavigationDelegateIvars {
        pending_scripts,
        navigation_policy_function,
        has_download_handler,
        download_delegate,
        on_page_load_handler,
      });

    unsafe { msg_send_id![super(delegate), init] }
  }
}

pub struct WryDownloadDelegateIvars {
  started: *mut Box<dyn FnMut(String, &mut PathBuf) -> bool>,
  completed: *mut Rc<dyn Fn(String, Option<PathBuf>, bool)>,
}

declare_class!(
  pub struct WryDownloadDelegate;

  unsafe impl ClassType for WryDownloadDelegate {
    type Super = NSObject;
    type Mutability = MainThreadOnly;
    const NAME: &'static str = "WryDownloadDelegate";
  }

  impl DeclaredClass for WryDownloadDelegate {
    type Ivars = WryDownloadDelegateIvars;
  }

  unsafe impl NSObjectProtocol for WryDownloadDelegate {}

  unsafe impl WKDownloadDelegate for WryDownloadDelegate {
    #[method(download:decideDestinationUsingResponse:suggestedFilename:completionHandler:)]
    fn download_policy(
      &self,
      download: &WKDownload,
      response: &NSURLResponse,
      suggested_path: &NSString,
      handler: &block2::Block<dyn Fn(*const NSURL)>,
    ) {
      download_policy(self, download, response, suggested_path, handler);
    }

    #[method(downloadDidFinish:)]
    fn download_did_finish(&self, download: &WKDownload) {
      download_did_finish(self, download);
    }

    #[method(download:didFailWithError:resumeData:)]
    fn download_did_fail(
      &self,
      download: &WKDownload,
      error: &NSError,
      resume_data: &NSData,
    ) {
      download_did_fail(self, download, error, resume_data);
    }
  }
);

impl WryDownloadDelegate {
  fn new(
    download_started_handler: Option<Box<dyn FnMut(String, &mut PathBuf) -> bool>>,
    download_completed_handler: Option<Rc<dyn Fn(String, Option<PathBuf>, bool)>>,
    mtm: MainThreadMarker,
  ) -> Retained<Self> {
    let started = match download_started_handler {
      Some(handler) => Box::into_raw(Box::new(handler)),
      None => null_mut(),
    };
    let completed = match download_completed_handler {
      Some(handler) => Box::into_raw(Box::new(handler)),
      None => null_mut(),
    };
    let delegate = mtm
      .alloc::<WryDownloadDelegate>()
      .set_ivars(WryDownloadDelegateIvars { started, completed });

    unsafe { msg_send_id![super(delegate), init] }
  }
}

impl Drop for WryDownloadDelegate {
  fn drop(&mut self) {
    if self.ivars().started != null_mut() {
      unsafe {
        drop(Box::from_raw(self.ivars().started));
      }
    }
    if self.ivars().completed != null_mut() {
      unsafe {
        drop(Box::from_raw(self.ivars().completed));
      }
    }
  }
}
