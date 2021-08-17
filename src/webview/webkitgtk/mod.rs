// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use gdk::{WindowEdge, RGBA};
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

use crate::{
  application::{platform::unix::*, window::Window},
  webview::{
    web_context::{unix::WebContextExt, WebContext},
    WebViewAttributes,
  },
  Error, Result,
};
use std::{
  collections::hash_map::DefaultHasher,
  hash::{Hash, Hasher},
};

mod file_drop;

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
    let wv = Rc::clone(&webview);
    let w = window_rc.clone();
    let rpc_handler = attributes.rpc_handler.take();

    // Use the window hash as the script handler name
    let window_hash = {
      let mut hasher = DefaultHasher::new();
      w.id().hash(&mut hasher);
      hasher.finish().to_string()
    };

    let manager = web_context.manager();

    // Connect before registering as recommended by the docs
    manager.connect_script_message_received(None, move |_m, msg| {
      if let (Some(js), Some(context)) = (msg.value(), msg.global_context()) {
        if let Some(js) = js.to_string(&context) {
          if let Some(rpc_handler) = &rpc_handler {
            match super::rpc_proxy(&w, js, rpc_handler) {
              Ok(result) => {
                let script = result.unwrap_or_default();
                let cancellable: Option<&Cancellable> = None;
                wv.run_javascript(&script, cancellable, |_| ());
              }
              Err(e) => {
                eprintln!("{}", e);
              }
            }
          }
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
              // Safe to unwrap since it's a valide GtkWindow
              let result = hit_test(&window.window().unwrap(), cx, cy);

              // we ignore the `__Unknown` variant so the webview receives the click correctly if it is not on the edges.
              match result {
                WindowEdge::__Unknown(_) => (),
                _ => window.begin_resize_drag(result, 1, cx as i32, cy as i32, event.time()),
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

    // Enable webgl, webaudio, canvas features and others as default.
    if let Some(settings) = WebViewExt::settings(&*webview) {
      settings.set_enable_webgl(true);
      settings.set_enable_webaudio(true);
      settings.set_enable_accelerated_2d_canvas(true);
      settings.set_javascript_can_access_clipboard(true);

      // Enable App cache
      settings.set_enable_offline_web_application_cache(true);
      settings.set_enable_page_cache(true);

      debug_assert_eq!(
        {
          settings.set_enable_developer_extras(true);
        },
        ()
      );
    }

    // Transparent
    if attributes.transparent {
      webview.set_background_color(&RGBA {
        red: 0.,
        green: 0.,
        blue: 0.,
        alpha: 0.,
      });
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
    let mut init = String::with_capacity(67 + 20 + 20);
    init.push_str("window.external={invoke:function(x){window.webkit.messageHandlers[\"");
    init.push_str(&window_hash);
    init.push_str("\"].postMessage(x);}}");
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
