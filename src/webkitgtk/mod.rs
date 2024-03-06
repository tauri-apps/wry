// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use gdkx11::{
  ffi::{gdk_x11_window_foreign_new_for_display, GdkX11Display},
  X11Display,
};
use gtk::{
  gdk::{self},
  gio::Cancellable,
  glib::{self, translate::FromGlibPtrFull},
  prelude::*,
};
use http::Request;
use javascriptcore::ValueExt;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
#[cfg(any(debug_assertions, feature = "devtools"))]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
#[cfg(any(debug_assertions, feature = "devtools"))]
use webkit2gtk::WebInspectorExt;
use webkit2gtk::{
  AutoplayPolicy, InputMethodContextExt, LoadEvent, NavigationPolicyDecision,
  NavigationPolicyDecisionExt, NetworkProxyMode, NetworkProxySettings, PolicyDecisionType,
  PrintOperationExt, SettingsExt, URIRequest, URIRequestExt, UserContentInjectedFrames,
  UserContentManagerExt, UserScript, UserScriptInjectionTime,
  WebContextExt as Webkit2gtkWeContextExt, WebView, WebViewExt, WebsiteDataManagerExt,
  WebsiteDataManagerExtManual, WebsitePolicies,
};
use webkit2gtk_sys::{
  webkit_get_major_version, webkit_get_micro_version, webkit_get_minor_version,
  webkit_policy_decision_ignore, webkit_policy_decision_use,
};
use x11_dl::xlib::*;

pub use web_context::WebContextImpl;

use crate::{
  proxy::ProxyConfig, web_context::WebContext, Error, PageLoadEvent, Rect, Result,
  WebViewAttributes, RGBA,
};

use self::web_context::WebContextExt;

mod file_drop;
mod synthetic_mouse_events;
mod web_context;

struct X11Data {
  is_child: bool,
  xlib: Xlib,
  x11_display: *mut std::ffi::c_void,
  x11_window: u64,
  gtk_window: gtk::Window,
}

impl Drop for X11Data {
  fn drop(&mut self) {
    unsafe { (self.xlib.XDestroyWindow)(self.x11_display as _, self.x11_window) };
    self.gtk_window.close();
  }
}

pub(crate) struct InnerWebView {
  pub webview: WebView,
  #[cfg(any(debug_assertions, feature = "devtools"))]
  is_inspector_open: Arc<AtomicBool>,
  pending_scripts: Arc<Mutex<Option<Vec<String>>>>,
  is_in_fixed_parent: bool,

  x11: Option<X11Data>,
}

impl Drop for InnerWebView {
  fn drop(&mut self) {
    unsafe { self.webview.destroy() }
  }
}

impl InnerWebView {
  pub fn new<W: HasWindowHandle>(
    window: &W,
    attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    Self::new_x11(window, attributes, pl_attrs, web_context, false)
  }

  pub fn new_as_child<W: HasWindowHandle>(
    parent: &W,
    attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    Self::new_x11(parent, attributes, pl_attrs, web_context, true)
  }

  fn new_x11<W: HasWindowHandle>(
    window: &W,
    attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    web_context: Option<&mut WebContext>,
    is_child: bool,
  ) -> Result<Self> {
    let parent = match window.window_handle()?.as_raw() {
      RawWindowHandle::Xlib(w) => w.window,
      _ => return Err(Error::UnsupportedWindowHandle),
    };

    let xlib = Xlib::open()?;

    let gdk_display = gdk::Display::default().ok_or(crate::Error::X11DisplayNotFound)?;
    let gx11_display: &X11Display = gdk_display.downcast_ref().unwrap();
    let raw = gx11_display.as_ptr();

    let x11_display = unsafe { gdkx11::ffi::gdk_x11_display_get_xdisplay(raw) };

    let x11_window = match is_child {
      true => Self::create_container_x11_window(&xlib, x11_display as _, parent, &attributes),
      false => parent,
    };

    let (gtk_window, vbox) = Self::create_gtk_window(raw, x11_window);

    let visible = attributes.visible;

    Self::new_gtk(&vbox, attributes, pl_attrs, web_context).map(|mut w| {
      // for some reason, if the webview starts as hidden,
      // we will need about 3 calls to `webview.set_visible`
      // with alternating value.
      // calling gtk_window.show_all() then hiding it again
      // seems to fix the issue.
      gtk_window.show_all();
      if !visible {
        let _ = w.set_visible(false);
      }

      w.x11.replace(X11Data {
        is_child,
        xlib,
        x11_display: x11_display as _,
        x11_window,
        gtk_window,
      });

      w
    })
  }

  fn create_container_x11_window(
    xlib: &Xlib,
    display: *mut _XDisplay,
    parent: u64,
    attributes: &WebViewAttributes,
  ) -> u64 {
    let window = unsafe {
      (xlib.XCreateSimpleWindow)(
        display,
        parent,
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
      unsafe { (xlib.XMapWindow)(display, window) };
    }

    window
  }

  pub fn create_gtk_window(raw: *mut GdkX11Display, x11_window: u64) -> (gtk::Window, gtk::Box) {
    // Gdk.Window
    let gdk_window = unsafe { gdk_x11_window_foreign_new_for_display(raw, x11_window) };
    let gdk_window = unsafe { gdk::Window::from_glib_full(gdk_window) };

    // Gtk.Window
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.connect_realize(glib::clone!(@weak gdk_window as wd => move |w| w.set_window(wd)));
    window.set_has_window(true);
    window.realize();

    // Gtk.Box (vertical)
    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
    window.add(&vbox);

    (window, vbox)
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
    if let Some(proxy_setting) = &attributes.proxy_config {
      let proxy_uri = match proxy_setting {
        ProxyConfig::Http(endpoint) => format!("http://{}:{}", endpoint.host, endpoint.port),
        ProxyConfig::Socks5(endpoint) => {
          format!("socks5://{}:{}", endpoint.host, endpoint.port)
        }
      };
      if let Some(website_data_manager) = web_context.context().website_data_manager() {
        let mut settings = NetworkProxySettings::new(Some(proxy_uri.as_str()), &[]);
        website_data_manager
          .set_network_proxy_settings(NetworkProxyMode::Custom, Some(&mut settings));
      }
    }

    let webview = Self::create_webview(web_context, &attributes);

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

    // Webview Settings
    Self::set_webview_settings(&webview, &attributes);

    // Webview handlers
    Self::attach_handlers(&webview, web_context, &mut attributes);

    // IPC handler
    Self::attach_ipc_handler(webview.clone(), web_context, &mut attributes);

    // File drop handler
    if let Some(file_drop_handler) = attributes.file_drop_handler.take() {
      file_drop::connect_drag_event(&webview, file_drop_handler);
    }

    web_context.register_automation(webview.clone());

    let is_in_fixed_parent = Self::add_to_container(&webview, container, &attributes);

    #[cfg(any(debug_assertions, feature = "devtools"))]
    let is_inspector_open = Self::attach_inspector_handlers(&webview);

    let w = Self {
      webview,
      pending_scripts: Arc::new(Mutex::new(Some(Vec::new()))),

      is_in_fixed_parent,
      x11: None,

      #[cfg(any(debug_assertions, feature = "devtools"))]
      is_inspector_open,
    };

    // Initialize message handler
    w.init("Object.defineProperty(window, 'ipc', { value: Object.freeze({ postMessage: function(x) { window.webkit.messageHandlers['ipc'].postMessage(x) } }) })")?;

    // Initialize scripts
    for js in attributes.initialization_scripts {
      w.init(&js)?;
    }

    // Run pending webview.eval() scripts once webview loads.
    let pending_scripts = w.pending_scripts.clone();
    w.webview.connect_load_changed(move |webview, event| {
      if let LoadEvent::Committed = event {
        let mut pending_scripts_ = pending_scripts.lock().unwrap();
        if let Some(pending_scripts) = pending_scripts_.take() {
          let cancellable: Option<&Cancellable> = None;
          for script in pending_scripts {
            webview.run_javascript(&script, cancellable, |_| ());
          }
        }
      }
    });

    // Custom protocols handler
    for (name, handler) in attributes.custom_protocols {
      web_context.register_uri_scheme(&name, handler)?;
    }

    // Navigation
    if let Some(url) = attributes.url {
      web_context.queue_load_uri(w.webview.clone(), url, attributes.headers);
      web_context.flush_queue_loader();
    } else if let Some(html) = attributes.html {
      w.webview.load_html(&html, None);
    }

    if attributes.visible {
      w.webview.show_all();
    }

    if attributes.focused {
      w.webview.grab_focus();
    }

    Ok(w)
  }

  fn create_webview(web_context: &WebContext, attributes: &WebViewAttributes) -> WebView {
    let mut builder = WebView::builder()
      .user_content_manager(web_context.manager())
      .web_context(web_context.context())
      .is_controlled_by_automation(web_context.allows_automation());

    if attributes.autoplay {
      builder = builder.website_policies(
        &WebsitePolicies::builder()
          .autoplay(AutoplayPolicy::Allow)
          .build(),
      );
    }

    builder.build()
  }

  fn set_webview_settings(webview: &WebView, attributes: &WebViewAttributes) {
    // Disable input preedit,fcitx input editor can anchor at edit cursor position
    if let Some(input_context) = webview.input_method_context() {
      input_context.set_enable_preedit(false);
    }

    // use system scrollbars
    if let Some(context) = webview.context() {
      context.set_use_system_appearance_for_scrollbars(false);
    }

    if let Some(settings) = WebViewExt::settings(webview) {
      // Enable webgl, webaudio, canvas features as default.
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

      // Devtools
      if attributes.devtools {
        settings.set_enable_developer_extras(true);
      }
    }
  }

  fn attach_handlers(
    webview: &WebView,
    web_context: &mut WebContext,
    attributes: &mut WebViewAttributes,
  ) {
    // Synthetic mouse events
    synthetic_mouse_events::setup(webview);

    // Document title changed handler
    if let Some(document_title_changed_handler) = attributes.document_title_changed_handler.take() {
      webview.connect_title_notify(move |webview| {
        let new_title = webview.title().map(|t| t.to_string()).unwrap_or_default();
        document_title_changed_handler(new_title)
      });
    }

    // Page load handler
    if let Some(on_page_load_handler) = attributes.on_page_load_handler.take() {
      webview.connect_load_changed(move |webview, load_event| match load_event {
        LoadEvent::Committed => {
          on_page_load_handler(PageLoadEvent::Started, webview.uri().unwrap().to_string());
        }
        LoadEvent::Finished => {
          on_page_load_handler(PageLoadEvent::Finished, webview.uri().unwrap().to_string());
        }
        _ => (),
      });
    }

    // Navigation handler && New window handler
    if attributes.navigation_handler.is_some() || attributes.new_window_req_handler.is_some() {
      let new_window_req_handler = attributes.new_window_req_handler.take();
      let navigation_handler = attributes.navigation_handler.take();

      webview.connect_decide_policy(move |_webview, policy_decision, policy_type| {
        let handler = match policy_type {
          PolicyDecisionType::NavigationAction => &navigation_handler,
          PolicyDecisionType::NewWindowAction => &new_window_req_handler,
          _ => return false,
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

                  return true;
                }
              }
            }
          }
        }

        false
      });
    }

    // Download handler
    if attributes.download_started_handler.is_some()
      || attributes.download_completed_handler.is_some()
    {
      web_context.register_download_handler(
        attributes.download_started_handler.take(),
        attributes.download_completed_handler.take(),
      )
    }
  }

  fn add_to_container<W>(webview: &WebView, container: &W, attributes: &WebViewAttributes) -> bool
  where
    W: IsA<gtk::Container>,
  {
    let mut is_in_fixed_parent = false;

    let container_type = container.type_().name();
    if container_type == "GtkBox" {
      container
        .dynamic_cast_ref::<gtk::Box>()
        .unwrap()
        .pack_start(webview, true, true, 0);
    } else if container_type == "GtkFixed" {
      webview.set_size_request(
        attributes.bounds.map(|s| s.width).unwrap_or(1) as i32,
        attributes.bounds.map(|s| s.height).unwrap_or(1) as i32,
      );

      container.dynamic_cast_ref::<gtk::Fixed>().unwrap().put(
        webview,
        attributes.bounds.map(|p| p.x).unwrap_or(0),
        attributes.bounds.map(|p| p.y).unwrap_or(0),
      );

      is_in_fixed_parent = true;
    } else {
      container.add(webview);
    }

    is_in_fixed_parent
  }

  fn attach_ipc_handler(
    webview: WebView,
    web_context: &WebContext,
    attributes: &mut WebViewAttributes,
  ) {
    // Message handler
    let ipc_handler = attributes.ipc_handler.take();
    let manager = web_context.manager();

    // Connect before registering as recommended by the docs
    manager.connect_script_message_received(None, move |_m, msg| {
      #[cfg(feature = "tracing")]
      let _span = tracing::info_span!("wry::ipc::handle").entered();

      if let Some(js) = msg.js_value() {
        if let Some(ipc_handler) = &ipc_handler {
          ipc_handler(
            Request::builder()
              .uri(webview.uri().unwrap().to_string())
              .body(js.to_string())
              .unwrap(),
          );
        }
      }
    });

    // Register the handler we just connected
    manager.register_script_message_handler("ipc");
  }

  #[cfg(any(debug_assertions, feature = "devtools"))]
  fn attach_inspector_handlers(webview: &WebView) -> Arc<AtomicBool> {
    let is_inspector_open = Arc::new(AtomicBool::default());
    if let Some(inspector) = webview.inspector() {
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
  }

  pub fn print(&self) -> Result<()> {
    let print = webkit2gtk::PrintOperation::new(&self.webview);
    print.run_dialog(None::<&gtk::Window>);
    Ok(())
  }

  pub fn url(&self) -> Result<String> {
    Ok(self.webview.uri().unwrap_or_default().to_string())
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
          let result = result
            .map(|r| r.js_value().and_then(|js| js.to_json(0)))
            .unwrap_or_default()
            .unwrap_or_default()
            .to_string();

          callback(result);
        }
      });
    }

    Ok(())
  }

  fn init(&self, js: &str) -> Result<()> {
    if let Some(manager) = self.webview.user_content_manager() {
      let script = UserScript::new(
        js,
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
    if let Some(inspector) = self.webview.inspector() {
      inspector.show();
      // `bring-to-front` is not received in this case
      self.is_inspector_open.store(true, Ordering::Relaxed);
    }
  }

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn close_devtools(&self) {
    if let Some(inspector) = self.webview.inspector() {
      inspector.close();
    }
  }

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn is_devtools_open(&self) -> bool {
    self.is_inspector_open.load(Ordering::Relaxed)
  }

  pub fn zoom(&self, scale_factor: f64) -> Result<()> {
    self.webview.set_zoom_level(scale_factor);
    Ok(())
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

  pub fn load_url(&self, url: &str) -> Result<()> {
    self.webview.load_uri(url);
    Ok(())
  }

  pub fn load_url_with_headers(&self, url: &str, headers: http::HeaderMap) -> Result<()> {
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

    Ok(())
  }

  pub fn clear_all_browsing_data(&self) -> Result<()> {
    if let Some(context) = self.webview.context() {
      if let Some(data_manger) = context.website_data_manager() {
        data_manger.clear(
          webkit2gtk::WebsiteDataTypes::ALL,
          gtk::glib::TimeSpan::from_seconds(0),
          None::<&Cancellable>,
          |_| {},
        );
      }
    }

    Ok(())
  }

  pub fn bounds(&self) -> Result<Rect> {
    let mut bounds = Rect::default();

    if let Some(x11_data) = &self.x11 {
      unsafe {
        let attributes: XWindowAttributes = std::mem::zeroed();
        let mut attributes = std::mem::MaybeUninit::new(attributes).assume_init();

        let ok = (x11_data.xlib.XGetWindowAttributes)(
          x11_data.x11_display as _,
          x11_data.x11_window,
          &mut attributes,
        );

        if ok != 0 {
          bounds.x = attributes.x;
          bounds.y = attributes.y;
          bounds.width = attributes.width as u32;
          bounds.height = attributes.height as u32;
        }
      }
    } else {
      let (size, _) = self.webview.allocated_size();
      bounds.width = size.width() as u32;
      bounds.height = size.height() as u32;
    }

    Ok(bounds)
  }

  pub fn set_bounds(&self, bounds: Rect) -> Result<()> {
    if let Some(x11_data) = &self.x11 {
      let window = &x11_data.gtk_window;
      window.move_(bounds.x, bounds.y);
      if let Some(window) = window.window() {
        window.resize(bounds.width as i32, bounds.height as i32);
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

    Ok(())
  }

  fn set_visible_x11(&self, visible: bool) {
    if let Some(x11_data) = &self.x11 {
      if x11_data.is_child {
        if visible {
          unsafe { (x11_data.xlib.XMapWindow)(x11_data.x11_display as _, x11_data.x11_window) };
        } else {
          unsafe { (x11_data.xlib.XUnmapWindow)(x11_data.x11_display as _, x11_data.x11_window) };
        }
      }
    }
  }

  fn set_visible_gtk(&self, visible: bool) {
    if let Some(x11_data) = &self.x11 {
      if x11_data.is_child {
        if visible {
          x11_data.gtk_window.show_all();
        } else {
          x11_data.gtk_window.hide();
        }
      }
    }
  }

  pub fn set_visible(&self, visible: bool) -> Result<()> {
    self.set_visible_x11(visible);

    if visible {
      self.webview.show_all();
    } else {
      self.webview.hide();
    }

    self.set_visible_gtk(visible);

    Ok(())
  }

  pub fn focus(&self) -> Result<()> {
    self.webview.grab_focus();
    Ok(())
  }

  pub fn reparent<W>(&self, container: &W) -> Result<()>
  where
    W: gtk::prelude::IsA<gtk::Container>,
  {
    if let Some(parent) = self
      .webview
      .parent()
      .and_then(|p| p.dynamic_cast::<gtk::Container>().ok())
    {
      parent.remove(&self.webview);

      let container_type = container.type_().name();
      if container_type == "GtkBox" {
        container
          .dynamic_cast_ref::<gtk::Box>()
          .unwrap()
          .pack_start(&self.webview, true, true, 0);
      } else if container_type == "GtkFixed" {
        container
          .dynamic_cast_ref::<gtk::Fixed>()
          .unwrap()
          .put(&self.webview, 0, 0);
      } else {
        container.add(&self.webview);
      }
    }

    Ok(())
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
