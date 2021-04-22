// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

mod file_drop;

use windows_webview2::{
  Microsoft::Web::WebView2::Core as webview2,
  Windows::{
    Foundation::*,
    Storage::Streams::*,
    Win32::{
      DisplayDevices::RECT,
      WindowsAndMessaging::{self, HWND},
    },
  },
};

use crate::{
  webview::{mimetype::MimeType, FileDropEvent, RpcRequest, RpcResponse},
  Result,
};

use file_drop::FileDropController;

use std::{
  collections::HashSet,
  path::PathBuf,
  rc::Rc,
  sync::mpsc::{self, RecvError},
};

use once_cell::unsync::OnceCell;
use url::Url;

use crate::application::{
  event_loop::{ControlFlow, EventLoop},
  platform::{run_return::EventLoopExtRunReturn, windows::WindowExtWindows},
  window::Window,
};

pub struct InnerWebView {
  controller: Rc<OnceCell<webview2::CoreWebView2Controller>>,
  webview: Rc<OnceCell<webview2::CoreWebView2>>,

  // Store FileDropController in here to make sure it gets dropped when
  // the webview gets dropped, otherwise we'll have a memory leak
  #[allow(dead_code)]
  file_drop_controller: Rc<OnceCell<FileDropController>>,
}

impl InnerWebView {
  pub fn new(
    window: Rc<Window>,
    scripts: Vec<String>,
    url: Option<Url>,
    // TODO default background color option just adds to webview2 recently and it requires
    // canary build. Implement this once it's in official release.
    #[allow(unused_variables)] transparent: bool,
    custom_protocols: Vec<(
      String,
      Box<dyn Fn(&Window, &str) -> Result<Vec<u8>> + 'static>,
    )>,
    rpc_handler: Option<Box<dyn Fn(&Window, RpcRequest) -> Option<RpcResponse>>>,
    file_drop_handler: Option<Box<dyn Fn(&Window, FileDropEvent) -> bool>>,
    data_directory: Option<PathBuf>,
  ) -> Result<Self> {
    let hwnd = HWND(window.hwnd() as _);

    let controller_rc: Rc<OnceCell<webview2::CoreWebView2Controller>> = Rc::new(OnceCell::new());
    let webview_rc: Rc<OnceCell<webview2::CoreWebView2>> = Rc::new(OnceCell::new());
    let file_drop_controller_rc: Rc<OnceCell<FileDropController>> = Rc::new(OnceCell::new());

    let env = wait_for_async_operation(match data_directory {
      Some(data_directory_provided) => webview2::CoreWebView2Environment::CreateWithOptionsAsync(
        "",
        data_directory_provided.to_str().unwrap_or(""),
        webview2::CoreWebView2EnvironmentOptions::new()?,
      )?,
      None => webview2::CoreWebView2Environment::CreateAsync()?,
    })?;

    // Webview controller
    let controller = wait_for_async_operation(env.CreateCoreWebView2ControllerAsync(
      webview2::CoreWebView2ControllerWindowReference::CreateFromWindowHandle(hwnd.0 as _)?,
    )?)?;
    let w = controller.CoreWebView2()?;
    // Enable sensible defaults
    let settings = w.Settings()?;
    settings.SetIsStatusBarEnabled(false)?;
    settings.SetAreDefaultContextMenusEnabled(true)?;
    settings.SetIsZoomControlEnabled(false)?;
    settings.SetAreDevToolsEnabled(false)?;
    debug_assert_eq!(settings.SetAreDevToolsEnabled(true)?, ());

    // Safety: System calls are unsafe
    unsafe {
      let mut rect = RECT::default();
      WindowsAndMessaging::GetClientRect(hwnd, &mut rect);
      let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
      controller.SetBounds(Rect {
        X: 0f32,
        Y: 0f32,
        Width: width as f32,
        Height: height as f32,
      })?;
    }

    // Initialize scripts
    wait_for_async_operation(w.AddScriptToExecuteOnDocumentCreatedAsync(
      "window.external={invoke:s=>window.chrome.webview.postMessage(s)}",
    )?)?;
    for js in scripts {
      wait_for_async_operation(w.AddScriptToExecuteOnDocumentCreatedAsync(js.as_str())?)?;
    }

    // Message handler
    let window_ = window.clone();
    w.WebMessageReceived(TypedEventHandler::<
      webview2::CoreWebView2,
      webview2::CoreWebView2WebMessageReceivedEventArgs,
    >::new(move |webview, args| {
      if let (Some(webview), Some(args)) = (webview, args) {
        if let (Ok(js), Some(rpc_handler)) = (
          String::from_utf16(args.TryGetWebMessageAsString()?.as_wide()),
          rpc_handler.as_ref(),
        ) {
          match super::rpc_proxy(&window_, js, rpc_handler) {
            Ok(result) => {
              if let Some(ref script) = result {
                let _ = webview.ExecuteScriptAsync(script.as_str())?;
              }
            }
            Err(e) => {
              eprintln!("{}", e);
            }
          }
        }
      }
      Ok(())
    }))?;

    let mut custom_protocol_names = HashSet::new();
    for (name, function) in custom_protocols {
      // WebView2 doesn't support non-standard protocols yet, so we have to use this workaround
      // See https://github.com/MicrosoftEdge/WebView2Feedback/issues/73
      custom_protocol_names.insert(name.clone());
      w.AddWebResourceRequestedFilter(
        format!("https://custom-protocol-{}*", name).as_str(),
        webview2::CoreWebView2WebResourceContext::All,
      )?;
      let env_ = env.clone();
      let window_ = window.clone();

      w.WebResourceRequested(TypedEventHandler::<
        webview2::CoreWebView2,
        webview2::CoreWebView2WebResourceRequestedEventArgs,
      >::new(move |_, args| {
        if let Some(args) = args {
          if let Ok(uri) = String::from_utf16(args.Request()?.Uri()?.as_wide()) {
            // Undo the protocol workaround when giving path to resolver
            let path = uri.replace(
              &format!("https://custom-protocol-{}", name),
              &format!("{}://", name),
            );

            if let Ok(content) = function(&window_, &path) {
              let mime = MimeType::parse(&content, &uri);
              let stream = InMemoryRandomAccessStream::new()?;
              let writer = DataWriter::CreateDataWriter(stream.clone())?;
              writer.WriteBytes(&content)?;
              writer.DetachStream()?;
              let response = env_.CreateWebResourceResponse(
                stream,
                200,
                "OK",
                format!("Content-Type: {}", mime).as_str(),
              )?;
              args.SetResponse(response)?;
            }
          }
        }

        Ok(())
      }))?;
    }

    // Enable clipboard
    w.PermissionRequested(TypedEventHandler::<
      webview2::CoreWebView2,
      webview2::CoreWebView2PermissionRequestedEventArgs,
    >::new(|_, args| {
      if let Some(args) = args {
        if args.PermissionKind()? == webview2::CoreWebView2PermissionKind::ClipboardRead {
          args.SetState(webview2::CoreWebView2PermissionState::Allow)?
        }
      }
      Ok(())
    }))?;

    // Navigation
    if let Some(url) = url {
      if url.cannot_be_a_base() {
        let s = url.as_str();
        if let Some(pos) = s.find(',') {
          let (_, path) = s.split_at(pos + 1);
          w.NavigateToString(path)?;
        }
      } else {
        let mut url_string = String::from(url.as_str());
        let name = url.scheme();
        if custom_protocol_names.contains(name) {
          // WebView2 doesn't support non-standard protocols yet, so we have to use this workaround
          // See https://github.com/MicrosoftEdge/WebView2Feedback/issues/73
          url_string = url.as_str().replace(
            &format!("{}://", name),
            &format!("https://custom-protocol-{}", name),
          )
        }
        w.Navigate(url_string.as_str())?;
      }
    }

    controller.SetIsVisible(true)?;

    let _ = controller_rc.set(controller).expect("set the controller");
    let _ = webview_rc.set(w).expect("set the webview");

    if let Some(file_drop_handler) = file_drop_handler {
      let mut file_drop_controller = FileDropController::new();
      file_drop_controller.listen(hwnd, window, file_drop_handler);
      let _ = file_drop_controller_rc.set(file_drop_controller);
    }

    Ok(Self {
      controller: controller_rc,
      webview: webview_rc,
      file_drop_controller: file_drop_controller_rc,
    })
  }

  pub fn eval(&self, js: &str) -> Result<()> {
    if let Some(w) = self.webview.get() {
      let _ = w.ExecuteScriptAsync(js)?;
    }
    Ok(())
  }

  pub fn resize(&self, hwnd: HWND) -> Result<()> {
    // Safety: System calls are unsafe
    unsafe {
      let mut rect = RECT::default();
      WindowsAndMessaging::GetClientRect(hwnd, &mut rect);
      if let Some(c) = self.controller.get() {
        let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
        c.SetBounds(Rect {
          X: 0f32,
          Y: 0f32,
          Width: width as f32,
          Height: height as f32,
        })?;
      }
    }

    Ok(())
  }
}

/// The WebView2 threading model runs everything on the UI thread, including callbacks which it triggers
/// with `PostMessage`, and we're using this here because it's waiting for some async operations in WebView2
/// to finish before starting the main message loop in `EventLoop::run`. As long as there are no pending
/// results in `rx`, it will poll the [`EventLoop`] with [`EventLoopExtRunReturn::run_return`] and check for a
/// result after each message is dispatched.
fn wait_for_async_operation<T>(op: IAsyncOperation<T>) -> Result<T>
where
  T: windows::RuntimeType,
{
  let (tx, rx) = mpsc::channel();
  op.SetCompleted(AsyncOperationCompletedHandler::new(move |op, _status| {
    if let Some(op) = op {
      tx.send(op.GetResults()?).expect("send over mpsc channel");
    }
    Ok(())
  }))?;

  let mut result = Err(RecvError.into());
  let mut event_loop = EventLoop::new();
  event_loop.run_return(|_, _, control_flow| {
    if let Ok(value) = rx.try_recv() {
      *control_flow = ControlFlow::Exit;
      result = Ok(value);
    } else {
      *control_flow = ControlFlow::Poll;
    }
  });

  result
}
