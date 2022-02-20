// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{
  collections::hash_map::DefaultHasher,
  hash::{Hash, Hasher},
  rc::Rc,
};

use gdk::{Cursor, EventMask, WindowEdge, RGBA};
use gio::Cancellable;
use glib::signal::Inhibit;
use gtk::prelude::*;
use webkit2gtk::{
  traits::*, UserContentInjectedFrames, UserScript, UserScriptInjectionTime, WebView,
  WebViewBuilder,
};
use webkit2gtk_sys::{
  webkit_get_major_version, webkit_get_micro_version, webkit_get_minor_version,
};

use web_context::WebContextExt;
pub use web_context::WebContextImpl;

use crate::{
  application::{platform::unix::*, window::Window},
  webview::{web_context::WebContext, WebViewAttributes},
  Error, Result,
};

mod file_drop;
mod web_context;

pub struct InnerWebView {
  webview: Rc<WebView>,
}

impl InnerWebView {
  pub fn new(
    window: Rc<Window>,
    mut attributes: WebViewAttributes,
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

    let webview = {
      let mut webview = WebViewBuilder::new();
      webview = webview.user_content_manager(web_context.manager());
      webview = webview.web_context(web_context.context());
      webview = webview.is_controlled_by_automation(web_context.allows_automation());
      webview.build()
    };

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
                // Safe to unwrap since it's a valide GtkWindow
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
          if !window.is_decorated() && window.is_resizable() {
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

    // Gtk application window can only contain one widget at a time.
    // In tao, we add a GtkBox to pack menu bar. So we check if
    // there's a box widget here.
    if let Some(widget) = window.children().pop() {
      let vbox = widget.downcast::<gtk::Box>().unwrap();
      vbox.pack_start(&*webview, true, true, 0);
    }
    webview.grab_focus();

    // Enable webgl, webaudio, canvas features as default.
    if let Some(settings) = WebViewExt::settings(&*webview) {
      settings.set_enable_webgl(true);
      settings.set_enable_webaudio(true);
      settings.set_enable_accelerated_2d_canvas(true);

      // Enable clipboard
      if attributes.clipboard {
        settings.set_javascript_can_access_clipboard(true);
      }

      // Enable App cache
      settings.set_enable_offline_web_application_cache(true);
      settings.set_enable_page_cache(true);

      // Set user agent
      settings.set_user_agent(attributes.user_agent.as_deref());

      if attributes.devtool {
        settings.set_enable_developer_extras(true);
      }
    }

    // Transparent
    if attributes.transparent {
      webview.set_background_color(&RGBA::new(0., 0., 0., 0.));
    }

    // File drop handling
    if let Some(file_drop_handler) = attributes.file_drop_handler {
      file_drop::connect_drag_event(webview.clone(), window_rc, file_drop_handler);
    }

    if window.get_visible() {
      window.show_all();
    }

    let w = Self { webview };

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
      web_context.queue_load_uri(Rc::clone(&w.webview), url);
      web_context.flush_queue_loader();
    } else if let Some(html) = attributes.html {
      w.webview.load_html(&html, Some("http://localhost"));
    }

    Ok(w)
  }

  pub fn print(&self) {
    let _ = self.eval("window.print()");
  }

  pub fn eval(&self, js: &str) -> Result<()> {
    let cancellable: Option<&Cancellable> = None;
    self.webview.run_javascript(js, cancellable, |_| ());
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

  pub fn focus(&self) {
    self.webview.grab_focus();
  }

  /// Open the web inspector which is usually called dev tool.
  pub fn devtool(&self) {
    if let Some(inspector) = WebViewExt::inspector(&*self.webview) {
      inspector.show();
    }
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
