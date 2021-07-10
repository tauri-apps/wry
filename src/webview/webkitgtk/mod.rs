// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::rc::Rc;

use gdk::{WindowEdge, WindowExt, RGBA};
use gio::Cancellable;
use glib::{signal::Inhibit, Bytes, Cast, FileError};
use gtk::{BoxExt, ContainerExt, GtkWindowExt, WidgetExt};
use webkit2gtk::{
  AutomationSessionExt, SecurityManagerExt, SettingsExt, URISchemeRequestExt,
  UserContentInjectedFrames, UserContentManager, UserContentManagerExt, UserScript,
  UserScriptInjectionTime, WebContextExt as WebKitWebContextExt, WebView, WebViewBuilder,
  WebViewExt,
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

mod file_drop;

pub struct InnerWebView {
  webview: Rc<WebView>,
}

impl InnerWebView {
  pub fn new(
    window: Rc<Window>,
    mut attributes: WebViewAttributes,
    web_context: Option<&WebContext>,
  ) -> Result<Self> {
    let window_rc = Rc::clone(&window);
    let window = &window.gtk_window();
    // Webview widget
    let manager = UserContentManager::new();

    let default_context;
    let web_context = match web_context {
      Some(w) => w,
      None => {
        default_context = Default::default();
        &default_context
      }
    };
    let context = web_context.context();
    let mut webview = WebViewBuilder::new();
    webview = webview.web_context(context);
    webview = webview.user_content_manager(&manager);
    webview = webview.is_controlled_by_automation(web_context.allows_automation());
    let webview = webview.build();

    let automation_webview = webview.clone();
    let app_info = web_context.app_info().clone();
    context.connect_automation_started(move |_, auto| {
      let webview = automation_webview.clone();
      auto.set_application_info(&app_info);
      auto.connect_create_web_view(move |_| webview.clone());
    });

    // Message handler
    let webview = Rc::new(webview);
    let wv = Rc::clone(&webview);
    let w = window_rc.clone();
    let rpc_handler = attributes.rpc_handler.take();
    manager.register_script_message_handler("external");
    manager.connect_script_message_received(move |_m, msg| {
      if let (Some(js), Some(context)) = (msg.get_value(), msg.get_global_context()) {
        if let Some(js) = js.to_string(&context) {
          if let Some(rpc_handler) = &rpc_handler {
            match super::rpc_proxy(&w, js, rpc_handler) {
              Ok(result) => {
                if let Some(ref script) = result {
                  let cancellable: Option<&Cancellable> = None;
                  wv.run_javascript(script, cancellable, |_| ());
                }
              }
              Err(e) => {
                eprintln!("{}", e);
              }
            }
          }
        }
      }
    });

    let close_window = window_rc.clone();
    webview.connect_close(move |_| {
      close_window.gtk_window().close();
    });

    webview.connect_button_press_event(|webview, event| {
      if event.get_button() == 1 {
        let (cx, cy) = event.get_root();
        if let Some(window) = webview.get_parent_window() {
          let result = crate::application::platform::unix::hit_test(&window, cx, cy);

          // this check is necessary, otherwise the webview won't recieve the click properly when resize isn't needed
          if result != WindowEdge::__Unknown(8) {
            window.begin_resize_drag(result, 1, cx as i32, cy as i32, event.get_time());
          }
        }
      }
      Inhibit(false)
    });

    // Gtk application window can only contain one widget at a time.
    // In tao, we add a gtk box if menu bar is required. So we check if
    // there's a box widget here.
    if let Some(widget) = window.get_children().pop() {
      let vbox = widget.downcast::<gtk::Box>().unwrap();
      vbox.pack_start(&*webview, true, true, 0);
    } else {
      window.add(&*webview);
    }
    webview.grab_focus();

    // Enable webgl, webaudio, canvas features and others as default.
    if let Some(settings) = WebViewExt::get_settings(&*webview) {
      settings.set_enable_webgl(true);
      settings.set_enable_webaudio(true);
      settings.set_enable_accelerated_2d_canvas(true);
      settings.set_javascript_can_access_clipboard(true);

      // Enable App cache
      settings.set_enable_offline_web_application_cache(true);
      settings.set_enable_page_cache(true);
      // Set user agent
      settings.set_user_agent(attributes.user_agent.as_deref());

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
      file_drop::connect_drag_event(webview.clone(), window_rc.clone(), file_drop_handler);
    }

    if window.get_visible() {
      window.show_all();
    }

    let w = Self { webview };

    // Initialize scripts
    w.init("window.external={invoke:function(x){window.webkit.messageHandlers.external.postMessage(x);}}")?;
    for js in attributes.initialization_scripts {
      w.init(&js)?;
    }

    // Custom protocol
    for (name, handler) in attributes.custom_protocols {
      context
        .get_security_manager()
        .ok_or(Error::MissingManager)?
        .register_uri_scheme_as_secure(&name);
      let w = window_rc.clone();
      context.register_uri_scheme(&name.clone(), move |request| {
        if let Some(uri) = request.get_uri() {
          let uri = uri.as_str();

          match handler(&w, uri) {
            Ok((buffer, mime)) => {
              let input = gio::MemoryInputStream::from_bytes(&Bytes::from(&buffer));
              request.finish(&input, buffer.len() as i64, Some(&mime))
            }
            Err(_) => request.finish_error(&mut glib::Error::new(
              FileError::Exist,
              "Could not get requested file.",
            )),
          }
        } else {
          request.finish_error(&mut glib::Error::new(
            FileError::Exist,
            "Could not get uri.",
          ));
        }
      });
    }

    // Navigation
    if let Some(url) = attributes.url {
      w.webview.load_uri(url.as_str());
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
    if let Some(manager) = self.webview.get_user_content_manager() {
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
