use std::{path::PathBuf, rc::Rc};

use gdk::RGBA;
use gio::Cancellable;
use glib::{Bytes, FileError};
use gtk::{ApplicationWindow as Window, ContainerExt, WidgetExt};
use url::Url;
use webkit2gtk::{
  SecurityManagerExt, SettingsExt, URISchemeRequestExt, UserContentInjectedFrames,
  UserContentManager, UserContentManagerExt, UserScript, UserScriptInjectionTime, WebContext,
  WebContextExt, WebView, WebViewExt, WebViewExtManual,
};

use crate::{
  Error,
  Result, RpcHandler, webview::{mimetype::MimeType, WV},
};
use crate::webview::FileDropHandler;

mod file_drop;

pub struct InnerWebView {
  webview: Rc<WebView>,
}

impl WV for InnerWebView {
  type Window = Window;

  fn new<F: 'static + Fn(&str) -> Result<Vec<u8>>>(
    window: &Window,
    scripts: Vec<String>,
    url: Option<Url>,
    transparent: bool,
    custom_protocol: Option<(String, F)>,
    rpc_handler: Option<RpcHandler>,
    file_drop_handler: Option<FileDropHandler>,
    _user_data_path: Option<PathBuf>,
  ) -> Result<Self> {
    // Webview widget
    let manager = UserContentManager::new();
    let context = WebContext::new();
    let webview = Rc::new(WebView::new_with_context_and_user_content_manager(
      &context, &manager,
    ));

    // Message handler
    let wv = Rc::clone(&webview);
    manager.register_script_message_handler("external");
    manager.connect_script_message_received(move |_m, msg| {
      if let (Some(js), Some(context)) = (msg.get_value(), msg.get_global_context()) {
        if let Some(js) = js.to_string(&context) {
          if let Some(rpc_handler) = rpc_handler.as_ref() {
            match super::rpc_proxy(js, rpc_handler) {
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

    window.add(&*webview);
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
          settings.set_enable_write_console_messages_to_stdout(true);
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
      file_drop::connect_drag_event(webview.clone(), file_drop_handler);
    }

    if window.get_visible() {
      window.show_all();
    }

    let w = Self { webview };

    // Initialize scripts
    w.init("window.external={invoke:function(x){window.webkit.messageHandlers.external.postMessage(x);}}")?;
    for js in scripts {
      w.init(&js)?;
    }

    // Custom protocol
    if let Some((name, handler)) = custom_protocol {
      context
        .get_security_manager()
        .ok_or(Error::MissingManager)?
        .register_uri_scheme_as_secure(&name);
      context.register_uri_scheme(&name.clone(), move |request| {
        if let Some(uri) = request.get_uri() {
          let uri = uri.as_str();

          match handler(uri) {
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

  fn eval(&self, js: &str) -> Result<()> {
    let cancellable: Option<&Cancellable> = None;
    self.webview.run_javascript(js, cancellable, |_| ());
    Ok(())
  }
}

impl InnerWebView {
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
