// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use gdk::{Cursor, EventMask, WindowEdge};
use gio::Cancellable;
use glib::signal::Inhibit;
use gtk::prelude::*;
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
  traits::*, LoadEvent, NavigationPolicyDecision, PolicyDecisionType, URIRequest,
  UserContentInjectedFrames, UserContentManager, UserScript, UserScriptInjectionTime, WebView,
  WebViewBuilder,
};
use webkit2gtk_sys::{
  webkit_get_major_version, webkit_get_micro_version, webkit_get_minor_version,
  webkit_policy_decision_ignore, webkit_policy_decision_use,
};

use web_context::WebContextExt;
pub use web_context::WebContextImpl;

use crate::{
  application::{platform::unix::*, window::Window},
  webview::{web_context::WebContext, WebViewAttributes, RGBA},
  Error, Result,
};

mod file_drop;
mod web_context;

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
    pl_attrs: super::PlatformSpecificWebViewAttributes,
    web_context: Option<&mut WebContext>,
  ) -> Result<Self> {
    let window_rc = Rc::clone(&window);
    let window = &window.gtk_window();

    // default_context allows us to create a scoped context on-demand
    let mut default_context;
    let web_context = match web_context {
      Some(w) => w,
      None => {
        default_context = Default::default();
        &mut default_context
      }
    };

    let manager = UserContentManager::new();

    let webview = {
      let mut webview = WebViewBuilder::new();
      webview = webview.user_content_manager(&manager);
      webview = webview.web_context(web_context.context());
      webview = webview.is_controlled_by_automation(web_context.allows_automation());
      webview.build()
    };

    web_context.register_automation(webview.clone());

    // Message handler
    let webview = Rc::new(webview);
    let w = window_rc.clone();
    let ipc_handler = attributes.ipc_handler.take();
    // Use the window hash as the script handler name to prevent from conflict when sharing same
    // web context.
    let window_hash = {
      let mut hasher = DefaultHasher::new();
      w.id().hash(&mut hasher);
      hasher.finish().to_string()
    };

    // Connect before registering as recommended by the docs
    manager.connect_script_message_received(None, move |_m, msg| {
      #[cfg(feature = "tracing")]
      let _span = tracing::info_span!(parent: None, "wry::ipc::handle").entered();

      if let Some(js) = msg.js_value() {
        if let Some(ipc_handler) = &ipc_handler {
          ipc_handler(&w, js.to_string());
        }
      }
    });

    // Register handler on JS side
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

    webview.add_events(
      EventMask::POINTER_MOTION_MASK
        | EventMask::BUTTON1_MOTION_MASK
        | EventMask::BUTTON_PRESS_MASK
        | EventMask::TOUCH_MASK,
    );
    webview.connect_motion_notify_event(|webview, event| {
      // This one should be GtkWindow
      if let Some(widget) = webview.parent() {
        // This one should be GtkWindow
        if let Some(window) = widget.parent() {
          // Safe to unwrap unless this is not from tao
          let window: gtk::Window = window.downcast().unwrap();
          if !window.is_decorated() && window.is_resizable() {
            if let Some(window) = window.window() {
              let (cx, cy) = event.root();
              let edge = hit_test(&window, cx, cy);
              // FIXME: calling `window.begin_resize_drag` seems to revert the cursor back to normal style
              window.set_cursor(
                Cursor::from_name(
                  &window.display(),
                  match edge {
                    WindowEdge::North => "n-resize",
                    WindowEdge::South => "s-resize",
                    WindowEdge::East => "e-resize",
                    WindowEdge::West => "w-resize",
                    WindowEdge::NorthWest => "nw-resize",
                    WindowEdge::NorthEast => "ne-resize",
                    WindowEdge::SouthEast => "se-resize",
                    WindowEdge::SouthWest => "sw-resize",
                    _ => "default",
                  },
                )
                .as_ref(),
              );
            }
          }
        }
      }
      Inhibit(false)
    });
    webview.connect_button_press_event(|webview, event| {
      if event.button() == 1 {
        let (cx, cy) = event.root();
        // This one should be GtkBox
        if let Some(widget) = webview.parent() {
          // This one should be GtkWindow
          if let Some(window) = widget.parent() {
            // Safe to unwrap unless this is not from tao
            let window: gtk::Window = window.downcast().unwrap();
            if !window.is_decorated() && window.is_resizable() {
              if let Some(window) = window.window() {
                // Safe to unwrap since it's a valid GtkWindow
                let result = hit_test(&window, cx, cy);

                // we ignore the `__Unknown` variant so the webview receives the click correctly if it is not on the edges.
                match result {
                  WindowEdge::__Unknown(_) => (),
                  _ => window.begin_resize_drag(result, 1, cx as i32, cy as i32, event.time()),
                }
              }
            }
          }
        }
      }
      Inhibit(false)
    });
    webview.connect_touch_event(|webview, event| {
      // This one should be GtkBox
      if let Some(widget) = webview.parent() {
        // This one should be GtkWindow
        if let Some(window) = widget.parent() {
          // Safe to unwrap unless this is not from tao
          let window: gtk::Window = window.downcast().unwrap();
          if !window.is_decorated() && window.is_resizable() && !window.is_maximized() {
            if let Some(window) = window.window() {
              if let Some((cx, cy)) = event.root_coords() {
                if let Some(device) = event.device() {
                  let result = hit_test(&window, cx, cy);

                  // we ignore the `__Unknown` variant so the window receives the click correctly if it is not on the edges.
                  match result {
                    WindowEdge::__Unknown(_) => (),
                    _ => window.begin_resize_drag_for_device(
                      result,
                      &device,
                      0,
                      cx as i32,
                      cy as i32,
                      event.time(),
                    ),
                  }
                }
              }
            }
          }
        }
      }
      Inhibit(false)
    });

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

    // Gtk application window can only contain one widget at a time.
    // In tao, we add a GtkBox to pack menu bar. So we check if
    // there's a box widget here.
    if let Some(widget) = window.children().pop() {
      let vbox = widget.downcast::<gtk::Box>().unwrap();
      vbox.pack_start(&*webview, true, true, 0);
    }

    if pl_attrs.focused {
      webview.grab_focus();
    }

    // Enable webgl, webaudio, canvas features as default.
    if let Some(settings) = WebViewExt::settings(&*webview) {
      settings.set_enable_webgl(true);
      settings.set_enable_webaudio(true);

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
      webview.set_background_color(&gdk::RGBA::new(0., 0., 0., 0.));
    } else {
      // background color
      if let Some(background_color) = attributes.background_color {
        webview.set_background_color(&gdk::RGBA::new(
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
      match web_context.try_register_uri_scheme(&name, handler) {
        // Swallow duplicate scheme errors to preserve current behavior.
        Err(Error::DuplicateCustomProtocol(_)) => {
          log::warn!("Ignoring already registered custom protocol {name}");
        }
        Err(e) => return Err(e),
        Ok(_) => (),
      }
    }

    // Navigation
    if let Some(url) = attributes.url {
      web_context.queue_load_uri(Rc::clone(&w.webview), url, attributes.headers);
      web_context.flush_queue_loader();
    } else if let Some(html) = attributes.html {
      w.webview.load_html(&html, Some("http://localhost"));
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
    let _ = self.eval("window.print()");
  }

  pub fn url(&self) -> Url {
    let uri = self.webview.uri().unwrap();

    Url::parse(uri.as_str()).unwrap()
  }

  pub fn eval(&self, js: &str) -> Result<()> {
    if let Some(pending_scripts) = &mut *self.pending_scripts.lock().unwrap() {
      pending_scripts.push(js.into());
    } else {
      let cancellable: Option<&Cancellable> = None;
      #[cfg(feature = "tracing")]
      {
        let span = SendEnteredSpan(tracing::debug_span!("wry::eval").entered());
        self.webview.run_javascript(js, cancellable, move |_| {
          drop(span);
        });
      }
      #[cfg(not(feature = "tracing"))]
      {
        self.webview.run_javascript(js, cancellable, |_| ());
      }
    }
    Ok(())
  }

  fn init(&self, js: &str) -> Result<()> {
    if let Some(manager) = self.webview.user_content_manager() {
      let script = UserScript::new(
        js,
        // TODO: feature to allow injecting into subframes
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
    self.webview.set_background_color(&gdk::RGBA::new(
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
