// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{path::PathBuf, rc::Rc};

use gdk::{WindowEdge, WindowExt, RGBA};
use gio::Cancellable;
use glib::{signal::Inhibit, Bytes, Cast, FileError};
use gtk::{BoxExt, ContainerExt, WidgetExt};
use url::Url;
use uuid::Uuid;
use webkit2gtk::{
  ApplicationInfo, AutomationSessionExt, SecurityManagerExt, SettingsExt, URISchemeRequestExt,
  UserContentInjectedFrames, UserContentManager, UserContentManagerExt, UserScript,
  UserScriptInjectionTime, WebContextBuilder, WebContextExt, WebView, WebViewBuilder, WebViewExt,
  WebViewExtManual, WebsiteDataManagerBuilder,
};
use webkit2gtk_sys::{
  webkit_get_major_version, webkit_get_micro_version, webkit_get_minor_version,
};

use crate::{
  application::{gtk::ApplicationGtkExt, platform::unix::*, window::Window, Application},
  webview::{mimetype::MimeType, FileDropEvent, RpcRequest, RpcResponse},
  Error, Result,
};
use std::env::var;

mod file_drop;

pub struct InnerWebView {
  webview: Rc<WebView>,
}

impl InnerWebView {
  pub fn new(
    application: &Application,
    window: Rc<Window>,
    scripts: Vec<String>,
    url: Option<Url>,
    transparent: bool,
    custom_protocols: Vec<(
      String,
      Box<dyn Fn(&Window, &str) -> Result<Vec<u8>> + 'static>,
    )>,
    rpc_handler: Option<Box<dyn Fn(&Window, RpcRequest) -> Option<RpcResponse>>>,
    file_drop_handler: Option<Box<dyn Fn(&Window, FileDropEvent) -> bool>>,
  ) -> Result<Self> {
    let id = Uuid::new_v4().to_string();
    let window_rc = Rc::clone(&window);
    let window = &window.gtk_window();

    // Webview widget
    let manager = UserContentManager::new();
    let context = application.context();

    let automation = var("TAURI_AUTOMATION_MODE").as_deref() == Ok("1");
    let mut webview = WebViewBuilder::new();
    webview = webview.web_context(context);
    webview = webview.user_content_manager(&manager);
    webview = webview.is_controlled_by_automation(automation);
    let webview = webview.build();

    let auto_webview = webview.clone();
    context.connect_automation_started(move |_, auto| {
      let webview = auto_webview.clone();
      let app_into = ApplicationInfo::new();
      app_into.set_name("wry");
      app_into.set_version(0, 9, 0);
      auto.set_application_info(&app_into);
      auto.connect_create_web_view(move |auto| webview.clone());
    });

    // Message handler
    let webview = Rc::new(webview);
    let wv = Rc::clone(&webview);
    let w = window_rc.clone();
    manager.register_script_message_handler(&id);
    manager.connect_script_message_received(move |m2, msg| {
      if let (Some(js), Some(context)) = (msg.get_value(), msg.get_global_context()) {
        if let Some(js) = js.to_string(&context) {
          if let Some(rpc_handler) = rpc_handler.as_ref() {
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

      // Enable Smooth scrooling
      settings.set_enable_smooth_scrolling(true);

      debug_assert_eq!(
        {
          settings.set_enable_developer_extras(true);
        },
        ()
      );
    }

    // Transparent
    if transparent {
      webview.set_background_color(&RGBA {
        red: 0.,
        green: 0.,
        blue: 0.,
        alpha: 0.,
      });
    }

    // File drop handling
    if let Some(file_drop_handler) = file_drop_handler {
      file_drop::connect_drag_event(webview.clone(), window_rc.clone(), file_drop_handler);
    }

    if window.get_visible() {
      window.show_all();
    }

    let w = Self { webview };

    let mut init = String::with_capacity(67 + 36 + 20);
    init.push_str("window.external={invoke:function(x){window.webkit.messageHandlers[\"");
    init.push_str(&id);
    init.push_str("\"].postMessage(x);}}");

    // Initialize scripts
    w.init(&init)?;
    for js in scripts {
      w.init(&js)?;
    }

    // Custom protocol
    for (name, handler) in custom_protocols {
      context
        .get_security_manager()
        .ok_or(Error::MissingManager)?
        .register_uri_scheme_as_secure(&name);
      let w = window_rc.clone();
      context.register_uri_scheme(&name.clone(), move |request| {
        if let Some(uri) = request.get_uri() {
          let uri = uri.as_str();

          match handler(&w, uri) {
            Ok(buffer) => {
              let mime = MimeType::parse(&buffer, uri);
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
    if let Some(url) = url {
      w.webview.load_uri(url.as_str());
    }

    Ok(w)
  }

  // not supported yet
  pub fn print(&self) {}

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
}

pub fn platform_webview_version() -> Result<String> {
  let (major, minor, patch) = unsafe {
    (
      webkit_get_major_version(),
      webkit_get_minor_version(),
      webkit_get_micro_version(),
    )
  };
  Ok(format!("{}.{}.{}", major, minor, patch).into())
}
