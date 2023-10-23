// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use gtk::{gdk::EventMask, gio::Cancellable, prelude::*};
#[cfg(any(debug_assertions, feature = "devtools"))]
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
  collections::hash_map::DefaultHasher,
  hash::{Hash, Hasher},
  rc::Rc,
  sync::{Arc, Mutex},
};
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

use web_context::WebContextExt;
pub use web_context::WebContextImpl;

use crate::{
  application::{platform::unix::*, window::Window},
  webview::{proxy::ProxyConfig, web_context::WebContext, PageLoadEvent, WebViewAttributes, RGBA},
  Error, Result,
};

mod file_drop;
mod synthetic_mouse_events;
mod undecorated_resizing;
mod web_context;

use javascriptcore::ValueExt;

pub(crate) struct InnerWebView {
  pub webview: Rc<WebView>,
  #[cfg(any(debug_assertions, feature = "devtools"))]
  is_inspector_open: Arc<AtomicBool>,
  pending_scripts: Arc<Mutex<Option<Vec<String>>>>,
}

impl InnerWebView {
  pub fn new(
    window: Rc<Window>,
    mut attributes: WebViewAttributes,
    _pl_attrs: super::PlatformSpecificWebViewAttributes,
    web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    let window_rc = Rc::clone(&window);
    let window = &window.gtk_window();

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
    let webview = Rc::new(webview);
    let w = window_rc.clone();
    let ipc_handler = attributes.ipc_handler.take();
    let manager = web_context.manager();
    // Use the window hash as the script handler name to prevent from conflict when sharing same
    // web context.
    let window_hash = {
      let mut hasher = DefaultHasher::new();
      w.id().hash(&mut hasher);
      hasher.finish().to_string()
    };

    // Connect before registering as recommended by the docs
    manager.connect_script_message_received(None, move |_m, msg| {
      if let Some(js) = msg.js_value() {
        if let Some(ipc_handler) = &ipc_handler {
          ipc_handler(&w, js.to_string());
        }
      }
    });

    // Register the handler we just connected
    manager.register_script_message_handler(&window_hash);

    // Allow the webview to close it's own window
    let close_window = window_rc.clone();
    webview.connect_close(move |_| {
      close_window.gtk_window().close();
    });

    // document title changed handler
    if let Some(document_title_changed_handler) = attributes.document_title_changed_handler {
      let w = window_rc.clone();
      webview.connect_title_notify(move |webview| {
        document_title_changed_handler(
          &w,
          webview.title().map(|t| t.to_string()).unwrap_or_default(),
        )
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
    undecorated_resizing::setup(&webview);

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

    // tao adds a default vertical box so we check for that first
    if let Some(vbox) = window_rc.default_vbox() {
      vbox.pack_start(&*webview, true, true, 0);
    } else {
      window.add(&*webview);
    }

    if attributes.focused {
      webview.grab_focus();
    }

    if let Some(context) = WebViewExt::context(&*webview) {
      use webkit2gtk::WebContextExt;
      context.set_use_system_appearance_for_scrollbars(false);
    }

    // Enable webgl, webaudio, canvas features as default.
    if let Some(settings) = WebViewExt::settings(&*webview) {
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
      file_drop::connect_drag_event(webview.clone(), window_rc, file_drop_handler);
    }

    if window.get_visible() {
      window.show_all();
    }

    #[cfg(any(debug_assertions, feature = "devtools"))]
    let is_inspector_open = {
      let is_inspector_open = Arc::new(AtomicBool::default());
      if let Some(inspector) = WebViewExt::inspector(&*webview) {
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
    };

    // Initialize message handler
    let mut init = String::with_capacity(115 + 20 + 22);
    init.push_str("Object.defineProperty(window, 'ipc', {value: Object.freeze({postMessage:function(x){window.webkit.messageHandlers[\"");
    init.push_str(&window_hash);
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
      web_context.queue_load_uri(Rc::clone(&w.webview), url, attributes.headers);
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

      match callback {
        Some(callback) => {
          self.webview.run_javascript(js, cancellable, |result| {
            let mut result_str = String::new();

            if let Ok(js_result) = result {
              if let Some(js_value) = js_result.js_value() {
                if let Some(json_str) = js_value.to_json(0) {
                  result_str = json_str.to_string();
                }
              }
            }

            callback(result_str);
          });
        }
        None => self.webview.run_javascript(js, cancellable, |_| ()),
      };
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
    if let Some(inspector) = WebViewExt::inspector(&*self.webview) {
      inspector.show();
      // `bring-to-front` is not received in this case
      self.is_inspector_open.store(true, Ordering::Relaxed);
    }
  }

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn close_devtools(&self) {
    if let Some(inspector) = WebViewExt::inspector(&*self.webview) {
      inspector.close();
    }
  }

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn is_devtools_open(&self) -> bool {
    self.is_inspector_open.load(Ordering::Relaxed)
  }

  pub fn zoom(&self, scale_factor: f64) {
    WebViewExt::set_zoom_level(&*self.webview, scale_factor);
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
    if let Some(context) = WebViewExt::context(&*self.webview) {
      use webkit2gtk::WebContextExt;
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
