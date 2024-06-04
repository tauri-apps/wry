// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

mod drag_drop;
mod util;

use std::{
  borrow::Cow, cell::RefCell, collections::HashSet, fmt::Write, path::PathBuf, rc::Rc, sync::mpsc,
};

use dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use http::{Request, Response as HttpResponse, StatusCode};
use once_cell::sync::Lazy;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use webview2_com::{Microsoft::Web::WebView2::Win32::*, *};
use windows::{
  core::{s, w, Interface, HSTRING, PCWSTR, PWSTR},
  Win32::{
    Foundation::*,
    Globalization::*,
    Graphics::Gdi::*,
    System::{Com::*, LibraryLoader::GetModuleHandleW, WinRT::EventRegistrationToken},
    UI::{Shell::*, WindowsAndMessaging::*},
  },
};

use self::drag_drop::DragDropController;
use super::Theme;
use crate::{
  proxy::ProxyConfig, Error, MemoryUsageLevel, PageLoadEvent, Rect, RequestAsyncResponder, Result,
  WebContext, WebViewAttributes, RGBA,
};

const PARENT_SUBCLASS_ID: u32 = WM_USER + 0x64;
const PARENT_DESTROY_MESSAGE: u32 = WM_USER + 0x65;
const MAIN_THREAD_DISPATCHER_SUBCLASS_ID: u32 = WM_USER + 0x66;
static EXEC_MSG_ID: Lazy<u32> = Lazy::new(|| unsafe { RegisterWindowMessageA(s!("Wry::ExecMsg")) });

impl From<webview2_com::Error> for Error {
  fn from(err: webview2_com::Error) -> Self {
    Error::WebView2Error(err)
  }
}

impl From<windows::core::Error> for Error {
  fn from(err: windows::core::Error) -> Self {
    Error::WebView2Error(webview2_com::Error::WindowsError(err))
  }
}

pub(crate) struct InnerWebView {
  parent: RefCell<HWND>,
  hwnd: HWND,
  is_child: bool,
  pub controller: ICoreWebView2Controller,
  webview: ICoreWebView2,
  env: ICoreWebView2Environment,
  // Store FileDropController in here to make sure it gets dropped when
  // the webview gets dropped, otherwise we'll have a memory leak
  #[allow(dead_code)]
  drag_drop_controller: Option<DragDropController>,
}

impl Drop for InnerWebView {
  fn drop(&mut self) {
    let _ = unsafe { self.controller.Close() };
    if self.is_child {
      let _ = unsafe { DestroyWindow(self.hwnd) };
    }
    unsafe { Self::dettach_parent_subclass(*self.parent.borrow()) }
  }
}

impl InnerWebView {
  #[inline]
  pub fn new(
    window: &impl HasWindowHandle,
    attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    let window = match window.window_handle()?.as_raw() {
      RawWindowHandle::Win32(window) => HWND(window.hwnd.get() as _),
      _ => return Err(Error::UnsupportedWindowHandle),
    };
    Self::new_in_hwnd(window, attributes, pl_attrs, web_context, false)
  }

  #[inline]
  pub fn new_as_child(
    parent: &impl HasWindowHandle,
    attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    let parent = match parent.window_handle()?.as_raw() {
      RawWindowHandle::Win32(parent) => HWND(parent.hwnd.get() as _),
      _ => return Err(Error::UnsupportedWindowHandle),
    };

    Self::new_in_hwnd(parent, attributes, pl_attrs, web_context, true)
  }

  #[inline]
  fn new_in_hwnd(
    parent: HWND,
    mut attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    web_context: Option<&mut WebContext>,
    is_child: bool,
  ) -> Result<Self> {
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

    let hwnd = Self::create_container_hwnd(parent, &attributes, is_child)?;

    let drop_handler = attributes.drag_drop_handler.take();
    let bounds = attributes.bounds;

    let env = Self::create_environment(&web_context, pl_attrs.clone(), &attributes)?;
    let controller = Self::create_controller(hwnd, &env, attributes.incognito)?;
    let webview = Self::init_webview(
      parent,
      hwnd,
      attributes,
      &env,
      &controller,
      pl_attrs,
      is_child,
    )?;

    let drag_drop_controller = drop_handler.map(|handler| DragDropController::new(hwnd, handler));

    let w = Self {
      parent: RefCell::new(parent),
      hwnd,
      controller,
      is_child,
      webview,
      env,
      drag_drop_controller,
    };

    if is_child {
      w.set_bounds(bounds.unwrap_or_default())?;
    } else {
      w.resize_to_parent()?;
    }

    Ok(w)
  }

  #[inline]
  fn create_container_hwnd(
    parent: HWND,
    attributes: &WebViewAttributes,
    is_child: bool,
  ) -> Result<HWND> {
    unsafe extern "system" fn default_window_proc(
      hwnd: HWND,
      msg: u32,
      wparam: WPARAM,
      lparam: LPARAM,
    ) -> LRESULT {
      DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    let class_name = w!("WRY_WEBVIEW");

    let class = WNDCLASSEXW {
      cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
      style: CS_HREDRAW | CS_VREDRAW,
      lpfnWndProc: Some(default_window_proc),
      cbClsExtra: 0,
      cbWndExtra: 0,
      hInstance: unsafe { HINSTANCE(GetModuleHandleW(PCWSTR::null()).unwrap_or_default().0) },
      hIcon: HICON::default(),
      hCursor: HCURSOR::default(),
      hbrBackground: HBRUSH::default(),
      lpszMenuName: PCWSTR::null(),
      lpszClassName: class_name,
      hIconSm: HICON::default(),
    };

    unsafe { RegisterClassExW(&class) };

    let mut window_styles = WS_CHILD | WS_CLIPCHILDREN;
    if attributes.visible {
      window_styles |= WS_VISIBLE;
    }

    let dpi = unsafe { util::hwnd_dpi(parent) };
    let scale_factor = util::dpi_to_scale_factor(dpi);

    let (x, y, width, height) = if is_child {
      let (x, y) = attributes
        .bounds
        .map(|b| b.position.to_physical::<f64>(scale_factor))
        .map(Into::into)
        .unwrap_or((CW_USEDEFAULT, CW_USEDEFAULT));
      let (width, height) = attributes
        .bounds
        .map(|b| b.size.to_physical::<u32>(scale_factor))
        .map(Into::into)
        .unwrap_or((CW_USEDEFAULT, CW_USEDEFAULT));

      (x, y, width, height)
    } else {
      let mut rect = RECT::default();
      unsafe { GetClientRect(parent, &mut rect)? };
      let width = rect.right - rect.left;
      let height = rect.bottom - rect.top;
      (0, 0, width, height)
    };

    let hwnd = unsafe {
      CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        class_name,
        PCWSTR::null(),
        window_styles,
        x,
        y,
        width,
        height,
        parent,
        HMENU::default(),
        GetModuleHandleW(PCWSTR::null()).unwrap_or_default(),
        None,
      )
    };

    unsafe {
      SetWindowPos(
        hwnd,
        HWND_TOP,
        0,
        0,
        0,
        0,
        SWP_ASYNCWINDOWPOS | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOOWNERZORDER | SWP_NOSIZE,
      )
    }?;

    Ok(hwnd)
  }

  #[inline]
  fn create_environment(
    web_context: &Option<&mut WebContext>,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    attributes: &WebViewAttributes,
  ) -> Result<ICoreWebView2Environment> {
    let data_directory = web_context
      .as_deref()
      .and_then(|context| context.data_directory())
      .map(HSTRING::from);

    // additional browser args
    let additional_browser_args = pl_attrs.additional_browser_args.unwrap_or_else(|| {
      // remove "mini menu" - See https://github.com/tauri-apps/wry/issues/535
      // and "smart screen" - See https://github.com/tauri-apps/tauri/issues/1345
      let default_args = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";
      let mut arguments = String::from(default_args);

      if attributes.autoplay {
        arguments.push_str(" --autoplay-policy=no-user-gesture-required");
      }

      if let Some(proxy_setting) = &attributes.proxy_config {
        match proxy_setting {
          ProxyConfig::Http(endpoint) => {
            arguments.push_str(" --proxy-server=http://");
            arguments.push_str(&endpoint.host);
            arguments.push(':');
            arguments.push_str(&endpoint.port);
          }
          ProxyConfig::Socks5(endpoint) => {
            arguments.push_str(" --proxy-server=socks5://");
            arguments.push_str(&endpoint.host);
            arguments.push(':');
            arguments.push_str(&endpoint.port);
          }
        };
      }

      arguments
    });

    let additional_browser_args = HSTRING::from(additional_browser_args);

    let (tx, rx) = mpsc::channel();
    CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
      Box::new(move |environmentcreatedhandler| unsafe {
        let options: ICoreWebView2EnvironmentOptions =
          CoreWebView2EnvironmentOptions::default().into();

        let _ = options.SetAdditionalBrowserArguments(&additional_browser_args);

        // Get user's system language
        let lcid = GetUserDefaultUILanguage();
        let mut lang = [0; MAX_LOCALE_NAME as usize];
        LCIDToLocaleName(lcid as u32, Some(&mut lang), LOCALE_ALLOW_NEUTRAL_NAMES);
        options.SetLanguage(PCWSTR::from_raw(lang.as_ptr()))?;

        CreateCoreWebView2EnvironmentWithOptions(
          PCWSTR::null(),
          &data_directory.unwrap_or_default(),
          &options,
          &environmentcreatedhandler,
        )
        .map_err(Into::into)
      }),
      Box::new(move |error_code, environment| {
        error_code?;
        tx.send(environment.ok_or_else(|| windows::core::Error::from(E_POINTER)))
          .map_err(|_| windows::core::Error::from(E_UNEXPECTED))
      }),
    )?;

    rx.recv()?.map_err(Into::into)
  }

  #[inline]
  fn create_controller(
    hwnd: HWND,
    env: &ICoreWebView2Environment,
    incognito: bool,
  ) -> Result<ICoreWebView2Controller> {
    let (tx, rx) = mpsc::channel();
    let env = env.clone().cast::<ICoreWebView2Environment10>()?;
    let controller_opts = unsafe { env.CreateCoreWebView2ControllerOptions()? };

    unsafe { controller_opts.SetIsInPrivateModeEnabled(incognito)? }

    CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
      Box::new(move |handler| unsafe {
        env
          .CreateCoreWebView2ControllerWithOptions(hwnd, &controller_opts, &handler)
          .map_err(Into::into)
      }),
      Box::new(move |error_code, controller| {
        error_code?;
        tx.send(controller.ok_or_else(|| windows::core::Error::from(E_POINTER)))
          .map_err(|_| windows::core::Error::from(E_UNEXPECTED))
      }),
    )?;

    rx.recv()?.map_err(Into::into)
  }

  #[inline]
  fn init_webview(
    parent: HWND,
    hwnd: HWND,
    mut attributes: WebViewAttributes,
    env: &ICoreWebView2Environment,
    controller: &ICoreWebView2Controller,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    is_child: bool,
  ) -> Result<ICoreWebView2> {
    let webview = unsafe { controller.CoreWebView2()? };

    // Theme
    if let Some(theme) = pl_attrs.theme {
      unsafe { set_theme(&webview, theme)? };
    }

    // Background color
    if let Some(background_color) = attributes.background_color {
      if !attributes.transparent {
        unsafe { set_background_color(controller, background_color)? };
      }
    }

    // Transparent
    if attributes.transparent && !is_windows_7() {
      unsafe { set_background_color(controller, (0, 0, 0, 0))? };
    }

    // The EventRegistrationToken is an out-param from all of the event registration calls. We're
    // taking it in the local variable and then just ignoring it because all of the event handlers
    // are registered for the life of the webview, but if we wanted to be able to remove them later
    // we would hold onto them in self.
    let mut token = EventRegistrationToken::default();

    // Webview Settings
    unsafe { Self::set_webview_settings(&webview, &attributes, &pl_attrs)? };

    // Webview handlers
    unsafe { Self::attach_handlers(hwnd, &webview, &mut attributes, &mut token)? };

    // IPC handler
    unsafe { Self::attach_ipc_handler(&webview, &mut attributes, &mut token)? };

    // Custom protocols handler
    let scheme = if pl_attrs.use_https { "https" } else { "http" };
    let custom_protocols: HashSet<String> = attributes
      .custom_protocols
      .iter()
      .map(|n| n.0.clone())
      .collect();
    if !attributes.custom_protocols.is_empty() {
      unsafe {
        Self::attach_custom_protocol_handler(
          &webview,
          env,
          hwnd,
          scheme,
          &mut attributes,
          &mut token,
        )?
      };
    }

    // Initialize scripts
    for js in attributes.initialization_scripts {
      Self::add_script_to_execute_on_document_created(&webview, js)?;
    }

    // Enable clipboard
    if attributes.clipboard {
      unsafe {
        webview.add_PermissionRequested(
          &PermissionRequestedEventHandler::create(Box::new(|_, args| {
            let Some(args) = args else { return Ok(()) };

            let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
            args.PermissionKind(&mut kind)?;
            if kind == COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ {
              args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW)?;
            }

            Ok(())
          })),
          &mut token,
        )?;
      }
    }

    // Navigation
    if let Some(mut url) = attributes.url {
      if let Some(pos) = url.find("://") {
        let name = &url[..pos];
        if custom_protocols.contains(name) {
          // WebView2 supports non-standard protocols only on Windows 10+, so we have to use this workaround
          // See https://github.com/MicrosoftEdge/WebView2Feedback/issues/73
          url = url.replace(&format!("{name}://"), &format!("{scheme}://{name}."))
        }
      }

      if let Some(headers) = attributes.headers {
        load_url_with_headers(&webview, env, &url, headers)?;
      } else {
        let url = HSTRING::from(url);
        unsafe { webview.Navigate(&url)? };
      }
    } else if let Some(html) = attributes.html {
      let html = HSTRING::from(html);
      unsafe { webview.NavigateToString(&html)? };
    }

    // Subclass parent for resizing and focus
    if !is_child {
      unsafe { Self::attach_parent_subclass(parent, controller) };
    }

    unsafe {
      controller.SetIsVisible(attributes.visible)?;

      if attributes.focused {
        controller.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC)?;
      }
    }

    Ok(webview)
  }

  #[inline]
  unsafe fn set_webview_settings(
    webview: &ICoreWebView2,
    attributes: &WebViewAttributes,
    pl_attrs: &super::PlatformSpecificWebViewAttributes,
  ) -> Result<()> {
    let settings = webview.Settings()?;
    settings.SetIsStatusBarEnabled(false)?;
    settings.SetAreDefaultContextMenusEnabled(true)?;
    settings.SetIsZoomControlEnabled(attributes.zoom_hotkeys_enabled)?;
    settings.SetAreDevToolsEnabled(attributes.devtools)?;

    if let Some(user_agent) = &attributes.user_agent {
      let settings2: ICoreWebView2Settings2 = webview.Settings()?.cast()?;
      let user_agent = HSTRING::from(user_agent);
      settings2.SetUserAgent(&user_agent)?;
    }

    if !pl_attrs.browser_accelerator_keys {
      let settings3 = settings.cast::<ICoreWebView2Settings5>()?;
      settings3.SetAreBrowserAcceleratorKeysEnabled(false)?;
    }

    let settings5 = settings.cast::<ICoreWebView2Settings5>()?;
    settings5.SetIsPinchZoomEnabled(attributes.zoom_hotkeys_enabled)?;

    let settings6 = settings.cast::<ICoreWebView2Settings6>()?;
    settings6.SetIsSwipeNavigationEnabled(attributes.back_forward_navigation_gestures)?;

    if let Ok(settings9) = settings.cast::<ICoreWebView2Settings9>() {
      settings9.SetIsNonClientRegionSupportEnabled(true)?;
    }

    Ok(())
  }

  #[inline]
  unsafe fn attach_handlers(
    hwnd: HWND,
    webview: &ICoreWebView2,
    attributes: &mut WebViewAttributes,
    token: &mut EventRegistrationToken,
  ) -> Result<()> {
    // Close container HWND when `window.close` is called in JS
    webview.add_WindowCloseRequested(
      &WindowCloseRequestedEventHandler::create(Box::new(move |_, _| DestroyWindow(hwnd))),
      token,
    )?;

    // Document title changed handler
    if let Some(document_title_changed_handler) = attributes.document_title_changed_handler.take() {
      webview.add_DocumentTitleChanged(
        &DocumentTitleChangedEventHandler::create(Box::new(move |webview, _| {
          let Some(webview) = webview else {
            return Ok(());
          };

          let title = {
            let mut title = PWSTR::null();
            webview.DocumentTitle(&mut title)?;
            take_pwstr(title)
          };

          document_title_changed_handler(title);
          Ok(())
        })),
        token,
      )?;
    }

    // Page load handler
    if let Some(on_page_load_handler) = attributes.on_page_load_handler.take() {
      let on_page_load_handler = Rc::new(on_page_load_handler);
      let on_page_load_handler_ = on_page_load_handler.clone();
      webview.add_ContentLoading(
        &ContentLoadingEventHandler::create(Box::new(move |webview, _| {
          let Some(webview) = webview else {
            return Ok(());
          };

          on_page_load_handler_(PageLoadEvent::Started, Self::url_from_webview(&webview)?);

          Ok(())
        })),
        token,
      )?;
      webview.add_NavigationCompleted(
        &NavigationCompletedEventHandler::create(Box::new(move |webview, _| {
          let Some(webview) = webview else {
            return Ok(());
          };

          on_page_load_handler(PageLoadEvent::Finished, Self::url_from_webview(&webview)?);

          Ok(())
        })),
        token,
      )?;
    }

    // Navigation handler
    if let Some(nav_callback) = attributes.navigation_handler.take() {
      webview.add_NavigationStarting(
        &NavigationStartingEventHandler::create(Box::new(move |_, args| {
          let Some(args) = args else {
            return Ok(());
          };

          let uri = {
            let mut uri = PWSTR::null();
            args.Uri(&mut uri)?;
            take_pwstr(uri)
          };

          let allow = nav_callback(uri);
          args.SetCancel(!allow)?;

          Ok(())
        })),
        token,
      )?;
    }

    // New window handler
    if let Some(new_window_req_handler) = attributes.new_window_req_handler.take() {
      webview.add_NewWindowRequested(
        &NewWindowRequestedEventHandler::create(Box::new(move |_, args| {
          let Some(args) = args else {
            return Ok(());
          };

          let uri = {
            let mut uri = PWSTR::null();
            args.Uri(&mut uri)?;
            take_pwstr(uri)
          };

          let allow = new_window_req_handler(uri);
          args.SetHandled(!allow)?;

          Ok(())
        })),
        token,
      )?;
    }

    // Download handler
    if attributes.download_started_handler.is_some()
      || attributes.download_completed_handler.is_some()
    {
      let mut download_started_handler = attributes.download_started_handler.take();
      let download_completed_handler = attributes.download_completed_handler.take();

      let webview4: ICoreWebView2_4 = webview.cast()?;
      webview4.add_DownloadStarting(
        &DownloadStartingEventHandler::create(Box::new(move |_, args| {
          let Some(args) = args else {
            return Ok(());
          };

          let uri = {
            let mut uri = PWSTR::null();
            args.DownloadOperation()?.Uri(&mut uri)?;
            take_pwstr(uri)
          };

          if let Some(download_completed_handler) = &download_completed_handler {
            let download_completed_handler = download_completed_handler.clone();

            args.DownloadOperation()?.add_StateChanged(
              &StateChangedEventHandler::create(Box::new(move |download_operation, _| {
                let Some(download_operation) = download_operation else {
                  return Ok(());
                };

                let mut state = COREWEBVIEW2_DOWNLOAD_STATE::default();
                download_operation.State(&mut state)?;

                if state != COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS {
                  let uri = {
                    let mut uri = PWSTR::null();
                    download_operation.Uri(&mut uri)?;
                    take_pwstr(uri)
                  };

                  let success = state == COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED;

                  let path = if success {
                    let mut path = PWSTR::null();
                    download_operation.ResultFilePath(&mut path)?;
                    Some(PathBuf::from(take_pwstr(path)))
                  } else {
                    None
                  };

                  download_completed_handler(uri, path, success);
                }

                Ok(())
              })),
              &mut EventRegistrationToken::default(),
            )?;
          }

          if let Some(download_started_handler) = &mut download_started_handler {
            let mut path = {
              let mut path = PWSTR::null();
              args.ResultFilePath(&mut path)?;
              let path = take_pwstr(path);
              PathBuf::from(&path)
            };

            if download_started_handler(uri, &mut path) {
              let simplified = dunce::simplified(&path);
              let path = HSTRING::from(simplified);
              args.SetResultFilePath(&path)?;
              args.SetHandled(true)?;
            } else {
              args.SetCancel(true)?;
            }
          }

          Ok(())
        })),
        token,
      )?;
    }

    Ok(())
  }

  #[inline]
  unsafe fn attach_ipc_handler(
    webview: &ICoreWebView2,
    attributes: &mut WebViewAttributes,
    token: &mut EventRegistrationToken,
  ) -> Result<()> {
    Self::add_script_to_execute_on_document_created(
      webview,
      String::from(
        r#"Object.defineProperty(window, 'ipc', { value: Object.freeze({ postMessage: s=> window.chrome.webview.postMessage(s) }) });"#,
      ),
    )?;

    let ipc_handler = attributes.ipc_handler.take();
    webview.add_WebMessageReceived(
      &WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
        let (Some(args), Some(ipc_handler)) = (args, &ipc_handler) else {
          return Ok(());
        };

        let url = {
          let mut url = PWSTR::null();
          args.Source(&mut url)?;
          take_pwstr(url)
        };

        let js = {
          let mut js = PWSTR::null();
          args.TryGetWebMessageAsString(&mut js)?;
          take_pwstr(js)
        };

        #[cfg(feature = "tracing")]
        let _span = tracing::info_span!(parent: None, "wry::ipc::handle").entered();
        ipc_handler(Request::builder().uri(url).body(js).unwrap());

        Ok(())
      })),
      token,
    )?;

    Ok(())
  }

  #[inline]
  unsafe fn attach_custom_protocol_handler(
    webview: &ICoreWebView2,
    env: &ICoreWebView2Environment,
    hwnd: HWND,
    scheme: &'static str,
    attributes: &mut WebViewAttributes,
    token: &mut EventRegistrationToken,
  ) -> Result<()> {
    for (name, _) in &attributes.custom_protocols {
      // WebView2 supports non-standard protocols only on Windows 10+, so we have to use this workaround
      // See https://github.com/MicrosoftEdge/WebView2Feedback/issues/73
      let filter = HSTRING::from(format!("{scheme}://{name}.*"));
      webview.AddWebResourceRequestedFilter(&filter, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL)?;
    }

    let env = env.clone();
    let custom_protocols = std::mem::take(&mut attributes.custom_protocols);
    let main_thread_id = std::thread::current().id();

    webview.add_WebResourceRequested(
      &WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else {
          return Ok(());
        };

        #[cfg(feature = "tracing")]
        let span = tracing::info_span!(parent: None, "wry::custom_protocol::handle", uri = tracing::field::Empty)
          .entered();

        // Request uri
        let webview_request = args.Request()?;

        // Request uri
        let uri = {
          let mut uri = PWSTR::null();
          webview_request.Uri(&mut uri)?;
          take_pwstr(uri)
        };
        #[cfg(feature = "tracing")]
        span.record("uri", &uri);

        if let Some((custom_protocol, custom_protocol_handler)) = custom_protocols
          .iter()
          .find(|(protocol, _)| is_custom_protocol_uri(&uri, scheme, protocol))
        {
          let request = match Self::perpare_request(scheme, custom_protocol, &webview_request, &uri)
          {
            Ok(req) => req,
            Err(e) => {
              let err_response = Self::prepare_web_request_err(&env, e)?;
              args.SetResponse(&err_response)?;
              return Ok(());
            }
          };

          let env = env.clone();
          let deferral = args.GetDeferral();

          let async_responder = Box::new(move |sent_response| {
            let handler = move || {
              match Self::prepare_web_request_response(&env, &sent_response) {
                Ok(response) => {
                  let _ = args.SetResponse(&response);
                }
                Err(e) => {
                  if let Ok(err_response) = Self::prepare_web_request_err(&env, e) {
                    let _ = args.SetResponse(&err_response);
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
              Self::dispatch_handler(hwnd, handler);
            }
          });

          #[cfg(feature = "tracing")]
          let _span = tracing::info_span!("wry::custom_protocol::call_handler").entered();
          custom_protocol_handler(
            request,
            RequestAsyncResponder {
              responder: async_responder,
            },
          );
        }

        Ok(())
      })),
      token,
    )?;

    Self::attach_main_thread_dispatcher(hwnd);

    Ok(())
  }

  #[inline]
  unsafe fn perpare_request(
    scheme: &'static str,
    custom_protocol: &str,
    webview_request: &ICoreWebView2WebResourceRequest,
    webview_request_uri: &str,
  ) -> Result<http::Request<Vec<u8>>> {
    let mut request = Request::builder();

    // Request method (GET, POST, PUT etc..)
    let mut method = PWSTR::null();
    webview_request.Method(&mut method)?;
    let method = take_pwstr(method);
    request = request.method(method.as_str());

    // Get all headers from the request
    let headers = webview_request.Headers()?.GetIterator()?;
    let mut has_current = BOOL::default();
    headers.HasCurrentHeader(&mut has_current)?;
    while has_current.as_bool() {
      let mut key = PWSTR::null();
      let mut value = PWSTR::null();
      headers.GetCurrentHeader(&mut key, &mut value)?;

      let (key, value) = (take_pwstr(key), take_pwstr(value));
      request = request.header(&key, &value);

      headers.MoveNext(&mut has_current)?;
    }

    // Get the body if available
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

    // Undo the protocol workaround when giving path to resolver
    let path = webview_request_uri.replace(
      &format!("{scheme}://{}.", custom_protocol),
      &format!("{}://", custom_protocol),
    );

    let request = request.uri(&path).body(body_sent)?;

    Ok(request)
  }

  #[inline]
  unsafe fn prepare_web_request_response(
    env: &ICoreWebView2Environment,
    sent_response: &HttpResponse<Cow<'static, [u8]>>,
  ) -> windows::core::Result<ICoreWebView2WebResourceResponse> {
    let content = sent_response.body();

    let status = sent_response.status();
    let status_code = status.as_u16();
    let status = HSTRING::from(status.canonical_reason().unwrap_or("OK"));

    let mut headers_map = String::new();
    for (name, value) in sent_response.headers().iter() {
      let header_key = name.to_string();
      if let Ok(value) = value.to_str() {
        let _ = writeln!(headers_map, "{}: {}", header_key, value);
      }
    }
    let headers_map = HSTRING::from(headers_map);

    let mut stream = None;
    if !content.is_empty() {
      stream = SHCreateMemStream(Some(content));
    }

    env.CreateWebResourceResponse(stream.as_ref(), status_code as i32, &status, &headers_map)
  }

  #[inline]
  unsafe fn prepare_web_request_err<T: ToString>(
    env: &ICoreWebView2Environment,
    err: T,
  ) -> windows::core::Result<ICoreWebView2WebResourceResponse> {
    let status = StatusCode::BAD_REQUEST;
    let status_code = status.as_u16();
    let status = HSTRING::from(status.canonical_reason().unwrap_or("Bad Request"));
    let error = HSTRING::from(err.to_string());
    env.CreateWebResourceResponse(None, status_code as i32, &status, &error)
  }

  #[inline]
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

  unsafe extern "system" fn main_thread_dispatcher_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uidsubclass: usize,
    _dwrefdata: usize,
  ) -> LRESULT {
    if msg == *EXEC_MSG_ID {
      let mut function: Box<Box<dyn FnMut()>> = Box::from_raw(wparam.0 as *mut _);
      function();
      let _ = RedrawWindow(hwnd, None, HRGN::default(), RDW_INTERNALPAINT);
      return LRESULT(0);
    }

    DefSubclassProc(hwnd, msg, wparam, lparam)
  }

  unsafe fn attach_main_thread_dispatcher(hwnd: HWND) {
    let _ = SetWindowSubclass(
      hwnd,
      Some(Self::main_thread_dispatcher_proc),
      MAIN_THREAD_DISPATCHER_SUBCLASS_ID as _,
      0,
    );
  }

  unsafe extern "system" fn parent_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uidsubclass: usize,
    dwrefdata: usize,
  ) -> LRESULT {
    match msg {
      WM_SIZE => {
        if wparam.0 != SIZE_MINIMIZED as usize {
          let controller = dwrefdata as *mut ICoreWebView2Controller;
          let mut rect = RECT::default();
          let _ = GetClientRect(hwnd, &mut rect);
          let width = rect.right - rect.left;
          let height = rect.bottom - rect.top;

          let _ = (*controller).SetBounds(RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
          });

          let mut hwnd = HWND::default();
          if (*controller).ParentWindow(&mut hwnd).is_ok() {
            let _ = SetWindowPos(
              hwnd,
              HWND::default(),
              0,
              0,
              width,
              height,
              SWP_ASYNCWINDOWPOS | SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOMOVE,
            );
          }
        }
      }

      WM_SETFOCUS | WM_ENTERSIZEMOVE => {
        let controller = dwrefdata as *mut ICoreWebView2Controller;
        let _ = (*controller).MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
      }

      WM_WINDOWPOSCHANGED => {
        let controller = dwrefdata as *mut ICoreWebView2Controller;
        let _ = (*controller).NotifyParentWindowPositionChanged();
      }

      msg if msg == WM_DESTROY || msg == PARENT_DESTROY_MESSAGE => {
        // check if `dwrefdata` is null to avoid double-freeing the controller
        if !(dwrefdata as *mut ()).is_null() {
          drop(Box::from_raw(dwrefdata as *mut ICoreWebView2Controller));

          // update `dwrefdata` to null to avoid double-freeing the controller
          let _ = SetWindowSubclass(
            hwnd,
            Some(Self::parent_subclass_proc),
            PARENT_SUBCLASS_ID as _,
            std::ptr::null::<()>() as _,
          );
        }
      }

      _ => (),
    }

    DefSubclassProc(hwnd, msg, wparam, lparam)
  }

  #[inline]
  unsafe fn attach_parent_subclass(parent: HWND, controller: &ICoreWebView2Controller) {
    let _ = SetWindowSubclass(
      parent,
      Some(Self::parent_subclass_proc),
      PARENT_SUBCLASS_ID as _,
      Box::into_raw(Box::new(controller.clone())) as _,
    );
  }

  #[inline]
  unsafe fn dettach_parent_subclass(parent: HWND) {
    SendMessageW(
      parent,
      PARENT_DESTROY_MESSAGE,
      WPARAM::default(),
      LPARAM::default(),
    );
    let _ = RemoveWindowSubclass(
      parent,
      Some(Self::parent_subclass_proc),
      PARENT_SUBCLASS_ID as _,
    );
  }

  // TODO: feature to allow injecting into (specific) subframes
  #[inline]
  fn add_script_to_execute_on_document_created(webview: &ICoreWebView2, js: String) -> Result<()> {
    let webview = webview.clone();
    AddScriptToExecuteOnDocumentCreatedCompletedHandler::wait_for_async_operation(
      Box::new(move |handler| unsafe {
        let js = HSTRING::from(js);
        webview
          .AddScriptToExecuteOnDocumentCreated(&js, &handler)
          .map_err(Into::into)
      }),
      Box::new(|e, _| e),
    )
    .map_err(Into::into)
  }

  #[inline]
  fn execute_script(
    webview: &ICoreWebView2,
    js: String,
    callback: impl FnOnce(String) + Send + 'static,
  ) -> windows::core::Result<()> {
    unsafe {
      #[cfg(feature = "tracing")]
      let span = tracing::debug_span!("wry::eval").entered();
      let js = HSTRING::from(js);
      webview.ExecuteScript(
        &js,
        &ExecuteScriptCompletedHandler::create(Box::new(|_, res| {
          #[cfg(feature = "tracing")]
          drop(span);
          callback(res);
          Ok(())
        })),
      )
    }
  }

  #[inline]
  fn url_from_webview(webview: &ICoreWebView2) -> windows::core::Result<String> {
    let mut pwstr = PWSTR::null();
    unsafe { webview.Source(&mut pwstr)? };
    Ok(take_pwstr(pwstr))
  }
}

/// Public APIs
impl InnerWebView {
  pub fn eval(
    &self,
    js: &str,
    callback: Option<impl FnOnce(String) + Send + 'static>,
  ) -> Result<()> {
    match callback {
      Some(callback) => Self::execute_script(&self.webview, js.to_string(), callback)?,
      None => Self::execute_script(&self.webview, js.to_string(), |_| ())?,
    }

    Ok(())
  }

  pub fn url(&self) -> Result<String> {
    Self::url_from_webview(&self.webview).map_err(Into::into)
  }

  pub fn zoom(&self, scale_factor: f64) -> Result<()> {
    unsafe { self.controller.SetZoomFactor(scale_factor) }.map_err(Into::into)
  }

  pub fn load_url(&self, url: &str) -> Result<()> {
    let url = HSTRING::from(url);
    unsafe { self.webview.Navigate(&url) }.map_err(Into::into)
  }

  pub fn load_url_with_headers(&self, url: &str, headers: http::HeaderMap) -> Result<()> {
    load_url_with_headers(&self.webview, &self.env, url, headers)
  }

  pub fn bounds(&self) -> Result<Rect> {
    let mut bounds = Rect::default();

    if self.is_child {
      let mut rect = RECT::default();
      unsafe { GetClientRect(self.hwnd, &mut rect)? };

      let position_point = &mut [POINT {
        x: rect.left,
        y: rect.top,
      }];
      unsafe { MapWindowPoints(self.hwnd, *self.parent.borrow(), position_point) };

      bounds.position = LogicalPosition::new(position_point[0].x, position_point[0].y).into();
      bounds.size = LogicalSize::new(
        (rect.right - rect.left) as u32,
        (rect.bottom - rect.top) as u32,
      )
      .into();
    } else {
      let mut rect = RECT::default();
      unsafe { self.controller.Bounds(&mut rect) }?;
      bounds.size = LogicalSize::new(
        (rect.right - rect.left) as u32,
        (rect.bottom - rect.top) as u32,
      )
      .into();
    }

    Ok(bounds)
  }

  pub fn set_bounds_inner(
    &self,
    size: PhysicalSize<i32>,
    position: PhysicalPosition<i32>,
  ) -> Result<()> {
    unsafe {
      self.controller.SetBounds(RECT {
        top: 0,
        left: 0,
        right: size.width,
        bottom: size.height,
      })?;

      SetWindowPos(
        self.hwnd,
        HWND::default(),
        position.x,
        position.y,
        size.width,
        size.height,
        SWP_ASYNCWINDOWPOS | SWP_NOACTIVATE | SWP_NOZORDER,
      )?;
    }

    Ok(())
  }

  pub fn set_bounds(&self, bounds: Rect) -> Result<()> {
    let dpi = unsafe { util::hwnd_dpi(self.hwnd) };
    let scale_factor = util::dpi_to_scale_factor(dpi);
    let size = bounds.size.to_physical::<i32>(scale_factor);
    let position = bounds.position.to_physical(scale_factor);
    self.set_bounds_inner(size, position)?;
    Ok(())
  }

  fn resize_to_parent(&self) -> crate::Result<()> {
    let mut rect = RECT::default();
    unsafe { GetClientRect(*self.parent.borrow(), &mut rect)? };
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    self.set_bounds_inner((width, height).into(), (0, 0).into())
  }

  pub fn set_visible(&self, visible: bool) -> Result<()> {
    unsafe {
      let _ = ShowWindow(
        self.hwnd,
        match visible {
          true => SW_SHOW,
          false => SW_HIDE,
        },
      );

      self.controller.SetIsVisible(visible)?;
    }

    Ok(())
  }

  pub fn focus(&self) -> Result<()> {
    unsafe {
      self
        .controller
        .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC)
        .map_err(Into::into)
    }
  }

  pub fn reparent(&self, parent: isize) -> Result<()> {
    let parent = HWND(parent);

    unsafe {
      SetParent(self.hwnd, parent);

      if !self.is_child {
        Self::dettach_parent_subclass(*self.parent.borrow());
        Self::attach_parent_subclass(parent, &self.controller);

        *self.parent.borrow_mut() = parent;

        let mut rect = RECT::default();
        GetClientRect(parent, &mut rect)?;

        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;

        self.set_bounds_inner((width, height).into(), (0, 0).into())?;
      }
    }

    Ok(())
  }

  pub fn print(&self) -> Result<()> {
    self.eval(
      "window.print()",
      None::<Box<dyn FnOnce(String) + Send + 'static>>,
    )
  }

  pub fn clear_all_browsing_data(&self) -> Result<()> {
    unsafe {
      self
        .webview
        .cast::<ICoreWebView2_13>()?
        .Profile()?
        .cast::<ICoreWebView2Profile2>()?
        .ClearBrowsingDataAll(&ClearBrowsingDataCompletedHandler::create(Box::new(
          move |_| Ok(()),
        )))
        .map_err(Into::into)
    }
  }

  pub fn set_theme(&self, theme: Theme) -> Result<()> {
    unsafe { set_theme(&self.webview, theme) }
  }

  pub fn set_background_color(&self, background_color: RGBA) -> Result<()> {
    unsafe { set_background_color(&self.controller, background_color).map_err(Into::into) }
  }

  pub fn set_memory_usage_level(&self, level: MemoryUsageLevel) -> Result<()> {
    let webview = self.webview.cast::<ICoreWebView2_19>()?;
    // https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2memoryusagetargetlevel
    let level = match level {
      MemoryUsageLevel::Normal => 0,
      MemoryUsageLevel::Low => 1,
    };
    let level = COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL(level);
    unsafe { webview.SetMemoryUsageTargetLevel(level).map_err(Into::into) }
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
}

#[inline]
fn load_url_with_headers(
  webview: &ICoreWebView2,
  env: &ICoreWebView2Environment,
  url: &str,
  headers: http::HeaderMap,
) -> Result<()> {
  let url = HSTRING::from(url);

  let headers_map = {
    let mut headers_map = String::new();
    for (name, value) in headers.iter() {
      let header_key = name.to_string();
      if let Ok(value) = value.to_str() {
        let _ = writeln!(headers_map, "{}: {}", header_key, value);
      }
    }
    HSTRING::from(headers_map)
  };

  unsafe {
    let env = env.cast::<ICoreWebView2Environment9>()?;
    let method = HSTRING::from("GET");
    if let Ok(request) = env.CreateWebResourceRequest(&url, &method, None, &headers_map) {
      let webview: ICoreWebView2_10 = webview.cast()?;
      webview.NavigateWithWebResourceRequest(&request)?;
    }
  };

  Ok(())
}

#[inline]
unsafe fn set_background_color(
  controller: &ICoreWebView2Controller,
  background_color: RGBA,
) -> Result<()> {
  let mut color = background_color;
  if is_windows_7() || color.3 != 0 {
    color.3 = 255;
  }

  let controller2: ICoreWebView2Controller2 = controller.cast()?;
  controller2
    .SetDefaultBackgroundColor(COREWEBVIEW2_COLOR {
      R: color.0,
      G: color.1,
      B: color.2,
      A: color.3,
    })
    .map_err(Into::into)
}

#[inline]
unsafe fn set_theme(webview: &ICoreWebView2, theme: Theme) -> Result<()> {
  let webview = webview.cast::<ICoreWebView2_13>()?;
  let profile = webview.Profile()?;
  profile
    .SetPreferredColorScheme(match theme {
      Theme::Dark => COREWEBVIEW2_PREFERRED_COLOR_SCHEME_DARK,
      Theme::Light => COREWEBVIEW2_PREFERRED_COLOR_SCHEME_LIGHT,
      Theme::Auto => COREWEBVIEW2_PREFERRED_COLOR_SCHEME_AUTO,
    })
    .map_err(Into::into)
}

#[inline]
fn is_custom_protocol_uri(uri: &str, scheme: &'static str, protocol: &str) -> bool {
  let uri_len = uri.len();
  let scheme_len = scheme.len();
  let protocol_len = protocol.len();

  // starts with `http` or `https``
  &uri[..scheme_len] == scheme
  // followed by `://`
  && &uri[scheme_len..scheme_len + 3] == "://"
  // followed by custom protocol name
  && scheme_len + 3 + protocol_len < uri_len && &uri[scheme_len + 3.. scheme_len + 3 + protocol_len] == protocol
  // and a dot
  && scheme_len + 3 + protocol_len < uri_len && uri.as_bytes()[scheme_len + 3 + protocol_len] == b'.'
}

pub fn platform_webview_version() -> Result<String> {
  let mut versioninfo = PWSTR::null();
  unsafe { GetAvailableCoreWebView2BrowserVersionString(PCWSTR::null(), &mut versioninfo) }?;
  Ok(take_pwstr(versioninfo))
}

#[inline]
fn is_windows_7() -> bool {
  let v = windows_version::OsVersion::current();
  // windows 7 is 6.1
  v.major == 6 && v.minor == 1
}

#[cfg(test)]
mod tests {
  use super::is_custom_protocol_uri;

  #[test]
  fn checks_if_custom_protocol_uri() {
    let scheme = "http";
    let uri = "http://wry.localhost/path/to/page";
    assert!(is_custom_protocol_uri(uri, scheme, "wry"));
    assert!(!is_custom_protocol_uri(uri, scheme, "asset"));
  }
}
