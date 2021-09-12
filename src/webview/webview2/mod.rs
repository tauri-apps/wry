// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

mod file_drop;

use crate::{
  webview::{WebContext, WebViewAttributes},
  Error, Result,
};

use file_drop::FileDropController;

use std::{collections::HashSet, rc::Rc, sync::mpsc};

use once_cell::unsync::OnceCell;
use webview2_com::{
  Microsoft::Web::WebView2::Win32::*,
  WebMessageReceivedEventHandler, WindowCloseRequestedEventHandler,
  Windows::Win32::{
    Foundation::{BOOL, E_FAIL, E_POINTER, HWND, POINT, PWSTR, RECT},
    Storage::StructuredStorage::CreateStreamOnHGlobal,
    UI::WindowsAndMessaging::{
      DestroyWindow, GetClientRect, GetCursorPos, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCLIENT,
      HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT,
    },
  },
  *,
};
use windows::Interface;

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
    let hwnd = window.hwnd() as HWND;

    let file_drop_controller: Rc<OnceCell<FileDropController>> = Rc::new(OnceCell::new());
    let file_drop_controller_clone = file_drop_controller.clone();

    let env = {
      let (tx, rx) = mpsc::channel();

      let data_directory = web_context
        .and_then(|context| context.data_directory())
        .and_then(|path| path.to_str())
        .and_then(|path| Some(String::from(path)));

      CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
        Box::new(move |environmentcreatedhandler| unsafe {
          if let Some(data_directory) = data_directory {
            // If we have a custom data_directory, we need to use a call to `CreateCoreWebView2EnvironmentWithOptions`
            // instead of the much simpler `CreateCoreWebView2Environment`.
            let options: ICoreWebView2EnvironmentOptions =
              CoreWebView2EnvironmentOptions::default().into();
            let data_directory = pwstr_from_str(&data_directory);
            let result = CreateCoreWebView2EnvironmentWithOptions(
              PWSTR::default(),
              data_directory,
              options,
              environmentcreatedhandler,
            )
            .map_err(webview2_com::Error::WindowsError);
            let _ = take_pwstr(data_directory);

            return result;
          }

          CreateCoreWebView2Environment(environmentcreatedhandler)
            .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |error_code, environment| {
          error_code?;
          tx.send(environment.ok_or_else(|| windows::Error::fast_error(E_POINTER)))
            .expect("send over mpsc channel");
          Ok(())
        }),
      )?;

      rx.recv()
        .map_err(|_| webview2_com::Error::SendError)?
        .map_err(webview2_com::Error::WindowsError)
    }?;

    let controller = {
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
          tx.send(controller.ok_or_else(|| windows::Error::fast_error(E_POINTER)))
            .expect("send over mpsc channel");
          Ok(())
        }),
      )?;

      rx.recv()
        .map_err(|_| webview2_com::Error::SendError)?
        .map_err(webview2_com::Error::WindowsError)
    }?;

    let webview =
      unsafe { controller.get_CoreWebView2() }.map_err(webview2_com::Error::WindowsError)?;

    // Transparent
    if attributes.transparent {
      if let Ok(c2) = controller.cast::<ICoreWebView2Controller2>() {
        unsafe {
          c2.put_DefaultBackgroundColor(COREWEBVIEW2_COLOR {
            R: 0,
            G: 0,
            B: 0,
            A: 0,
          })
          .map_err(webview2_com::Error::WindowsError)?;
        }
      }
    }

    let mut token = Windows::Win32::System::WinRT::EventRegistrationToken::default();

    // Safety: System calls are unsafe
    unsafe {
      let hwnd = HWND(hwnd.0);
      let handler: ICoreWebView2WindowCloseRequestedEventHandler =
        WindowCloseRequestedEventHandler::create(Box::new(move |_, _| {
          if DestroyWindow(hwnd).as_bool() {
            Ok(())
          } else {
            Err(E_FAIL.into())
          }
        }))
        .into();
      webview
        .add_WindowCloseRequested(handler, &mut token)
        .map_err(webview2_com::Error::WindowsError)?;

      let settings = webview
        .get_Settings()
        .map_err(webview2_com::Error::WindowsError)?;
      settings
        .put_IsStatusBarEnabled(false)
        .map_err(webview2_com::Error::WindowsError)?;
      settings
        .put_AreDefaultContextMenusEnabled(true)
        .map_err(webview2_com::Error::WindowsError)?;
      settings
        .put_IsZoomControlEnabled(false)
        .map_err(webview2_com::Error::WindowsError)?;
      settings
        .put_AreDevToolsEnabled(false)
        .map_err(webview2_com::Error::WindowsError)?;
      debug_assert_eq!(settings.put_AreDevToolsEnabled(true), Ok(()));

      let mut rect = RECT::default();
      GetClientRect(hwnd, &mut rect);
      controller
        .put_Bounds(rect)
        .map_err(webview2_com::Error::WindowsError)?;
    }

    // Initialize scripts
    let handler_webview = webview.clone();
    AddScriptToExecuteOnDocumentCreatedCompletedHandler::wait_for_async_operation(
      Box::new(move |handler| unsafe {
        handler_webview.AddScriptToExecuteOnDocumentCreated(
          r#"
            window.external={invoke:s=>window.chrome.webview.postMessage(s)};

            window.addEventListener('mousedown', (e) => {
              if (e.buttons === 1) window.chrome.webview.postMessage('__WEBVIEW_LEFT_MOUSE_DOWN__')
            });
            window.addEventListener('mousemove', () => window.chrome.webview.postMessage('__WEBVIEW_MOUSE_MOVE__'));
          "#,
          handler
        ).map_err(webview2_com::Error::WindowsError)
      }),
      Box::new(|_, _| Ok(())),
    )?;
    for js in attributes.initialization_scripts {
      let handler_webview = webview.clone();
      AddScriptToExecuteOnDocumentCreatedCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
          handler_webview
            .AddScriptToExecuteOnDocumentCreated(js, handler)
            .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(|_, _| Ok(())),
      )?;
    }

    // Message handler
    let window_ = window.clone();

    let rpc_handler = attributes.rpc_handler.take();
    unsafe {
      webview.add_WebMessageReceived(
        WebMessageReceivedEventHandler::create(Box::new(move |webview, args| {
          if let (Some(webview), Some(args)) = (webview, args) {
            let mut js = PWSTR::default();
            args.TryGetWebMessageAsString(&mut js)?;
            let js = take_pwstr(js);
            if js == "__WEBVIEW_LEFT_MOUSE_DOWN__" || js == "__WEBVIEW_MOUSE_MOVE__" {
              if !window_.is_decorated() && window_.is_resizable() {
                use crate::application::{platform::windows::hit_test, window::CursorIcon};

                let mut point = POINT::default();
                GetCursorPos(&mut point);
                let result = hit_test(window_.hwnd() as _, point.x, point.y);
                let cursor = match result.0 as u32 {
                  HTLEFT => CursorIcon::WResize,
                  HTTOP => CursorIcon::NResize,
                  HTRIGHT => CursorIcon::EResize,
                  HTBOTTOM => CursorIcon::SResize,
                  HTTOPLEFT => CursorIcon::NwResize,
                  HTTOPRIGHT => CursorIcon::NeResize,
                  HTBOTTOMLEFT => CursorIcon::SwResize,
                  HTBOTTOMRIGHT => CursorIcon::SeResize,
                  _ => CursorIcon::Arrow,
                };
                // don't use `CursorIcon::Arrow` variant or cursor manipulation using css will cause cursor flickering
                if cursor != CursorIcon::Arrow {
                  window_.set_cursor_icon(cursor);
                }

                if js == "__WEBVIEW_LEFT_MOUSE_DOWN__" {
                  // we ignore `HTCLIENT` variant so the webview receives the click correctly if it is not on the edges
                  // and prevent conflict with `tao::window::drag_window`.
                  if result.0 as u32 != HTCLIENT {
                    window_.begin_resize_drag(result.0 as isize);
                  }
                }
              }
              // these are internal messages, rpc_handlers don't need it so exit early
              return Ok(());
            }

            if let Some(rpc_handler) = &rpc_handler {
              match super::rpc_proxy(&window_, js, rpc_handler) {
                Ok(result) => {
                  if let Some(script) = result {
                    webview.ExecuteScript(
                      script,
                      ExecuteScriptCompletedHandler::create(Box::new(|_, _| (Ok(())))),
                    )?;
                  }
                }
                Err(e) => {
                  eprintln!("{}", e);
                }
              }
            }
          }

          Ok(())
        })),
        &mut token,
      )
    }
    .map_err(webview2_com::Error::WindowsError)?;

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
                let webview_request = args.get_Request()?;
                let mut request = HttpRequestBuilder::new();

                // request method (GET, POST, PUT etc..)
                let mut request_method = PWSTR::default();
                webview_request.get_Method(&mut request_method)?;
                let request_method = take_pwstr(request_method);

                // get all headers from the request
                let headers = webview_request.get_Headers()?.GetIterator()?;
                loop {
                  let mut has_current = BOOL::default();
                  headers.get_HasCurrentHeader(&mut has_current)?;
                  if !has_current.as_bool() {
                    break;
                  }

                  let mut key = PWSTR::default();
                  let mut value = PWSTR::default();
                  headers.GetCurrentHeader(&mut key, &mut value)?;
                  let (key, value) = (take_pwstr(key), take_pwstr(value));
                  request = request.header(&key, &value);
                }

                // get the body content if available
                let content = webview_request.get_Content()?;
                let mut buffer: [u8; 1024] = [0; 1024];
                let mut body_sent = Vec::new();
                loop {
                  let mut cb_read = 0;
                  content.Read(
                    buffer.as_mut_ptr() as *mut _,
                    buffer.len() as u32,
                    &mut cb_read,
                  )?;

                  if cb_read == 0 {
                    break;
                  }

                  body_sent.extend_from_slice(&buffer[..(cb_read as usize)]);
                }

                // uri
                let mut uri = PWSTR::default();
                webview_request.get_Uri(&mut uri)?;
                let uri = take_pwstr(uri);

                // Undo the protocol workaround when giving path to resolver
                let path = uri.replace("https://", "").replacen(".", "://", 1);

                let scheme = path.split("://").next().unwrap();

                let final_request = request
                  .uri(&path)
                  .method(request_method.as_str())
                  .body(body_sent)
                  .unwrap();
                match (custom_protocols
                  .iter()
                  .find(|(name, _)| name == &scheme)
                  .unwrap()
                  .1)(&final_request)
                {
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

                    let stream = CreateStreamOnHGlobal(0, true)?;
                    if stream.Write(content.as_ptr() as *const _, content.len() as u32)? as usize
                      != content.len()
                    {
                      return Err(E_FAIL.into());
                    }

                    // FIXME: Set http response version

                    let response =
                      env.CreateWebResourceResponse(stream, status_code, "OK", headers_map)?;

                    args.put_Response(response)?;
                    Ok(())
                  }
                  Err(_) => Err(E_FAIL.into()),
                }
              } else {
                Ok(())
              }
            })),
            &mut token,
          )
          .map_err(webview2_com::Error::WindowsError)?;
      }
    }

    // Enable clipboard
    unsafe {
      webview
        .add_PermissionRequested(
          PermissionRequestedEventHandler::create(Box::new(|_, args| {
            if let Some(args) = args {
              let mut kind = COREWEBVIEW2_PERMISSION_KIND_UNKNOWN_PERMISSION;
              args.get_PermissionKind(&mut kind)?;
              if kind == COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ {
                args.put_State(COREWEBVIEW2_PERMISSION_STATE_ALLOW)?;
              }
            }
            Ok(())
          })),
          &mut token,
        )
        .map_err(webview2_com::Error::WindowsError)?;
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
          .NavigateToString(html.to_string())
          .map_err(webview2_com::Error::WindowsError)?;
      }
    }

    unsafe {
      controller
        .put_IsVisible(true)
        .map_err(webview2_com::Error::WindowsError)?;
      controller
        .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC)
        .map_err(webview2_com::Error::WindowsError)?;

      if let Some(file_drop_handler) = attributes.file_drop_handler {
        let mut file_drop_controller = FileDropController::new();
        file_drop_controller.listen(hwnd, window.clone(), file_drop_handler);
        let _ = file_drop_controller_clone.set(file_drop_controller);
      }
    }

    Ok(Self {
      controller,
      webview,
      file_drop_controller,
    })
  }

  pub fn print(&self) {
    let _ = self.eval("window.print()");
  }

  pub fn eval(&self, js: &str) -> Result<()> {
    unsafe {
      self.webview.ExecuteScript(
        js.to_string(),
        ExecuteScriptCompletedHandler::create(Box::new(|_, _| (Ok(())))),
      )
    }
    .map_err(|error| Error::WebView2Error(webview2_com::Error::WindowsError(error)))
  }

  pub fn resize(&self, hwnd: HWND) -> Result<()> {
    // Safety: System calls are unsafe
    // XXX: Resizing on Windows is usually sluggish. Many other applications share same behavior.
    unsafe {
      let mut rect = RECT::default();
      GetClientRect(hwnd, &mut rect);
      self.controller.put_Bounds(rect)
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
}

pub fn platform_webview_version() -> Result<String> {
  let mut versioninfo = PWSTR::default();
  unsafe { GetAvailableCoreWebView2BrowserVersionString(PWSTR::default(), &mut versioninfo) }
    .map_err(webview2_com::Error::WindowsError)?;
  Ok(take_pwstr(versioninfo))
}
