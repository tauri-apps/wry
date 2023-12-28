// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use gdkx11::{
  glib::translate::{FromGlibPtrFull, ToGlibPtr},
  X11Display,
};
use gtk::{
  gdk::{self, EventMask},
  gio::Cancellable,
  prelude::*,
};
use javascriptcore::ValueExt;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
#[cfg(any(debug_assertions, feature = "devtools"))]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use url::Url;
use webkit2gtk::{
  AutoplayPolicy, InputMethodContextExt, LoadEvent, NavigationPolicyDecision,
  NavigationPolicyDecisionExt, NetworkProxyMode, NetworkProxySettings, PolicyDecisionType,
  SettingsExt, URIRequest, URIRequestExt, UserContentInjectedFrames, UserContentManagerExt,
  UserScript, UserScriptInjectionTime, WebInspectorExt, WebView, WebViewExt, WebsiteDataManagerExt,
  WebsitePolicies,
};
use webkit2gtk_sys::{
  webkit_get_major_version, webkit_get_micro_version, webkit_get_minor_version,
  webkit_policy_decision_ignore, webkit_policy_decision_use,
};
use x11_dl::xlib::*;

use web_context::WebContextExt;
pub use web_context::WebContextImpl;

use crate::{
  proxy::ProxyConfig, web_context::WebContext, Error, PageLoadEvent, Rect, Result,
  WebViewAttributes, RGBA,
};

mod file_drop;
mod synthetic_mouse_events;
mod web_context;

pub(crate) struct InnerWebView {
  pub webview: WebView,
  #[cfg(any(debug_assertions, feature = "devtools"))]
  is_inspector_open: Arc<AtomicBool>,
  pending_scripts: Arc<Mutex<Option<Vec<String>>>>,

  is_child: bool,
  xlib: Option<Xlib>,
  x11_display: Option<*mut std::ffi::c_void>,
  x11_window: Option<u64>,
  display: Option<gdk::Display>,
  gtk_window: Option<gtk::Window>,

  is_in_fixed_parent: bool,
}

impl Drop for InnerWebView {
  fn drop(&mut self) {
    unsafe { self.webview.destroy() }

    if let Some(xlib) = &self.xlib {
      if self.is_child {
        unsafe { (xlib.XDestroyWindow)(self.x11_display.unwrap() as _, self.x11_window.unwrap()) };
      }
    }

    if let Some(window) = &self.gtk_window {
      window.close();
    }
  }
}

impl InnerWebView {
  pub fn new<W: HasRawWindowHandle>(
    window: &W,
    attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    Self::new_x11(window, attributes, pl_attrs, web_context, false)
  }

  pub fn new_as_child<W: HasRawWindowHandle>(
    parent: &W,
    attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    Self::new_x11(parent, attributes, pl_attrs, web_context, true)
  }

  fn new_x11<W: HasRawWindowHandle>(
    window: &W,
    attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    web_context: Option<&mut WebContext>,
    is_child: bool,
  ) -> Result<Self> {
    let xlib = Xlib::open()?;

    let window_handle = match window.raw_window_handle() {
      RawWindowHandle::Xlib(w) => w.window,
      _ => return Err(Error::UnsupportedWindowHandle),
    };

    let gdk_display = gdk::Display::default().ok_or(Error::X11DisplayNotFound)?;
    let gx11_display: &X11Display = gdk_display.downcast_ref().unwrap();
    let raw = gx11_display.to_glib_none().0;
    let display = unsafe { gdkx11::ffi::gdk_x11_display_get_xdisplay(raw) };

    let window = if is_child {
      let child = unsafe {
        (xlib.XCreateSimpleWindow)(
          display as _,
          window_handle,
          attributes.bounds.map(|p| p.x).unwrap_or(0),
          attributes.bounds.map(|p| p.y).unwrap_or(0),
          // it is unlikey that bounds are not set because
          // we have a default for it, but anyways we need to have a fallback
          // and we need to use 1 not 0 here otherwise xlib will crash
          attributes.bounds.map(|s| s.width).unwrap_or(1),
          attributes.bounds.map(|s| s.height).unwrap_or(1),
          0,
          0,
          0,
        )
      };
      if attributes.visible {
        unsafe { (xlib.XMapWindow)(display as _, child) };
      }
      child
    } else {
      window_handle
    };

    let gdk_window = unsafe {
      let raw = gdkx11::ffi::gdk_x11_window_foreign_new_for_display(raw, window);
      gdk::Window::from_glib_full(raw)
    };
    let gtk_window = gtk::Window::new(gtk::WindowType::Toplevel);
    gtk_window.connect_realize(move |widget| widget.set_window(gdk_window.clone()));
    gtk_window.set_has_window(true);
    gtk_window.realize();

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
    gtk_window.add(&vbox);

    let hidden = !attributes.visible;

    Self::new_gtk(&vbox, attributes, pl_attrs, web_context).map(|mut w| {
      // for some reason, if the webview starts as hidden,
      // we will need about 3 calls to `webview.set_visible`
      // with alternating value.
      // calling gtk_window.show_all() then hiding it again
      // seems to fix the issue.
      gtk_window.show_all();
      if hidden {
        gtk_window.hide();
      }

      w.is_child = is_child;
      w.xlib = Some(xlib);
      w.display = Some(gdk_display);
      w.x11_display = Some(display as _);
      w.x11_window = Some(window);
      w.gtk_window = Some(gtk_window);

      w
    })
  }

  pub fn new_gtk<W>(
    container: &W,
    mut attributes: WebViewAttributes,
    _pl_attrs: super::PlatformSpecificWebViewAttributes,
    web_context: Option<&mut WebContext>,
  ) -> Result<Self>
  where
    W: IsA<gtk::Container>,
  {
    let window_id = container.as_ptr() as isize;

    // default_context allows us to create a scoped context on-demand
    let mut default_context;
    let web_context = if attributes.incognito {
      default_context = WebContext::new_ephemeral();
      &mut default_context
    } else {
      match web_context {
        Some(w) => w,
        None => {
          default_context = Default::default();
          &mut default_context
        }
      }
    };
    if let Some(proxy_setting) = attributes.proxy_config {
      let proxy_uri = match proxy_setting {
        ProxyConfig::Http(endpoint) => format!("http://{}:{}", endpoint.host, endpoint.port),
        ProxyConfig::Socks5(endpoint) => {
          format!("socks5://{}:{}", endpoint.host, endpoint.port)
        }
      };
      use webkit2gtk::WebContextExt;
      if let Some(website_data_manager) = web_context.context().website_data_manager() {
        let mut settings = NetworkProxySettings::new(Some(proxy_uri.as_str()), &[]);
        website_data_manager
          .set_network_proxy_settings(NetworkProxyMode::Custom, Some(&mut settings));
      }
    }
    let webview = {
      let mut webview = WebView::builder();
      webview = webview.user_content_manager(web_context.manager());
      webview = webview.web_context(web_context.context());
      webview = webview.is_controlled_by_automation(web_context.allows_automation());
      if attributes.autoplay {
        webview = webview.website_policies(
          &WebsitePolicies::builder()
            .autoplay(AutoplayPolicy::Allow)
            .build(),
        );
      }
      webview.build()
    };

    // Disable input preedit,fcitx input editor can anchor at edit cursor position
    if let Some(input_context) = webview.input_method_context() {
      input_context.set_enable_preedit(false);
    }

    web_context.register_automation(webview.clone());

    // Message handler
    let ipc_handler = attributes.ipc_handler.take();
    let manager = web_context.manager();

    // Connect before registering as recommended by the docs
    manager.connect_script_message_received(None, move |_m, msg| {
      #[cfg(feature = "tracing")]
      let _span = tracing::info_span!("wry::ipc::handle").entered();

      if let Some(js) = msg.js_value() {
        if let Some(ipc_handler) = &ipc_handler {
          ipc_handler(js.to_string());
        }
      }
    });

    // Register the handler we just connected
    manager.register_script_message_handler(&window_id.to_string());

    // document title changed handler
    if let Some(document_title_changed_handler) = attributes.document_title_changed_handler {
      webview.connect_title_notify(move |webview| {
        document_title_changed_handler(webview.title().map(|t| t.to_string()).unwrap_or_default())
      });
    }

    let on_page_load_handler = attributes.on_page_load_handler.take();
    if on_page_load_handler.is_some() {
      webview.connect_load_changed(move |webview, load_event| match load_event {
        LoadEvent::Committed => {
          if let Some(ref f) = on_page_load_handler {
            f(PageLoadEvent::Started, webview.uri().unwrap().to_string());
          }
        }
        LoadEvent::Finished => {
          if let Some(ref f) = on_page_load_handler {
            f(PageLoadEvent::Finished, webview.uri().unwrap().to_string());
          }
        }
        _ => (),
      });
    }

    webview.add_events(
      EventMask::POINTER_MOTION_MASK
        | EventMask::BUTTON1_MOTION_MASK
        | EventMask::BUTTON_PRESS_MASK
        | EventMask::TOUCH_MASK,
    );

    synthetic_mouse_events::setup(&webview);

    if attributes.navigation_handler.is_some() || attributes.new_window_req_handler.is_some() {
      webview.connect_decide_policy(move |_webview, policy_decision, policy_type| {
        let handler = match policy_type {
          PolicyDecisionType::NavigationAction => &attributes.navigation_handler,
          PolicyDecisionType::NewWindowAction => &attributes.new_window_req_handler,
          _ => &None,
        };

        if let Some(handler) = handler {
          if let Some(policy) = policy_decision.dynamic_cast_ref::<NavigationPolicyDecision>() {
            if let Some(nav_action) = policy.navigation_action() {
              if let Some(uri_req) = nav_action.request() {
                if let Some(uri) = uri_req.uri() {
                  let allow = handler(uri.to_string());
                  let pointer = policy_decision.as_ptr();
                  unsafe {
                    if allow {
                      webkit_policy_decision_use(pointer)
                    } else {
                      webkit_policy_decision_ignore(pointer)
                    }
                  }
                }
              }
            }
          }
        }
        true
      });
    }

    if attributes.download_started_handler.is_some()
      || attributes.download_completed_handler.is_some()
    {
      web_context.register_download_handler(
        attributes.download_started_handler,
        attributes.download_completed_handler,
      )
    }

    let mut is_in_fixed_parent = false;

    if container.type_().name() == "GtkBox" {
      container
        .dynamic_cast_ref::<gtk::Box>()
        .unwrap()
        .pack_start(&webview, true, true, 0);
    } else if container.type_().name() == "GtkFixed" {
      webview.set_size_request(
        attributes.bounds.map(|s| s.width).unwrap_or(1) as i32,
        attributes.bounds.map(|s| s.height).unwrap_or(1) as i32,
      );

      container.dynamic_cast_ref::<gtk::Fixed>().unwrap().put(
        &webview,
        attributes.bounds.map(|p| p.x).unwrap_or(0),
        attributes.bounds.map(|p| p.y).unwrap_or(0),
      );

      is_in_fixed_parent = true;
    } else {
      container.add(&webview);
    }

    if attributes.visible {
      webview.show_all();
    }

    if attributes.focused {
      webview.grab_focus();
    }

    if let Some(context) = WebViewExt::context(&webview) {
      use webkit2gtk::WebContextExt;
      context.set_use_system_appearance_for_scrollbars(false);
    }

    // Enable webgl, webaudio, canvas features as default.
    if let Some(settings) = WebViewExt::settings(&webview) {
      settings.set_enable_webgl(true);
      settings.set_enable_webaudio(true);
      settings
        .set_enable_back_forward_navigation_gestures(attributes.back_forward_navigation_gestures);

      // Enable clipboard
      if attributes.clipboard {
        settings.set_javascript_can_access_clipboard(true);
      }

      // Enable App cache
      settings.set_enable_offline_web_application_cache(true);
      settings.set_enable_page_cache(true);

      // Set user agent
      settings.set_user_agent(attributes.user_agent.as_deref());

      if attributes.devtools {
        settings.set_enable_developer_extras(true);
      }
    }

    // Transparent
    if attributes.transparent {
      webview.set_background_color(&gtk::gdk::RGBA::new(0., 0., 0., 0.));
    } else {
      // background color
      if let Some(background_color) = attributes.background_color {
        webview.set_background_color(&gtk::gdk::RGBA::new(
          background_color.0 as _,
          background_color.1 as _,
          background_color.2 as _,
          background_color.3 as _,
        ));
      }
    }

    // File drop handling
    if let Some(file_drop_handler) = attributes.file_drop_handler {
      file_drop::connect_drag_event(webview.clone(), file_drop_handler);
    }

    #[cfg(any(debug_assertions, feature = "devtools"))]
    let is_inspector_open = {
      let is_inspector_open = Arc::new(AtomicBool::default());
      if let Some(inspector) = WebViewExt::inspector(&webview) {
        let is_inspector_open_ = is_inspector_open.clone();
        inspector.connect_bring_to_front(move |_| {
          is_inspector_open_.store(true, Ordering::Relaxed);
          false
        });
        let is_inspector_open_ = is_inspector_open.clone();
        inspector.connect_closed(move |_| {
          is_inspector_open_.store(false, Ordering::Relaxed);
        });
      }
      is_inspector_open
    };

    let w = Self {
      webview,
      #[cfg(any(debug_assertions, feature = "devtools"))]
      is_inspector_open,
      pending_scripts: Arc::new(Mutex::new(Some(Vec::new()))),
      is_child: false,
      xlib: None,
      display: None,
      x11_display: None,
      x11_window: None,
      gtk_window: None,
      is_in_fixed_parent,
    };

    // Initialize message handler
    let mut init = String::with_capacity(115 + 20 + 22);
    init.push_str("Object.defineProperty(window, 'ipc', {value: Object.freeze({postMessage:function(x){window.webkit.messageHandlers[\"");
    init.push_str(&window_id.to_string());
    init.push_str("\"].postMessage(x)}})})");
    w.init(&init)?;

    // Initialize scripts
    for js in attributes.initialization_scripts {
      w.init(&js)?;
    }

    for (name, handler) in attributes.custom_protocols {
      match web_context.register_uri_scheme(&name, handler) {
        // Swallow duplicate scheme errors to preserve current behavior.
        // FIXME: we should log this error in the future
        Err(Error::DuplicateCustomProtocol(_)) => (),
        Err(e) => return Err(e),
        Ok(_) => (),
      }
    }

    // Navigation
    if let Some(url) = attributes.url {
      web_context.queue_load_uri(w.webview.clone(), url, attributes.headers);
      web_context.flush_queue_loader();
    } else if let Some(html) = attributes.html {
      w.webview.load_html(&html, None);
    }

    let pending_scripts = w.pending_scripts.clone();
    w.webview.connect_load_changed(move |webview, event| {
      if let LoadEvent::Committed = event {
        let mut pending_scripts_ = pending_scripts.lock().unwrap();
        if let Some(pending_scripts) = &*pending_scripts_ {
          let cancellable: Option<&Cancellable> = None;
          for script in pending_scripts {
            webview.run_javascript(script, cancellable, |_| ());
          }
          *pending_scripts_ = None;
        }
      }
    });

    Ok(w)
  }

  pub fn print(&self) {
    let _ = self.eval(
      "window.print()",
      None::<Box<dyn FnOnce(String) + Send + 'static>>,
    );
  }

  pub fn url(&self) -> Url {
    let uri = self.webview.uri().unwrap();

    Url::parse(uri.as_str()).unwrap()
  }

  pub fn eval(
    &self,
    js: &str,
    callback: Option<impl FnOnce(String) + Send + 'static>,
  ) -> Result<()> {
    if let Some(pending_scripts) = &mut *self.pending_scripts.lock().unwrap() {
      pending_scripts.push(js.into());
    } else {
      let cancellable: Option<&Cancellable> = None;

      #[cfg(feature = "tracing")]
      let span = SendEnteredSpan(tracing::debug_span!("wry::eval").entered());

      self.webview.run_javascript(js, cancellable, |result| {
        #[cfg(feature = "tracing")]
        drop(span);

        if let Some(callback) = callback {
          let mut result_str = String::new();

          if let Ok(js_result) = result {
            if let Some(js_value) = js_result.js_value() {
              if let Some(json_str) = js_value.to_json(0) {
                result_str = json_str.to_string();
              }
            }
          }

          callback(result_str);
        }
      });
    }

    Ok(())
  }

  fn init(&self, js: &str) -> Result<()> {
    if let Some(manager) = self.webview.user_content_manager() {
      let script = UserScript::new(
        js,
        // FIXME: We allow subframe injection because webview2 does and cannot be disabled (currently).
        // once webview2 allows disabling all-frame script injection, TopFrame should be set
        // if it does not break anything. (originally added for isolation pattern).
        UserContentInjectedFrames::TopFrame,
        UserScriptInjectionTime::Start,
        &[],
        &[],
      );
      manager.add_script(&script);
    } else {
      return Err(Error::InitScriptError);
    }
    Ok(())
  }

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn open_devtools(&self) {
    if let Some(inspector) = WebViewExt::inspector(&self.webview) {
      inspector.show();
      // `bring-to-front` is not received in this case
      self.is_inspector_open.store(true, Ordering::Relaxed);
    }
  }

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn close_devtools(&self) {
    if let Some(inspector) = WebViewExt::inspector(&self.webview) {
      inspector.close();
    }
  }

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn is_devtools_open(&self) -> bool {
    self.is_inspector_open.load(Ordering::Relaxed)
  }

  pub fn zoom(&self, scale_factor: f64) {
    WebViewExt::set_zoom_level(&self.webview, scale_factor);
  }

  pub fn set_background_color(&self, background_color: RGBA) -> Result<()> {
    self.webview.set_background_color(&gtk::gdk::RGBA::new(
      background_color.0 as _,
      background_color.1 as _,
      background_color.2 as _,
      background_color.3 as _,
    ));
    Ok(())
  }

  pub fn load_url(&self, url: &str) {
    self.webview.load_uri(url)
  }

  pub fn load_url_with_headers(&self, url: &str, headers: http::HeaderMap) {
    let req = URIRequest::builder().uri(url).build();

    if let Some(ref mut req_headers) = req.http_headers() {
      for (header, value) in headers.iter() {
        req_headers.append(
          header.to_string().as_str(),
          value.to_str().unwrap_or_default(),
        );
      }
    }

    self.webview.load_request(&req);
  }

  pub fn clear_all_browsing_data(&self) -> Result<()> {
    use webkit2gtk::WebContextExt;
    if let Some(context) = WebViewExt::context(&self.webview) {
      if let Some(data_manger) = context.website_data_manager() {
        webkit2gtk::WebsiteDataManagerExtManual::clear(
          &data_manger,
          webkit2gtk::WebsiteDataTypes::ALL,
          gtk::glib::TimeSpan::from_seconds(0),
          None::<&Cancellable>,
          |_| {},
        );
      }
    }

    Ok(())
  }

  pub fn bounds(&self) -> Rect {
    let mut bounds = Rect::default();

    if let (Some(xlib), Some(display), Some(window_handle)) =
      (&self.xlib, self.x11_display, self.x11_window)
    {
      unsafe {
        let mut attributes = std::mem::MaybeUninit::new(x11_dl::xlib::XWindowAttributes {
          ..std::mem::zeroed()
        })
        .assume_init();
        let ok = (xlib.XGetWindowAttributes)(display as _, window_handle, &mut attributes);

        if ok != 0 {
          bounds.x = attributes.x;
          bounds.y = attributes.y;
          bounds.width = attributes.width as u32;
          bounds.height = attributes.height as u32;
        }
      }
    } else if let Some(window) = &self.gtk_window {
      let position = window.position();
      let size = window.size();

      bounds.x = position.0;
      bounds.y = position.1;
      bounds.width = size.0 as u32;
      bounds.height = size.1 as u32;
    } else {
      let (size, _) = self.webview.allocated_size();
      bounds.width = size.width() as u32;
      bounds.height = size.height() as u32;
    }

    bounds
  }

  pub fn set_bounds(&self, bounds: Rect) {
    if self.is_child {
      if let Some(window) = &self.gtk_window {
        window.move_(bounds.x, bounds.y);
      }
    }

    if let Some(window) = &self.gtk_window {
      if self.is_child {
        window
          .window()
          .unwrap()
          .resize(bounds.width as i32, bounds.height as i32);
      }
      window.size_allocate(&gtk::Allocation::new(
        0,
        0,
        bounds.width as i32,
        bounds.height as i32,
      ));
    }

    if self.is_in_fixed_parent {
      self.webview.size_allocate(&gtk::Allocation::new(
        bounds.x,
        bounds.y,
        bounds.width as i32,
        bounds.height as i32,
      ));
    }
  }

  pub fn set_visible(&self, visible: bool) {
    if self.is_child {
      let xlib = self.xlib.as_ref().unwrap();
      if visible {
        unsafe { (xlib.XMapWindow)(self.x11_display.unwrap() as _, self.x11_window.unwrap()) };
      } else {
        unsafe { (xlib.XUnmapWindow)(self.x11_display.unwrap() as _, self.x11_window.unwrap()) };
      }
    }

    if visible {
      self.webview.show_all();
    } else {
      self.webview.hide();
    }

    if let Some(window) = &self.gtk_window {
      if visible {
        window.show_all();
      } else {
        window.hide();
      }
    }
  }

  pub fn focus(&self) {
    self.webview.grab_focus();
  }
}

pub fn platform_webview_version() -> Result<String> {
  let (major, minor, patch) = unsafe {
    (
      webkit_get_major_version(),
      webkit_get_minor_version(),
      webkit_get_micro_version(),
    )
  };
  Ok(format!("{}.{}.{}", major, minor, patch))
}

// SAFETY: only use this when you are sure the span will be dropped on the same thread it was entered
#[cfg(feature = "tracing")]
struct SendEnteredSpan(tracing::span::EnteredSpan);

#[cfg(feature = "tracing")]
unsafe impl Send for SendEnteredSpan {}
