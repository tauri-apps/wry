// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

mod file_drop;

use crate::{
  webview::{WebContext, WebViewAttributes},
  Error, Result,
};

use file_drop::FileDropController;

use std::{collections::HashSet, mem::MaybeUninit, rc::Rc, sync::mpsc};

use once_cell::unsync::OnceCell;

use windows::{
  core::{Interface, PCWSTR, PWSTR},
  Win32::{
    Foundation::{BOOL, E_FAIL, E_POINTER, FARPROC, HWND, POINT, RECT},
    System::{
      Com::{IStream, StructuredStorage::CreateStreamOnHGlobal},
      LibraryLoader::{GetProcAddress, LoadLibraryA},
      SystemInformation::OSVERSIONINFOW,
      WinRT::EventRegistrationToken,
    },
    UI::WindowsAndMessaging::{
      self as win32wm, DestroyWindow, GetClientRect, GetCursorPos, WM_NCLBUTTONDOWN,
    },
  },
};

use webview2_com::{Microsoft::Web::WebView2::Win32::*, *};

use crate::{
  application::{platform::windows::WindowExtWindows, window::Window},
  http::RequestBuilder as HttpRequestBuilder,
};

impl From<webview2_com::Error> for Error {
  fn from(err: webview2_com::Error) -> Self {
    Error::WebView2Error(err)
  }
}

pub struct InnerWebView {
  pub(crate) controller: ICoreWebView2Controller,
  webview: ICoreWebView2,
  // Store FileDropController in here to make sure it gets dropped when
  // the webview gets dropped, otherwise we'll have a memory leak
  #[allow(dead_code)]
  file_drop_controller: Rc<OnceCell<FileDropController>>,
}

impl InnerWebView {
  pub fn new(
    window: Rc<Window>,
    mut attributes: WebViewAttributes,
    web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    let hwnd = HWND(window.hwnd() as _);
    let file_drop_controller: Rc<OnceCell<FileDropController>> = Rc::new(OnceCell::new());
    let file_drop_handler = attributes.file_drop_handler.take();
    let file_drop_window = window.clone();

    let env = Self::create_environment(&web_context)?;
    let controller = Self::create_controller(hwnd, &env)?;
    let webview = Self::init_webview(window, hwnd, attributes, &env, &controller)?;

    if let Some(file_drop_handler) = file_drop_handler {
      let mut controller = FileDropController::new();
      controller.listen(hwnd, file_drop_window, file_drop_handler);
      let _ = file_drop_controller.set(controller);
    }

    Ok(Self {
      controller,
      webview,
      file_drop_controller,
    })
  }

  fn create_environment(
    web_context: &Option<&mut WebContext>,
  ) -> webview2_com::Result<ICoreWebView2Environment> {
    let (tx, rx) = mpsc::channel();

    let data_directory = web_context
      .as_deref()
      .and_then(|context| context.data_directory())
      .and_then(|path| path.to_str())
      .map(String::from);

    CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
      Box::new(move |environmentcreatedhandler| unsafe {
        let options: ICoreWebView2EnvironmentOptions =
          CoreWebView2EnvironmentOptions::default().into();

        // remove "mini menu" - See https://github.com/tauri-apps/wry/issues/535
        let _ = options.SetAdditionalBrowserArguments("--disable-features=msWebOOUI,msPdfOOUI");

        if let Some(data_directory) = data_directory {
          CreateCoreWebView2EnvironmentWithOptions(
            PCWSTR::default(),
            data_directory,
            options,
            environmentcreatedhandler,
          )
        } else {
          CreateCoreWebView2EnvironmentWithOptions(
            PCWSTR::default(),
            PCWSTR::default(),
            options,
            environmentcreatedhandler,
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
  ) -> webview2_com::Result<ICoreWebView2Controller> {
    let (tx, rx) = mpsc::channel();
    let env = env.clone();

    CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
      Box::new(move |handler| unsafe {
        env
          .CreateCoreWebView2Controller(hwnd, handler)
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
    window: Rc<Window>,
    hwnd: HWND,
    mut attributes: WebViewAttributes,
    env: &ICoreWebView2Environment,
    controller: &ICoreWebView2Controller,
  ) -> webview2_com::Result<ICoreWebView2> {
    let webview =
      unsafe { controller.CoreWebView2() }.map_err(webview2_com::Error::WindowsError)?;

    // Transparent
    if attributes.transparent && !is_windows_7() {
      let controller2: ICoreWebView2Controller2 = controller
        .cast()
        .map_err(webview2_com::Error::WindowsError)?;
      unsafe {
        controller2
          .SetDefaultBackgroundColor(COREWEBVIEW2_COLOR {
            R: 0,
            G: 0,
            B: 0,
            A: 0,
          })
          .map_err(webview2_com::Error::WindowsError)?;
      }
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
          if DestroyWindow(hwnd).as_bool() {
            Ok(())
          } else {
            Err(E_FAIL.into())
          }
        }));
      webview
        .add_WindowCloseRequested(handler, &mut token)
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
        .SetAreDevToolsEnabled(false)
        .map_err(webview2_com::Error::WindowsError)?;
      if attributes.devtools {
        settings
          .SetAreDevToolsEnabled(true)
          .map_err(webview2_com::Error::WindowsError)?;
      }

      let settings5 = settings.cast::<ICoreWebView2Settings5>()?;
      let _ = settings5.SetIsPinchZoomEnabled(attributes.zoom_hotkeys_enabled);

      let mut rect = RECT::default();
      GetClientRect(hwnd, &mut rect);
      controller
        .SetBounds(rect)
        .map_err(webview2_com::Error::WindowsError)?;
    }

    // Initialize scripts
    Self::add_script_to_execute_on_document_created(
      &webview,
      String::from(
        r#"Object.defineProperty(window, 'ipc', {
  value: Object.freeze({postMessage:s=>window.chrome.webview.postMessage(s)})
});

window.addEventListener('mousedown', (e) => {
  if (e.buttons === 1) window.chrome.webview.postMessage('__WEBVIEW_LEFT_MOUSE_DOWN__')
});
window.addEventListener('mousemove', (e) => window.chrome.webview.postMessage('__WEBVIEW_MOUSE_MOVE__'));"#,
      ),
    )?;
    for js in attributes.initialization_scripts {
      Self::add_script_to_execute_on_document_created(&webview, js)?;
    }

    // Message handler
    let ipc_handler = attributes.ipc_handler.take();
    unsafe {
      webview.add_WebMessageReceived(
        WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
          if let Some(args) = args {
            let mut js = PWSTR::default();
            args.TryGetWebMessageAsString(&mut js)?;
            let js = take_pwstr(js);
            if js == "__WEBVIEW_LEFT_MOUSE_DOWN__" || js == "__WEBVIEW_MOUSE_MOVE__" {
              if !window.is_decorated() && window.is_resizable() && !window.is_maximized() {
                use crate::application::{platform::windows::hit_test, window::CursorIcon};

                let mut point = POINT::default();
                GetCursorPos(&mut point);
                let result = hit_test(HWND(window.hwnd() as _), point.x, point.y);
                let cursor = match result.0 as u32 {
                  win32wm::HTLEFT => CursorIcon::WResize,
                  win32wm::HTTOP => CursorIcon::NResize,
                  win32wm::HTRIGHT => CursorIcon::EResize,
                  win32wm::HTBOTTOM => CursorIcon::SResize,
                  win32wm::HTTOPLEFT => CursorIcon::NwResize,
                  win32wm::HTTOPRIGHT => CursorIcon::NeResize,
                  win32wm::HTBOTTOMLEFT => CursorIcon::SwResize,
                  win32wm::HTBOTTOMRIGHT => CursorIcon::SeResize,
                  _ => CursorIcon::Arrow,
                };
                // don't use `CursorIcon::Arrow` variant or cursor manipulation using css will cause cursor flickering
                if cursor != CursorIcon::Arrow {
                  window.set_cursor_icon(cursor);
                }

                if js == "__WEBVIEW_LEFT_MOUSE_DOWN__" {
                  // we ignore `HTCLIENT` variant so the webview receives the click correctly if it is not on the edges
                  // and prevent conflict with `tao::window::drag_window`.
                  if result.0 as u32 != win32wm::HTCLIENT {
                    window.begin_resize_drag(result.0, WM_NCLBUTTONDOWN, point.x, point.y);
                  }
                }
              }
              // these are internal messages, ipc_handlers don't need it so exit early
              return Ok(());
            }

            if let Some(ipc_handler) = &ipc_handler {
              ipc_handler(&window, js);
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
            NavigationStartingEventHandler::create(Box::new(move |_, args| {
              if let Some(args) = args {
                let mut uri = PWSTR::default();
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

    let mut custom_protocol_names = HashSet::new();
    if !attributes.custom_protocols.is_empty() {
      for (name, _) in &attributes.custom_protocols {
        // WebView2 doesn't support non-standard protocols yet, so we have to use this workaround
        // See https://github.com/MicrosoftEdge/WebView2Feedback/issues/73
        custom_protocol_names.insert(name.clone());
        unsafe {
          webview.AddWebResourceRequestedFilter(
            format!("https://{}.*", name),
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
          )
        }
        .map_err(webview2_com::Error::WindowsError)?;
      }

      let custom_protocols = attributes.custom_protocols;
      let env = env.clone();
      unsafe {
        webview
          .add_WebResourceRequested(
            WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
              if let Some(args) = args {
                let webview_request = args.Request()?;
                let mut request = HttpRequestBuilder::new();

                // request method (GET, POST, PUT etc..)
                let mut request_method = PWSTR::default();
                webview_request.Method(&mut request_method)?;
                let request_method = take_pwstr(request_method);

                // get all headers from the request
                let headers = webview_request.Headers()?.GetIterator()?;
                let mut has_current = BOOL::default();
                headers.HasCurrentHeader(&mut has_current)?;
                if has_current.as_bool() {
                  loop {
                    let mut key = PWSTR::default();
                    let mut value = PWSTR::default();
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
                        &mut cb_read,
                      )
                      .ok()?;

                    if cb_read == 0 {
                      break;
                    }

                    body_sent.extend_from_slice(&buffer[..(cb_read as usize)]);
                  }
                }

                // uri
                let mut uri = PWSTR::default();
                webview_request.Uri(&mut uri)?;
                let uri = take_pwstr(uri);

                if let Some(custom_protocol) = custom_protocols
                  .iter()
                  .find(|(name, _)| uri.starts_with(&format!("https://{}.", name)))
                {
                  // Undo the protocol workaround when giving path to resolver
                  let path = uri.replace(
                    &format!("https://{}.", custom_protocol.0),
                    &format!("{}://", custom_protocol.0),
                  );
                  let final_request = request
                    .uri(&path)
                    .method(request_method.as_str())
                    .body(body_sent)
                    .unwrap();

                  return match (custom_protocol.1)(&final_request) {
                    Ok(sent_response) => {
                      let content = sent_response.body();
                      let status_code = sent_response.status().as_u16() as i32;

                      let mut headers_map = String::new();

                      // set mime type if provided
                      if let Some(mime) = sent_response.mimetype() {
                        headers_map.push_str(&format!("Content-Type: {}\n", mime))
                      }

                      // build headers
                      for (name, value) in sent_response.headers().iter() {
                        let header_key = name.to_string();
                        if let Ok(value) = value.to_str() {
                          headers_map.push_str(&format!("{}: {}\n", header_key, value))
                        }
                      }

                      let mut body_sent = None;
                      if !content.is_empty() {
                        let stream = CreateStreamOnHGlobal(0, true)?;
                        stream.SetSize(content.len() as u64)?;
                        let mut cb_write = MaybeUninit::uninit();
                        if stream
                          .Write(
                            content.as_ptr() as *const _,
                            content.len() as u32,
                            cb_write.as_mut_ptr(),
                          )
                          .is_ok()
                          && cb_write.assume_init() as usize == content.len()
                        {
                          body_sent = Some(stream);
                        }
                      }

                      // FIXME: Set http response version

                      let body_sent = body_sent.map(|content| content.cast().unwrap());
                      let response =
                        env.CreateWebResourceResponse(body_sent, status_code, "OK", headers_map)?;

                      args.SetResponse(response)?;
                      Ok(())
                    }
                    Err(_) => Err(E_FAIL.into()),
                  };
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
            PermissionRequestedEventHandler::create(Box::new(|_, args| {
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
        settings.SetUserAgent(String::from(user_agent.as_str()))?;
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
              .NavigateToString(path.to_string())
              .map_err(webview2_com::Error::WindowsError)?;
          }
        }
      } else {
        let mut url_string = String::from(url.as_str());
        let name = url.scheme();
        if custom_protocol_names.contains(name) {
          // WebView2 doesn't support non-standard protocols yet, so we have to use this workaround
          // See https://github.com/MicrosoftEdge/WebView2Feedback/issues/73
          url_string = url
            .as_str()
            .replace(&format!("{}://", name), &format!("https://{}.", name))
        }
        unsafe {
          webview
            .Navigate(url_string)
            .map_err(webview2_com::Error::WindowsError)?;
        }
      }
    } else if let Some(html) = attributes.html {
      unsafe {
        webview
          .NavigateToString(html)
          .map_err(webview2_com::Error::WindowsError)?;
      }
    }

    unsafe {
      controller
        .SetIsVisible(true)
        .map_err(webview2_com::Error::WindowsError)?;
      controller
        .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC)
        .map_err(webview2_com::Error::WindowsError)?;
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
          .AddScriptToExecuteOnDocumentCreated(js, handler)
          .map_err(webview2_com::Error::WindowsError)
      }),
      Box::new(|_, _| Ok(())),
    )
  }

  fn execute_script(webview: &ICoreWebView2, js: String) -> windows::core::Result<()> {
    unsafe {
      webview.ExecuteScript(
        js,
        ExecuteScriptCompletedHandler::create(Box::new(|_, _| (Ok(())))),
      )
    }
  }

  pub fn print(&self) {
    let _ = self.eval("window.print()");
  }

  pub fn eval(&self, js: &str) -> Result<()> {
    Self::execute_script(&self.webview, js.to_string())
      .map_err(|err| Error::WebView2Error(webview2_com::Error::WindowsError(err)))
  }

  pub fn resize(&self, hwnd: HWND) -> Result<()> {
    // Safety: System calls are unsafe
    // XXX: Resizing on Windows is usually sluggish. Many other applications share same behavior.
    unsafe {
      let mut rect = RECT::default();
      GetClientRect(hwnd, &mut rect);
      self.controller.SetBounds(rect)
    }
    .map_err(|error| Error::WebView2Error(webview2_com::Error::WindowsError(error)))
  }

  pub fn focus(&self) {
    let _ = unsafe {
      self
        .controller
        .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC)
    };
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
}

pub fn platform_webview_version() -> Result<String> {
  let mut versioninfo = PWSTR::default();
  unsafe { GetAvailableCoreWebView2BrowserVersionString(PCWSTR::default(), &mut versioninfo) }
    .map_err(webview2_com::Error::WindowsError)?;
  Ok(take_pwstr(versioninfo))
}

fn is_windows_7() -> bool {
  if let Some(v) = get_windows_ver() {
    // windows 7 is 6.1
    if v.0 == 6 && v.1 == 1 {
      return true;
    }
  }
  false
}

fn get_function_impl(library: &str, function: &str) -> Option<FARPROC> {
  assert_eq!(library.chars().last(), Some('\0'));
  assert_eq!(function.chars().last(), Some('\0'));

  let module = unsafe { LoadLibraryA(library) }.unwrap_or_default();
  if module.is_invalid() {
    None
  } else {
    Some(unsafe { GetProcAddress(module, function) })
  }
}

macro_rules! get_function {
  ($lib:expr, $func:ident) => {
    get_function_impl(concat!($lib, '\0'), concat!(stringify!($func), '\0'))
      .map(|f| unsafe { std::mem::transmute::<windows::Win32::Foundation::FARPROC, $func>(f) })
  };
}

/// Returns a tuple of (major, minor, buildnumber)
fn get_windows_ver() -> Option<(u32, u32, u32)> {
  type RtlGetVersion = unsafe extern "system" fn(*mut OSVERSIONINFOW) -> i32;
  let handle = get_function!("ntdll.dll", RtlGetVersion);
  if let Some(rtl_get_version) = handle {
    unsafe {
      let mut vi = OSVERSIONINFOW {
        dwOSVersionInfoSize: 0,
        dwMajorVersion: 0,
        dwMinorVersion: 0,
        dwBuildNumber: 0,
        dwPlatformId: 0,
        szCSDVersion: [0; 128],
      };

      let status = (rtl_get_version)(&mut vi as _);

      if status >= 0 {
        Some((vi.dwMajorVersion, vi.dwMinorVersion, vi.dwBuildNumber))
      } else {
        None
      }
    }
  } else {
    None
  }
}
