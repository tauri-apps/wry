// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use dpi::LogicalSize;
use gtk::{
  gio::Cancellable,
  glib::{self},
  prelude::*,
};
use http::Request;
use raw_window_handle::HasWindowHandle;
#[cfg(any(debug_assertions, feature = "devtools"))]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use webkit::{
  prelude::*, AutoplayPolicy, LoadEvent, NavigationPolicyDecision, NetworkProxyMode,
  NetworkProxySettings, PolicyDecisionType, URIRequest, UserContentInjectedFrames,
  UserContentManager, UserScript, UserScriptInjectionTime, WebView, WebsitePolicies,
};

pub use web_context::WebContextImpl;

use crate::{
  proxy::ProxyConfig, web_context::WebContext, Error, PageLoadEvent, Rect, Result,
  WebViewAttributes, RGBA,
};

use self::web_context::WebContextExt;

const WEBVIEW_ID: &str = "webview_id";

mod drag_drop;
mod synthetic_mouse_events;
mod web_context;

pub(crate) struct InnerWebView {
  id: String,
  pub webview: WebView,
  #[cfg(any(debug_assertions, feature = "devtools"))]
  is_inspector_open: Arc<AtomicBool>,
  pending_scripts: Arc<Mutex<Option<Vec<String>>>>,
  is_in_fixed_parent: bool,
  gtk_window: Option<gtk::Window>,
}

impl InnerWebView {
  pub fn new(
    _window: &impl HasWindowHandle,
    attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
  ) -> Result<Self> {
    let (gtk_window, vbox) = Self::create_gtk_window();

    let visible = attributes.visible;

    Self::new_gtk(&vbox, attributes, pl_attrs).map(|mut w| {
      // for some reason, if the webview starts as hidden,
      // we will need about 3 calls to `webview.set_visible`
      // with alternating value.
      // calling gtk_window.show_all() then hiding it again
      // seems to fix the issue.
      gtk_window.present();
      if !visible {
        let _ = w.set_visible(false);
      }

      w.gtk_window = Some(gtk_window);

      w
    })
  }

  pub fn new_as_child(
    _window: &impl HasWindowHandle,
    attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
  ) -> Result<Self> {
    Self::new(_window, attributes, pl_attrs)
  }

  pub fn create_gtk_window() -> (gtk::Window, gtk::Box) {
    // Gtk.Window
    let window = gtk::Window::new();

    // Gtk.Box (vertical)
    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
    window.set_child(Some(&vbox));

    (window, vbox)
  }

  pub fn new_gtk<W>(
    container: &W,
    mut attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
  ) -> Result<Self>
  where
    W: gtk::prelude::IsA<gtk::Widget>,
  {
    // default_context allows us to create a scoped context on-demand
    let mut default_context;
    let web_context = if attributes.incognito {
      default_context = WebContext::new_ephemeral();
      &mut default_context
    } else {
      match attributes.context.take() {
        Some(w) => w,
        None => {
          default_context = Default::default();
          &mut default_context
        }
      }
    };

    web_context
      .context()
      .connect_initialize_web_process_extensions(move |context| {
        // Extension loading
        if let Some(extension_path) = &pl_attrs.extension_path {
          context.set_web_process_extensions_directory(&extension_path.to_string_lossy());
        }
      });

    if let Some(proxy_setting) = &attributes.proxy_config {
      let proxy_uri = match proxy_setting {
        ProxyConfig::Http(endpoint) => format!("http://{}:{}", endpoint.host, endpoint.port),
        ProxyConfig::Socks5(endpoint) => {
          format!("socks5://{}:{}", endpoint.host, endpoint.port)
        }
      };
      let network_session = web_context.network_session();
      let settings = NetworkProxySettings::new(Some(proxy_uri.as_str()), &[]);
      network_session.set_proxy_settings(NetworkProxyMode::Custom, Some(&settings));
    }

    let webview = Self::create_webview(web_context, &attributes);
    // Webview will forever be invisible if we don't set vexpand before rendering.
    // Webkit bug maybe??
    webview.set_vexpand(true);

    // Transparent
    if attributes.transparent {
      webview.set_background_color(&gtk::gdk::RGBA::new(0., 0., 0., 0.));
    } else {
      // background color
      if let Some((red, green, blue, alpha)) = attributes.background_color {
        webview.set_background_color(&gtk::gdk::RGBA::new(
          red as _, green as _, blue as _, alpha as _,
        ));
      }
    }

    // Webview Settings
    Self::set_webview_settings(&webview, &attributes);

    // Webview handlers
    Self::attach_handlers(&webview, web_context, &mut attributes);

    // IPC handler
    Self::attach_ipc_handler(webview.clone(), &mut attributes);

    // Drag drop handler
    if let Some(drag_drop_handler) = attributes.drag_drop_handler.take() {
      drag_drop::connect_drag_event(&webview, drag_drop_handler);
    }

    web_context.register_automation(webview.clone());

    let is_in_fixed_parent = Self::add_to_container(&webview, container, attributes.bounds)?;

    #[cfg(any(debug_assertions, feature = "devtools"))]
    let is_inspector_open = Self::attach_inspector_handlers(&webview);

    let id = attributes
      .id
      .map(|id| id.to_string())
      .unwrap_or_else(|| (webview.as_ptr() as isize).to_string());
    unsafe { webview.set_data(WEBVIEW_ID, id.clone()) };

    let w = Self {
      id,
      webview,
      pending_scripts: Arc::new(Mutex::new(Some(Vec::new()))),
      is_in_fixed_parent,
      #[cfg(any(debug_assertions, feature = "devtools"))]
      is_inspector_open,
      gtk_window: None,
    };

    // Initialize message handler
    w.init("Object.defineProperty(window, 'ipc', { value: Object.freeze({ postMessage: function(x) { window.webkit.messageHandlers['ipc'].postMessage(x) } }) })", true)?;

    // Initialize scripts
    for (js, for_main_only) in attributes.initialization_scripts {
      w.init(&js, for_main_only)?;
    }

    // Run pending webview.eval() scripts once webview loads.
    let pending_scripts = w.pending_scripts.clone();
    w.webview.connect_load_changed(move |webview, event| {
      if let LoadEvent::Committed = event {
        let mut pending_scripts_ = pending_scripts.lock().unwrap();
        if let Some(pending_scripts) = pending_scripts_.take() {
          let cancellable: Option<&Cancellable> = None;
          for script in pending_scripts {
            webview.evaluate_javascript(&script, None, None, cancellable, |_| ());
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

    if !attributes.visible {
      w.webview.set_visible(false);
    }

    if attributes.focused {
      w.webview.grab_focus();
    }

    Ok(w)
  }

  fn create_webview(web_context: &WebContext, attributes: &WebViewAttributes) -> WebView {
    let mut builder = WebView::builder()
      .user_content_manager(&UserContentManager::new())
      .web_context(web_context.context())
      .network_session(web_context.network_session())
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
      settings.set_enable_page_cache(true);

      // Set user agent
      settings.set_user_agent(attributes.user_agent.as_deref());

      // Devtools
      if attributes.devtools {
        settings.set_enable_developer_extras(true);
      }

      if attributes.javascript_disabled {
        settings.set_enable_javascript(false);
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
            if let Some(mut nav_action) = policy.navigation_action() {
              if let Some(uri_req) = nav_action.request() {
                if let Some(uri) = uri_req.uri() {
                  let allow = handler(uri.to_string());
                  if allow {
                    policy_decision.use_();
                  } else {
                    policy_decision.ignore();
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

  fn attach_ipc_handler(webview: WebView, attributes: &mut WebViewAttributes) {
    // Message handler
    let ipc_handler = attributes.ipc_handler.take();
    let manager = webview
      .user_content_manager()
      .expect("WebView does not have UserContentManager");

    // Connect before registering as recommended by the docs
    manager.connect_script_message_received(None, move |_m, msg| {
      #[cfg(feature = "tracing")]
      let _span = tracing::info_span!(parent: None, "wry::ipc::handle").entered();

      if let Some(ipc_handler) = &ipc_handler {
        ipc_handler(
          Request::builder()
            .uri(webview.uri().unwrap().to_string())
            .body(msg.to_string())
            .unwrap(),
        );
      }
    });

    // Register the handler we just connected
    manager.register_script_message_handler("ipc", None);
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

  pub fn id(&self) -> crate::WebViewId {
    &self.id
  }

  pub fn print(&self) -> Result<()> {
    let print = webkit::PrintOperation::new(&self.webview);
    print.run_dialog(gtk::Window::NONE);
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

      self
        .webview
        .evaluate_javascript(js, None, None, cancellable, |result| {
          #[cfg(feature = "tracing")]
          drop(span);

          if let Some(callback) = callback {
            let result = result
              .map(|r| r.to_json(0))
              .unwrap_or_default()
              .unwrap_or_default()
              .to_string();

            callback(result);
          }
        });
    }

    Ok(())
  }

  fn init(&self, js: &str, for_main_only: bool) -> Result<()> {
    if let Some(manager) = self.webview.user_content_manager() {
      let script = UserScript::new(
        js,
        if for_main_only {
          UserContentInjectedFrames::TopFrame
        } else {
          UserContentInjectedFrames::AllFrames
        },
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

  pub fn set_background_color(&self, (red, green, blue, alpha): RGBA) -> Result<()> {
    self.webview.set_background_color(&gtk::gdk::RGBA::new(
      red as _, green as _, blue as _, alpha as _,
    ));
    Ok(())
  }

  pub fn load_url(&self, url: &str) -> Result<()> {
    self.webview.load_uri(url);
    Ok(())
  }

  pub fn load_url_with_headers(&self, url: &str, headers: http::HeaderMap) -> Result<()> {
    let req = URIRequest::new(url);

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

  pub fn load_html(&self, html: &str) -> Result<()> {
    self.webview.load_html(html, None);
    Ok(())
  }

  pub fn reload(&self) -> Result<()> {
    self.webview.reload();
    Ok(())
  }

  pub fn clear_all_browsing_data(&self) -> Result<()> {
    if let Some(network_session) = self.webview.network_session() {
      if let Some(data_manger) = network_session.website_data_manager() {
        data_manger.clear(
          webkit::WebsiteDataTypes::ALL,
          gtk::glib::TimeSpan::from_seconds(0),
          Cancellable::NONE,
          |_| {},
        );
      }
    }

    Ok(())
  }

  pub fn bounds(&self) -> Result<Rect> {
    let bounds = Rect {
      size: LogicalSize::new(self.webview.width(), self.webview.height()).into(),
      ..Default::default()
    };

    Ok(bounds)
  }

  pub fn set_bounds(&self, bounds: Rect) -> Result<()> {
    let scale_factor = self.webview.scale_factor() as f64;
    let (width, height) = bounds.size.to_logical::<i32>(scale_factor).into();
    let (x, y) = bounds.position.to_logical::<i32>(scale_factor).into();

    if let Some(gtk_window) = &self.gtk_window {
      gtk_window.set_default_width(width);
      gtk_window.set_default_height(height);
    }

    if self.is_in_fixed_parent {
      self
        .webview
        .size_allocate(&gtk::Allocation::new(x, y, width, height), -1);
    }

    Ok(())
  }

  fn set_visible_gtk(&self, visible: bool) {
    if let Some(gtk_window) = &self.gtk_window {
      gtk_window.set_visible(visible);
    }
  }

  pub fn set_visible(&self, visible: bool) -> Result<()> {
    self.webview.set_visible(visible);

    self.set_visible_gtk(visible);

    Ok(())
  }

  pub fn focus(&self) -> Result<()> {
    self.webview.grab_focus();
    Ok(())
  }

  pub fn focus_parent(&self) -> Result<()> {
    if let Some(window) = self.webview.root() {
      if let Some(t) = window
        .surface()
        .and_then(|s| s.downcast::<gtk::gdk::Toplevel>().ok())
      {
        t.focus(gtk::gdk::ffi::GDK_CURRENT_TIME as _)
      }
    }

    Ok(())
  }

  fn cookie_from_soup_cookie(mut cookie: soup::Cookie) -> cookie::Cookie<'static> {
    let name = cookie.name().map(|n| n.to_string()).unwrap_or_default();
    let value = cookie.value().map(|n| n.to_string()).unwrap_or_default();

    let mut cookie_builder = cookie::CookieBuilder::new(name, value);

    if let Some(domain) = cookie.domain().map(|n| n.to_string()) {
      cookie_builder = cookie_builder.domain(domain);
    }

    if let Some(path) = cookie.path().map(|n| n.to_string()) {
      cookie_builder = cookie_builder.path(path);
    }

    let http_only = cookie.is_http_only();
    cookie_builder = cookie_builder.http_only(http_only);

    let secure = cookie.is_secure();
    cookie_builder = cookie_builder.secure(secure);

    let same_site = cookie.same_site_policy();
    let same_site = match same_site {
      soup::SameSitePolicy::Lax => cookie::SameSite::Lax,
      soup::SameSitePolicy::Strict => cookie::SameSite::Strict,
      soup::SameSitePolicy::None => cookie::SameSite::None,
      _ => cookie::SameSite::None,
    };
    cookie_builder = cookie_builder.same_site(same_site);

    let expires = cookie.expires();
    let expires = match expires {
      Some(datetime) => cookie::time::OffsetDateTime::from_unix_timestamp(datetime.to_unix())
        .ok()
        .map(cookie::Expiration::DateTime),
      None => Some(cookie::Expiration::Session),
    };
    if let Some(expires) = expires {
      cookie_builder = cookie_builder.expires(expires);
    }

    cookie_builder.build()
  }

  pub fn cookies_for_url(&self, url: &str) -> Result<Vec<cookie::Cookie<'static>>> {
    let (tx, rx) = std::sync::mpsc::channel();
    if let Some(cookies_manager) = self
      .webview
      .network_session()
      .and_then(|network_session| network_session.cookie_manager())
    {
      cookies_manager.cookies(url, Cancellable::NONE, move |cookies| {
        let cookies = cookies.map(|cookies| {
          cookies
            .into_iter()
            .map(Self::cookie_from_soup_cookie)
            .collect()
        });
        let _ = tx.send(cookies);
      })
    }

    let main_context = glib::MainContext::default();

    loop {
      main_context.iteration(true);

      if let Ok(response) = rx.try_recv() {
        return response.map_err(Into::into);
      }
    }
  }

  pub fn cookies(&self) -> Result<Vec<cookie::Cookie<'static>>> {
    let (tx, rx) = std::sync::mpsc::channel();
    if let Some(cookies_manager) = self
      .webview
      .network_session()
      .and_then(|network_session| network_session.cookie_manager())
    {
      cookies_manager.all_cookies(Cancellable::NONE, move |cookies| {
        let cookies = cookies.map(|cookies| {
          cookies
            .into_iter()
            .map(Self::cookie_from_soup_cookie)
            .collect()
        });
        let _ = tx.send(cookies);
      })
    }

    let main_context = glib::MainContext::default();

    loop {
      main_context.iteration(true);

      if let Ok(response) = rx.try_recv() {
        return response.map_err(Into::into);
      }
    }
  }

  fn add_to_container<W>(webview: &WebView, container: &W, bounds: Option<Rect>) -> Result<bool>
  where
    W: IsA<gtk::Widget>,
  {
    let mut is_in_fixed_parent = false;
    if let Some(c) = container.dynamic_cast_ref::<gtk::Window>() {
      c.set_child(Some(webview));
    } else if let Some(c) = container.dynamic_cast_ref::<gtk::Box>() {
      c.append(webview);
    } else if let Some(c) = container.dynamic_cast_ref::<gtk::Fixed>() {
      let scale_factor = webview.scale_factor() as f64;
      let (width, height) = bounds
        .map(|b| b.size.to_logical::<i32>(scale_factor))
        .map(Into::into)
        .unwrap_or((1, 1));
      let (x, y) = bounds
        .map(|b| b.position.to_logical::<f64>(scale_factor))
        .map(Into::into)
        .unwrap_or((0., 0.));

      // This seems to be the same bug where the webview is invisible without vexpand
      // GtkFixed ignores vexpand though so we need this here.
      webview.set_size_request(1, 1);
      webview.size_allocate(&gtk::Allocation::new(x as _, y as _, width, height), -1);
      c.put(webview, x, y);

      is_in_fixed_parent = true;
    } else {
      return Err(Error::UnsupportedParentWidget(
        container.type_().to_string(),
      ));
    }
    Ok(is_in_fixed_parent)
  }

  pub fn reparent<W>(&mut self, container: &W) -> Result<()>
  where
    W: IsA<gtk::Widget>,
  {
    if let Some(parent) = self.webview.parent() {
      if let Some(p) = parent.dynamic_cast_ref::<gtk::Window>() {
        p.set_child(gtk::Widget::NONE);
      } else if let Some(p) = parent.dynamic_cast_ref::<gtk::Box>() {
        p.remove(&self.webview);
      } else if let Some(p) = parent.dynamic_cast_ref::<gtk::Fixed>() {
        p.remove(&self.webview);
      } else {
        return Err(Error::UnsupportedParentWidget(parent.type_().to_string()));
      }
    }

    self.is_in_fixed_parent = Self::add_to_container(&self.webview, container, self.bounds().ok())?;
    Ok(())
  }
}

pub fn platform_webview_version() -> Result<String> {
  let (major, minor, patch) = (
    webkit::functions::major_version(),
    webkit::functions::minor_version(),
    webkit::functions::micro_version(),
  );
  Ok(format!("{major}.{minor}.{patch}"))
}

// SAFETY: only use this when you are sure the span will be dropped on the same thread it was entered
#[cfg(feature = "tracing")]
struct SendEnteredSpan(tracing::span::EnteredSpan);

#[cfg(feature = "tracing")]
unsafe impl Send for SendEnteredSpan {}
