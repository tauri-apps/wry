// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

mod file_drop;

use std::{
  borrow::Cow, collections::HashSet, fmt::Write, iter::once, os::windows::prelude::OsStrExt,
  path::PathBuf, rc::Rc, sync::mpsc,
};

use http::{Request, Response as HttpResponse, StatusCode};
use once_cell::sync::Lazy;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use url::Url;
use webview2_com::{Microsoft::Web::WebView2::Win32::*, *};
use windows::{
  core::{s, ComInterface, PCWSTR, PWSTR},
  Win32::{
    Foundation::*,
    Globalization::{self, MAX_LOCALE_NAME},
    Graphics::Gdi::{MapWindowPoints, RedrawWindow, HBRUSH, HRGN, RDW_INTERNALPAINT},
    System::{
      Com::{CoInitializeEx, IStream, COINIT_APARTMENTTHREADED},
      LibraryLoader::GetModuleHandleW,
      WinRT::EventRegistrationToken,
    },
    UI::{
      Shell::{DefSubclassProc, SHCreateMemStream, SetWindowSubclass},
      WindowsAndMessaging::{
        self as win32wm, CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetParent,
        PostMessageW, RegisterClassExW, RegisterWindowMessageA, SetWindowPos, ShowWindow,
        CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, HCURSOR, HICON, HMENU, SWP_ASYNCWINDOWPOS,
        SWP_NOACTIVATE, SWP_NOZORDER, SW_HIDE, SW_SHOW, WINDOW_EX_STYLE, WNDCLASSEXW, WS_CHILD,
        WS_CLIPCHILDREN, WS_VISIBLE,
      },
    },
  },
};

use self::file_drop::FileDropController;
use super::Theme;
use crate::{
  proxy::ProxyConfig, Error, MemoryUsageLevel, PageLoadEvent, Rect, RequestAsyncResponder, Result,
  WebContext, WebViewAttributes, RGBA,
};

impl From<webview2_com::Error> for Error {
  fn from(err: webview2_com::Error) -> Self {
    Error::WebView2Error(err)
  }
}

pub(crate) struct InnerWebView {
  hwnd: HWND,
  is_child: bool,
  pub controller: ICoreWebView2Controller,
  webview: ICoreWebView2,
  env: ICoreWebView2Environment,
  // Store FileDropController in here to make sure it gets dropped when
  // the webview gets dropped, otherwise we'll have a memory leak
  #[allow(dead_code)]
  file_drop_controller: Option<FileDropController>,
}

impl Drop for InnerWebView {
  fn drop(&mut self) {
    let _ = unsafe { self.controller.Close() };
    if self.is_child {
      let _ = unsafe { DestroyWindow(self.hwnd) };
    }
  }
}

impl InnerWebView {
  pub fn new(
    window: &impl HasRawWindowHandle,
    attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    let window = match window.raw_window_handle() {
      RawWindowHandle::Win32(window) => window.hwnd as _,
      _ => return Err(Error::UnsupportedWindowHandle),
    };
    Self::new_hwnd(HWND(window), attributes, pl_attrs, web_context)
  }

  pub fn new_as_child(
    parent: &impl HasRawWindowHandle,
    attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    let parent = match parent.raw_window_handle() {
      RawWindowHandle::Win32(parent) => parent.hwnd as _,
      _ => return Err(Error::UnsupportedWindowHandle),
    };

    let class_name = encode_wide("WRY_WEBVIEW");

    unsafe extern "system" fn default_window_proc(
      hwnd: HWND,
      msg: u32,
      wparam: WPARAM,
      lparam: LPARAM,
    ) -> LRESULT {
      DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    let class = WNDCLASSEXW {
      cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
      style: CS_HREDRAW | CS_VREDRAW,
      lpfnWndProc: Some(default_window_proc),
      cbClsExtra: 0,
      cbWndExtra: 0,
      hInstance: unsafe { HINSTANCE(GetModuleHandleW(PCWSTR::null()).unwrap_or_default().0) },
      hIcon: HICON::default(),
      hCursor: HCURSOR::default(), // must be null in order for cursor state to work properly
      hbrBackground: HBRUSH::default(),
      lpszMenuName: PCWSTR::null(),
      lpszClassName: PCWSTR::from_raw(class_name.as_ptr()),
      hIconSm: HICON::default(),
    };

    unsafe { RegisterClassExW(&class) };

    let mut flags = WS_CHILD | WS_CLIPCHILDREN;
    if attributes.visible {
      flags |= WS_VISIBLE;
    }

    let child = unsafe {
      CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        PCWSTR::from_raw(class_name.as_ptr()),
        PCWSTR::null(),
        flags,
        attributes.bounds.map(|a| a.x).unwrap_or(CW_USEDEFAULT),
        attributes.bounds.map(|a| a.y).unwrap_or(CW_USEDEFAULT),
        attributes
          .bounds
          .map(|a| a.width as i32)
          .unwrap_or(CW_USEDEFAULT),
        attributes
          .bounds
          .map(|a| a.height as i32)
          .unwrap_or(CW_USEDEFAULT),
        HWND(parent),
        HMENU::default(),
        GetModuleHandleW(PCWSTR::null()).unwrap_or_default(),
        None,
      )
    };

    Self::new_as_child_hwnd(child, attributes, pl_attrs, web_context)
  }

  fn new_as_child_hwnd(
    hwnd: HWND,
    attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    Self::new_hwnd(hwnd, attributes, pl_attrs, web_context).map(|mut w| {
      w.is_child = true;
      w
    })
  }

  fn new_hwnd(
    hwnd: HWND,
    mut attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

    let file_drop_controller = attributes
      .file_drop_handler
      .take()
      .map(|handler| FileDropController::new(hwnd, handler));

    let env = Self::create_environment(&web_context, pl_attrs.clone(), &attributes)?;
    let controller = Self::create_controller(hwnd, &env, attributes.incognito)?;
    let webview = Self::init_webview(hwnd, attributes, &env, &controller, pl_attrs)?;

    Ok(Self {
      hwnd,
      controller,
      is_child: false,
      webview,
      env,
      file_drop_controller,
    })
  }

  fn create_environment(
    web_context: &Option<&mut WebContext>,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    attributes: &WebViewAttributes,
  ) -> webview2_com::Result<ICoreWebView2Environment> {
    let (tx, rx) = mpsc::channel();

    let data_directory = web_context
      .as_deref()
      .and_then(|context| context.data_directory())
      .and_then(|path| path.to_str())
      .map(String::from);

    let argument = PCWSTR::from_raw(
      encode_wide(pl_attrs.additional_browser_args.unwrap_or_else(|| {
        // remove "mini menu" - See https://github.com/tauri-apps/wry/issues/535
        // and "smart screen" - See https://github.com/tauri-apps/tauri/issues/1345
        format!(
          "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection{}{}",
          if attributes.autoplay {
            " --autoplay-policy=no-user-gesture-required"
          } else {
            ""
          },
          if let Some(proxy_setting) = &attributes.proxy_config {
            match proxy_setting {
              ProxyConfig::Http(endpoint) => {
                format!(" --proxy-server=http://{}:{}", endpoint.host, endpoint.port)
              }
              ProxyConfig::Socks5(endpoint) => format!(
                " --proxy-server=socks5://{}:{}",
                endpoint.host, endpoint.port
              ),
            }
          } else {
            "".to_string()
          }
        )
      }))
      .as_ptr(),
    );

    CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
      Box::new(move |environmentcreatedhandler| unsafe {
        let options = {
          let options: ICoreWebView2EnvironmentOptions =
            CoreWebView2EnvironmentOptions::default().into();

          // Get user's system language
          let lcid = Globalization::GetUserDefaultUILanguage();
          let mut lang = [0; MAX_LOCALE_NAME as usize];
          Globalization::LCIDToLocaleName(
            lcid as u32,
            Some(&mut lang),
            Globalization::LOCALE_ALLOW_NEUTRAL_NAMES,
          );

          options
            .SetLanguage(PCWSTR::from_raw(lang.as_ptr()))
            .map_err(webview2_com::Error::WindowsError)?;
          options
        };

        let _ = options.SetAdditionalBrowserArguments(argument);

        if let Some(data_directory) = data_directory {
          CreateCoreWebView2EnvironmentWithOptions(
            PCWSTR::null(),
            PCWSTR::from_raw(encode_wide(data_directory).as_ptr()),
            &options,
            &environmentcreatedhandler,
          )
        } else {
          CreateCoreWebView2EnvironmentWithOptions(
            PCWSTR::null(),
            PCWSTR::null(),
            &options,
            &environmentcreatedhandler,
          )
        }
        .map_err(webview2_com::Error::WindowsError)
      }),
      Box::new(move |error_code, environment| {
        error_code?;
        tx.send(environment.ok_or_else(|| windows::core::Error::from(E_POINTER)))
          .expect("send over mpsc channel");
        Ok(())
      }),
    )?;

    rx.recv()
      .map_err(|_| webview2_com::Error::SendError)?
      .map_err(webview2_com::Error::WindowsError)
  }

  fn create_controller(
    hwnd: HWND,
    env: &ICoreWebView2Environment,
    incognito: bool,
  ) -> webview2_com::Result<ICoreWebView2Controller> {
    let (tx, rx) = mpsc::channel();
    let env = env.clone().cast::<ICoreWebView2Environment10>()?;
    let controller_opts = unsafe { env.CreateCoreWebView2ControllerOptions()? };

    unsafe { controller_opts.SetIsInPrivateModeEnabled(incognito)? }

    CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
      Box::new(move |handler| unsafe {
        env
          .CreateCoreWebView2ControllerWithOptions(hwnd, &controller_opts, &handler)
          .map_err(webview2_com::Error::WindowsError)
      }),
      Box::new(move |error_code, controller| {
        error_code?;
        tx.send(controller.ok_or_else(|| windows::core::Error::from(E_POINTER)))
          .expect("send over mpsc channel");
        Ok(())
      }),
    )?;

    rx.recv()
      .map_err(|_| webview2_com::Error::SendError)?
      .map_err(webview2_com::Error::WindowsError)
  }

  fn init_webview(
    hwnd: HWND,
    mut attributes: WebViewAttributes,
    env: &ICoreWebView2Environment,
    controller: &ICoreWebView2Controller,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
  ) -> webview2_com::Result<ICoreWebView2> {
    let webview =
      unsafe { controller.CoreWebView2() }.map_err(webview2_com::Error::WindowsError)?;

    // theme
    if let Some(theme) = pl_attrs.theme {
      set_theme(&webview, theme);
    }

    // background color
    if !attributes.transparent {
      if let Some(background_color) = attributes.background_color {
        set_background_color(controller, background_color)?;
      }
    }

    // Transparent
    if attributes.transparent && !is_windows_7() {
      set_background_color(controller, (0, 0, 0, 0))?;
    }

    // The EventRegistrationToken is an out-param from all of the event registration calls. We're
    // taking it in the local variable and then just ignoring it because all of the event handlers
    // are registered for the life of the webview, but if we wanted to be able to remove them later
    // we would hold onto them in self.
    let mut token = EventRegistrationToken::default();

    // Safety: System calls are unsafe
    unsafe {
      let handler: ICoreWebView2WindowCloseRequestedEventHandler =
        WindowCloseRequestedEventHandler::create(Box::new(move |_, _| {
          win32wm::DestroyWindow(hwnd)
        }));
      webview
        .add_WindowCloseRequested(&handler, &mut token)
        .map_err(webview2_com::Error::WindowsError)?;

      let settings = webview
        .Settings()
        .map_err(webview2_com::Error::WindowsError)?;
      settings
        .SetIsStatusBarEnabled(false)
        .map_err(webview2_com::Error::WindowsError)?;
      settings
        .SetAreDefaultContextMenusEnabled(true)
        .map_err(webview2_com::Error::WindowsError)?;
      settings
        .SetIsZoomControlEnabled(attributes.zoom_hotkeys_enabled)
        .map_err(webview2_com::Error::WindowsError)?;
      settings
        .SetAreDevToolsEnabled(attributes.devtools)
        .map_err(webview2_com::Error::WindowsError)?;
      if !pl_attrs.browser_accelerator_keys {
        if let Ok(settings3) = settings.cast::<ICoreWebView2Settings3>() {
          settings3
            .SetAreBrowserAcceleratorKeysEnabled(false)
            .map_err(webview2_com::Error::WindowsError)?;
        }
      }

      let settings5 = settings.cast::<ICoreWebView2Settings5>()?;
      settings5
        .SetIsPinchZoomEnabled(attributes.zoom_hotkeys_enabled)
        .map_err(webview2_com::Error::WindowsError)?;

      let settings6 = settings.cast::<ICoreWebView2Settings6>()?;
      settings6
        .SetIsSwipeNavigationEnabled(attributes.back_forward_navigation_gestures)
        .map_err(webview2_com::Error::WindowsError)?;

      let mut rect = RECT::default();
      win32wm::GetClientRect(hwnd, &mut rect)?;
      controller
        .SetBounds(rect)
        .map_err(webview2_com::Error::WindowsError)?;
    }

    // document title changed handler
    if let Some(document_title_changed_handler) = attributes.document_title_changed_handler {
      unsafe {
        webview
          .add_DocumentTitleChanged(
            &DocumentTitleChangedEventHandler::create(Box::new(move |webview, _| {
              let mut title = PWSTR::null();
              if let Some(webview) = webview {
                webview.DocumentTitle(&mut title)?;
                let title = take_pwstr(title);
                document_title_changed_handler(title);
              }
              Ok(())
            })),
            &mut token,
          )
          .map_err(webview2_com::Error::WindowsError)?;
      }
    }

    if let Some(on_page_load_handler) = attributes.on_page_load_handler {
      let on_page_load_handler = Rc::new(on_page_load_handler);
      let on_page_load_handler_ = on_page_load_handler.clone();

      unsafe {
        webview
          .add_ContentLoading(
            &ContentLoadingEventHandler::create(Box::new(move |webview, _| {
              if let Some(webview) = webview {
                on_page_load_handler_(PageLoadEvent::Started, url_from_webview(&webview))
              }
              Ok(())
            })),
            &mut token,
          )
          .map_err(webview2_com::Error::WindowsError)?;
      }

      unsafe {
        webview
          .add_NavigationCompleted(
            &NavigationCompletedEventHandler::create(Box::new(move |webview, _| {
              if let Some(webview) = webview {
                on_page_load_handler(PageLoadEvent::Finished, url_from_webview(&webview))
              }
              Ok(())
            })),
            &mut token,
          )
          .map_err(webview2_com::Error::WindowsError)?;
      }
    }

    // Initialize scripts
    Self::add_script_to_execute_on_document_created(
      &webview,
      String::from(
        r#"Object.defineProperty(window, 'ipc', { value: Object.freeze({ postMessage: s=> window.chrome.webview.postMessage(s) }) });"#,
      ),
    )?;
    for js in attributes.initialization_scripts {
      Self::add_script_to_execute_on_document_created(&webview, js)?;
    }

    // Message handler
    let ipc_handler = attributes.ipc_handler.take();
    unsafe {
      webview.add_WebMessageReceived(
        &WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
          if let Some(args) = args {
            let mut js = PWSTR::null();
            args.TryGetWebMessageAsString(&mut js)?;
            let js = take_pwstr(js);
            if let Some(ipc_handler) = &ipc_handler {
              ipc_handler(js);
            }
          }

          Ok(())
        })),
        &mut token,
      )
    }
    .map_err(webview2_com::Error::WindowsError)?;

    if let Some(nav_callback) = attributes.navigation_handler {
      unsafe {
        webview
          .add_NavigationStarting(
            &NavigationStartingEventHandler::create(Box::new(move |_, args| {
              if let Some(args) = args {
                let mut uri = PWSTR::null();
                args.Uri(&mut uri)?;
                let uri = take_pwstr(uri);

                let allow = nav_callback(uri);

                args.SetCancel(!allow)?;
              }

              Ok(())
            })),
            &mut token,
          )
          .map_err(webview2_com::Error::WindowsError)?;
      }
    }

    if attributes.download_started_handler.is_some()
      || attributes.download_completed_handler.is_some()
    {
      unsafe {
        let webview4: ICoreWebView2_4 =
          webview.cast().map_err(webview2_com::Error::WindowsError)?;

        let mut download_started_handler = attributes.download_started_handler.take();
        let download_completed_handler = attributes.download_completed_handler.take();

        webview4
          .add_DownloadStarting(
            &DownloadStartingEventHandler::create(Box::new(move |_, args| {
              if let Some(args) = args {
                let mut uri = PWSTR::null();
                args.DownloadOperation()?.Uri(&mut uri)?;
                let uri = take_pwstr(uri);

                if let Some(download_completed_handler) = download_completed_handler.clone() {
                  args.DownloadOperation()?.add_StateChanged(
                    &StateChangedEventHandler::create(Box::new(move |download_operation, _| {
                      if let Some(download_operation) = download_operation {
                        let mut state: COREWEBVIEW2_DOWNLOAD_STATE =
                          COREWEBVIEW2_DOWNLOAD_STATE::default();
                        download_operation.State(&mut state)?;
                        if state != COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS {
                          let mut path = PWSTR::null();
                          download_operation.ResultFilePath(&mut path)?;
                          let path = take_pwstr(path);
                          let mut uri = PWSTR::null();
                          download_operation.Uri(&mut uri)?;
                          let uri = take_pwstr(uri);

                          let success = state == COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED;
                          download_completed_handler(
                            uri,
                            success.then(|| PathBuf::from(path)),
                            success,
                          );
                        }
                      }

                      Ok(())
                    })),
                    &mut token,
                  )?;
                }
                if let Some(download_started_handler) = download_started_handler.as_mut() {
                  let mut path = PWSTR::null();
                  args.ResultFilePath(&mut path)?;
                  let path = take_pwstr(path);
                  let mut path = PathBuf::from(&path);

                  if download_started_handler(uri, &mut path) {
                    let simplified = dunce::simplified(&path);
                    let result_file_path =
                      PCWSTR::from_raw(encode_wide(simplified.as_os_str()).as_ptr());
                    args.SetResultFilePath(result_file_path)?;
                    args.SetHandled(true)?;
                  } else {
                    args.SetCancel(true)?;
                  }
                }
              }

              Ok(())
            })),
            &mut token,
          )
          .map_err(webview2_com::Error::WindowsError)?;
      }
    }

    if let Some(new_window_req_handler) = attributes.new_window_req_handler {
      unsafe {
        webview
          .add_NewWindowRequested(
            &NewWindowRequestedEventHandler::create(Box::new(move |_, args| {
              if let Some(args) = args {
                let mut uri = PWSTR::null();
                args.Uri(&mut uri)?;
                let uri = take_pwstr(uri);

                let allow = new_window_req_handler(uri);

                args.SetHandled(!allow)?;
              }

              Ok(())
            })),
            &mut token,
          )
          .map_err(webview2_com::Error::WindowsError)?;
      }
    }

    let scheme = if pl_attrs.https_scheme {
      "https"
    } else {
      "http"
    };
    let mut custom_protocol_names = HashSet::new();
    if !attributes.custom_protocols.is_empty() {
      for (name, _) in &attributes.custom_protocols {
        // WebView2 supports non-standard protocols only on Windows 10+, so we have to use this workaround
        // See https://github.com/MicrosoftEdge/WebView2Feedback/issues/73
        custom_protocol_names.insert(name.clone());
        unsafe {
          webview.AddWebResourceRequestedFilter(
            PCWSTR::from_raw(encode_wide(format!("{scheme}://{name}.*")).as_ptr()),
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
          )
        }
        .map_err(webview2_com::Error::WindowsError)?;
      }

      let custom_protocols = attributes.custom_protocols;
      let env = env.clone();
      let main_thread_id = std::thread::current().id();

      unsafe {
        webview
          .add_WebResourceRequested(
            &WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
              if let Some(args) = args {
                let webview_request = args.Request()?;
                let mut request = Request::builder();

                // request method (GET, POST, PUT etc..)
                let mut request_method = PWSTR::null();
                webview_request.Method(&mut request_method)?;
                let request_method = take_pwstr(request_method);

                // get all headers from the request
                let headers = webview_request.Headers()?.GetIterator()?;
                let mut has_current = BOOL::default();
                headers.HasCurrentHeader(&mut has_current)?;
                if has_current.as_bool() {
                  loop {
                    let mut key = PWSTR::null();
                    let mut value = PWSTR::null();
                    headers.GetCurrentHeader(&mut key, &mut value)?;
                    let (key, value) = (take_pwstr(key), take_pwstr(value));
                    request = request.header(&key, &value);

                    headers.MoveNext(&mut has_current)?;
                    if !has_current.as_bool() {
                      break;
                    }
                  }
                }

                // get the body content if available
                let mut body_sent = Vec::new();
                if let Ok(content) = webview_request.Content() {
                  let mut buffer: [u8; 1024] = [0; 1024];
                  loop {
                    let mut cb_read = 0;
                    let content: IStream = content.cast()?;
                    content
                      .Read(
                        buffer.as_mut_ptr() as *mut _,
                        buffer.len() as u32,
                        Some(&mut cb_read),
                      )
                      .ok()?;

                    if cb_read == 0 {
                      break;
                    }

                    body_sent.extend_from_slice(&buffer[..(cb_read as usize)]);
                  }
                }

                // uri
                let mut uri = PWSTR::null();
                webview_request.Uri(&mut uri)?;
                let uri = take_pwstr(uri);

                if let Some(custom_protocol) = custom_protocols
                  .iter()
                  .find(|(name, _)| uri.starts_with(&format!("{scheme}://{name}.")))
                {
                  // Undo the protocol workaround when giving path to resolver
                  let path = uri.replace(
                    &format!("{scheme}://{}.", custom_protocol.0),
                    &format!("{}://", custom_protocol.0),
                  );

                  let final_request = match request
                    .uri(&path)
                    .method(request_method.as_str())
                    .body(body_sent)
                  {
                    Ok(req) => req,
                    Err(_) => return Err(E_FAIL.into()),
                  };

                  let env = env.clone();
                  let deferral = args.GetDeferral();

                  let responder: Box<dyn FnOnce(HttpResponse<Cow<'static, [u8]>>)> =
                    Box::new(move |sent_response| {
                      let handler = move || {
                        match prepare_web_request_response(&env, &sent_response) {
                          Ok(response) => {
                            let _ = args.SetResponse(&response);
                          }
                          Err(_) => {
                            let status = StatusCode::BAD_REQUEST;
                            if let Ok(res) = env.CreateWebResourceResponse(
                              None,
                              status.as_u16() as i32,
                              PCWSTR::from_raw(
                                encode_wide(status.canonical_reason().unwrap_or("")).as_ptr(),
                              ),
                              PCWSTR::from_raw(encode_wide(String::new()).as_ptr()),
                            ) {
                              let _ = args.SetResponse(&res);
                            }
                          }
                        }

                        if let Ok(deferral) = &deferral {
                          let _ = deferral.Complete();
                        }
                      };

                      if std::thread::current().id() == main_thread_id {
                        handler();
                      } else {
                        dispatch_handler(hwnd, handler);
                      }
                    });

                  (custom_protocol.1)(final_request, RequestAsyncResponder { responder });
                  return Ok(());
                }
              }

              Ok(())
            })),
            &mut token,
          )
          .map_err(webview2_com::Error::WindowsError)?;
      }
    }

    // Enable clipboard
    if attributes.clipboard {
      unsafe {
        webview
          .add_PermissionRequested(
            &PermissionRequestedEventHandler::create(Box::new(|_, args| {
              if let Some(args) = args {
                let mut kind = COREWEBVIEW2_PERMISSION_KIND_UNKNOWN_PERMISSION;
                args.PermissionKind(&mut kind)?;
                if kind == COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ {
                  args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW)?;
                }
              }
              Ok(())
            })),
            &mut token,
          )
          .map_err(webview2_com::Error::WindowsError)?;
      }
    }

    // Set user agent
    if let Some(user_agent) = attributes.user_agent {
      unsafe {
        let settings: ICoreWebView2Settings2 = webview
          .Settings()?
          .cast()
          .map_err(webview2_com::Error::WindowsError)?;
        settings.SetUserAgent(PCWSTR::from_raw(encode_wide(user_agent).as_ptr()))?;
      }
    }

    // Navigation
    if let Some(url) = attributes.url {
      if url.cannot_be_a_base() {
        let s = url.as_str();
        if let Some(pos) = s.find(',') {
          let (_, path) = s.split_at(pos + 1);
          unsafe {
            webview
              .NavigateToString(PCWSTR::from_raw(encode_wide(path).as_ptr()))
              .map_err(webview2_com::Error::WindowsError)?;
          }
        }
      } else {
        let mut url_string = String::from(url.as_str());
        let name = url.scheme();
        if custom_protocol_names.contains(name) {
          // WebView2 supports non-standard protocols only on Windows 10+, so we have to use this workaround
          // See https://github.com/MicrosoftEdge/WebView2Feedback/issues/73
          url_string = url
            .as_str()
            .replace(&format!("{}://", name), &format!("{scheme}://{name}."))
        }

        if let Some(headers) = attributes.headers {
          load_url_with_headers(&webview, env, &url_string, headers);
        } else {
          unsafe {
            webview
              .Navigate(PCWSTR::from_raw(encode_wide(url_string).as_ptr()))
              .map_err(webview2_com::Error::WindowsError)?;
          }
        }
      }
    } else if let Some(html) = attributes.html {
      unsafe {
        webview
          .NavigateToString(PCWSTR::from_raw(encode_wide(html).as_ptr()))
          .map_err(webview2_com::Error::WindowsError)?;
      }
    }

    unsafe extern "system" fn subclass_proc(
      hwnd: HWND,
      msg: u32,
      wparam: WPARAM,
      lparam: LPARAM,
      _uidsubclass: usize,
      dwrefdata: usize,
    ) -> LRESULT {
      match msg {
        win32wm::WM_SIZE => {
          if wparam.0 != win32wm::SIZE_MINIMIZED as usize {
            let controller = dwrefdata as *mut ICoreWebView2Controller;
            let mut client_rect = RECT::default();
            let _ = win32wm::GetClientRect(hwnd, &mut client_rect);
            let _ = (*controller).SetBounds(RECT {
              left: 0,
              top: 0,
              right: client_rect.right - client_rect.left,
              bottom: client_rect.bottom - client_rect.top,
            });
          }
        }

        win32wm::WM_SETFOCUS | win32wm::WM_ENTERSIZEMOVE => {
          let controller = dwrefdata as *mut ICoreWebView2Controller;
          let _ = (*controller).MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
        }

        win32wm::WM_WINDOWPOSCHANGED => {
          let controller = dwrefdata as *mut ICoreWebView2Controller;
          let _ = (*controller).NotifyParentWindowPositionChanged();
        }

        win32wm::WM_DESTROY => {
          drop(Box::from_raw(dwrefdata as *mut ICoreWebView2Controller));
        }

        _ if msg == *EXEC_MSG_ID => {
          let mut function: Box<Box<dyn FnMut()>> = Box::from_raw(wparam.0 as *mut _);
          function();
          RedrawWindow(hwnd, None, HRGN::default(), RDW_INTERNALPAINT);
          return LRESULT(0);
        }

        _ => (),
      }

      DefSubclassProc(hwnd, msg, wparam, lparam)
    }
    unsafe {
      SetWindowSubclass(
        hwnd,
        Some(subclass_proc),
        8080,
        Box::into_raw(Box::new(controller.clone())) as _,
      );
    }

    unsafe {
      controller
        .SetIsVisible(attributes.visible)
        .map_err(webview2_com::Error::WindowsError)?;
      if attributes.focused {
        controller
          .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC)
          .map_err(webview2_com::Error::WindowsError)?;
      }
    }

    Ok(webview)
  }

  fn add_script_to_execute_on_document_created(
    webview: &ICoreWebView2,
    js: String,
  ) -> webview2_com::Result<()> {
    let handler_webview = webview.clone();
    AddScriptToExecuteOnDocumentCreatedCompletedHandler::wait_for_async_operation(
      Box::new(move |handler| unsafe {
        handler_webview
          .AddScriptToExecuteOnDocumentCreated(PCWSTR::from_raw(encode_wide(js).as_ptr()), &handler)
          .map_err(webview2_com::Error::WindowsError)
      }),
      Box::new(|_, _| Ok(())),
    )
  }

  fn execute_script(
    webview: &ICoreWebView2,
    js: String,
    callback: impl FnOnce(String) + Send + 'static,
  ) -> windows::core::Result<()> {
    unsafe {
      webview.ExecuteScript(
        PCWSTR::from_raw(encode_wide(js).as_ptr()),
        &ExecuteScriptCompletedHandler::create(Box::new(|_, return_str| {
          callback(return_str);
          Ok(())
        })),
      )
    }
  }

  pub fn print(&self) {
    let _ = self.eval(
      "window.print()",
      None::<Box<dyn FnOnce(String) + Send + 'static>>,
    );
  }

  pub fn url(&self) -> Url {
    Url::parse(&url_from_webview(&self.webview)).unwrap()
  }

  pub fn eval(
    &self,
    js: &str,
    callback: Option<impl FnOnce(String) + Send + 'static>,
  ) -> Result<()> {
    match callback {
      Some(callback) => Self::execute_script(&self.webview, js.to_string(), callback)
        .map_err(|err| Error::WebView2Error(webview2_com::Error::WindowsError(err))),
      None => Self::execute_script(&self.webview, js.to_string(), |_| ())
        .map_err(|err| Error::WebView2Error(webview2_com::Error::WindowsError(err))),
    }
  }

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn open_devtools(&self) {
    let _ = unsafe { self.webview.OpenDevToolsWindow() };
  }

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn close_devtools(&self) {}

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn is_devtools_open(&self) -> bool {
    false
  }

  pub fn zoom(&self, scale_factor: f64) {
    let _ = unsafe { self.controller.SetZoomFactor(scale_factor) };
  }

  pub fn set_background_color(&self, background_color: RGBA) -> Result<()> {
    set_background_color(&self.controller, background_color).map_err(Into::into)
  }

  pub fn load_url(&self, url: &str) {
    let url = encode_wide(url);
    let _ = unsafe { self.webview.Navigate(PCWSTR::from_raw(url.as_ptr())) };
  }

  pub fn load_url_with_headers(&self, url: &str, headers: http::HeaderMap) {
    load_url_with_headers(&self.webview, &self.env, url, headers);
  }

  pub fn clear_all_browsing_data(&self) -> Result<()> {
    let handler = ClearBrowsingDataCompletedHandler::create(Box::new(move |_| Ok(())));
    unsafe {
      self
        .webview
        .cast::<ICoreWebView2_13>()
        .map_err(|e| Error::WebView2Error(webview2_com::Error::WindowsError(e)))?
        .Profile()
        .map_err(|e| Error::WebView2Error(webview2_com::Error::WindowsError(e)))?
        .cast::<ICoreWebView2Profile2>()
        .map_err(|e| Error::WebView2Error(webview2_com::Error::WindowsError(e)))?
        .ClearBrowsingDataAll(&handler)
        .map_err(|e| Error::WebView2Error(webview2_com::Error::WindowsError(e)))
    }
  }

  pub fn set_theme(&self, theme: Theme) {
    set_theme(&self.webview, theme);
  }

  pub fn bounds(&self) -> Rect {
    let mut bounds = Rect::default();

    if self.is_child {
      let mut rect = RECT::default();
      if unsafe { GetClientRect(self.hwnd, &mut rect) }.is_err() {
        panic!("Unexpected GetClientRect failure")
      }

      let position_point = &mut [POINT {
        x: rect.left,
        y: rect.top,
      }];
      unsafe { MapWindowPoints(self.hwnd, GetParent(self.hwnd), position_point) };

      bounds.x = position_point[0].x;
      bounds.y = position_point[0].y;
      bounds.width = (rect.right - rect.left) as u32;
      bounds.height = (rect.bottom - rect.top) as u32;
    } else {
      let mut rect = RECT::default();
      if unsafe { self.controller.Bounds(&mut rect) }.is_ok() {
        bounds.width = (rect.right - rect.left) as u32;
        bounds.height = (rect.bottom - rect.top) as u32;
      }
    }

    bounds
  }

  pub fn set_bounds(&self, bounds: Rect) {
    if self.is_child {
      unsafe {
        let _ = SetWindowPos(
          self.hwnd,
          HWND::default(),
          bounds.x,
          bounds.y,
          bounds.width as i32,
          bounds.height as i32,
          SWP_ASYNCWINDOWPOS | SWP_NOACTIVATE | SWP_NOZORDER,
        );
      }
    }
  }

  pub fn set_visible(&self, visible: bool) {
    unsafe {
      if self.is_child {
        ShowWindow(
          self.hwnd,
          match visible {
            true => SW_SHOW,
            false => SW_HIDE,
          },
        );
      }

      let _ = self.controller.SetIsVisible(visible);
    }
  }

  pub fn focus(&self) {
    unsafe {
      let _ = self
        .controller
        .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
    }
  }

  pub fn set_memory_usage_level(&self, level: MemoryUsageLevel) {
    let Ok(webview) = self.webview.cast::<ICoreWebView2_19>() else {
      return;
    };
    // https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2memoryusagetargetlevel
    let level = match level {
      MemoryUsageLevel::Normal => 0,
      MemoryUsageLevel::Low => 1,
    };
    let level = COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL(level);
    let _ = unsafe { webview.SetMemoryUsageTargetLevel(level) };
  }
}

unsafe fn prepare_web_request_response(
  env: &ICoreWebView2Environment,
  sent_response: &HttpResponse<Cow<'static, [u8]>>,
) -> windows::core::Result<ICoreWebView2WebResourceResponse> {
  let content = sent_response.body();
  let status_code = sent_response.status();

  let mut headers_map = String::new();

  // build headers
  for (name, value) in sent_response.headers().iter() {
    let header_key = name.to_string();
    if let Ok(value) = value.to_str() {
      let _ = writeln!(headers_map, "{}: {}", header_key, value);
    }
  }

  let mut stream = None;
  if !content.is_empty() {
    stream = SHCreateMemStream(Some(content));
  }

  // FIXME: Set http response version

  env.CreateWebResourceResponse(
    stream.as_ref(),
    status_code.as_u16() as i32,
    PCWSTR::from_raw(encode_wide(status_code.canonical_reason().unwrap_or("OK")).as_ptr()),
    PCWSTR::from_raw(encode_wide(headers_map).as_ptr()),
  )
}

fn encode_wide(string: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
  string.as_ref().encode_wide().chain(once(0)).collect()
}

fn load_url_with_headers(
  webview: &ICoreWebView2,
  env: &ICoreWebView2Environment,
  url: &str,
  headers: http::HeaderMap,
) {
  let url = encode_wide(url);

  let headers_map = {
    let mut headers_map = String::new();
    for (name, value) in headers.iter() {
      let header_key = name.to_string();
      if let Ok(value) = value.to_str() {
        let _ = writeln!(headers_map, "{}: {}", header_key, value);
      }
    }
    encode_wide(headers_map)
  };

  unsafe {
    let env = env.cast::<ICoreWebView2Environment9>().unwrap();

    if let Ok(request) = env.CreateWebResourceRequest(
      PCWSTR::from_raw(url.as_ptr()),
      PCWSTR::from_raw(encode_wide("GET").as_ptr()),
      None,
      PCWSTR::from_raw(headers_map.as_ptr()),
    ) {
      let webview: ICoreWebView2_10 = webview.cast().unwrap();
      let _ = webview.NavigateWithWebResourceRequest(&request);
    }
  };
}

pub fn set_background_color(
  controller: &ICoreWebView2Controller,
  background_color: RGBA,
) -> webview2_com::Result<()> {
  let mut color = background_color;
  if is_windows_7() || color.3 != 0 {
    color.3 = 255;
  }

  let controller2: ICoreWebView2Controller2 = controller
    .cast()
    .map_err(webview2_com::Error::WindowsError)?;
  unsafe {
    controller2
      .SetDefaultBackgroundColor(COREWEBVIEW2_COLOR {
        R: color.0,
        G: color.1,
        B: color.2,
        A: color.3,
      })
      .map_err(webview2_com::Error::WindowsError)
  }
}

fn set_theme(webview: &ICoreWebView2, theme: Theme) {
  unsafe {
    let _ = webview
      .cast::<ICoreWebView2_13>()
      .unwrap()
      .Profile()
      .unwrap()
      .SetPreferredColorScheme(match theme {
        Theme::Dark => COREWEBVIEW2_PREFERRED_COLOR_SCHEME_DARK,
        Theme::Light => COREWEBVIEW2_PREFERRED_COLOR_SCHEME_LIGHT,
        Theme::Auto => COREWEBVIEW2_PREFERRED_COLOR_SCHEME_AUTO,
      });
  }
}

pub fn platform_webview_version() -> Result<String> {
  let mut versioninfo = PWSTR::null();
  unsafe { GetAvailableCoreWebView2BrowserVersionString(PCWSTR::null(), &mut versioninfo) }
    .map_err(webview2_com::Error::WindowsError)?;
  Ok(take_pwstr(versioninfo))
}

fn is_windows_7() -> bool {
  let v = windows_version::OsVersion::current();
  // windows 7 is 6.1
  v.major == 6 && v.minor == 1
}

fn url_from_webview(webview: &ICoreWebView2) -> String {
  let mut pwstr = PWSTR::null();
  unsafe { webview.Source(&mut pwstr).unwrap() };
  take_pwstr(pwstr)
}

static EXEC_MSG_ID: Lazy<u32> = Lazy::new(|| unsafe { RegisterWindowMessageA(s!("Wry::ExecMsg")) });

unsafe fn dispatch_handler<F>(hwnd: HWND, function: F)
where
  F: FnMut() + 'static,
{
  // We double-box because the first box is a fat pointer.
  let boxed = Box::new(function) as Box<dyn FnMut()>;
  let boxed2: Box<Box<dyn FnMut()>> = Box::new(boxed);

  let raw = Box::into_raw(boxed2);

  let res = PostMessageW(hwnd, *EXEC_MSG_ID, WPARAM(raw as _), LPARAM(0));
  assert!(
    res.is_ok(),
    "PostMessage failed ; is the messages queue full?"
  );
}
