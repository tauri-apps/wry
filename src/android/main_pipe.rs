// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::{Error, RGBA};
use crossbeam_channel::*;
use jni::{
  errors::Result as JniResult,
  objects::{GlobalRef, JMap, JObject, JString},
  JNIEnv,
};
use once_cell::sync::Lazy;
use std::os::unix::prelude::*;

use super::{find_class, PACKAGE};

static CHANNEL: Lazy<(Sender<WebViewMessage>, Receiver<WebViewMessage>)> = Lazy::new(|| bounded(8));
pub static MAIN_PIPE: Lazy<[RawFd; 2]> = Lazy::new(|| {
  let mut pipe: [RawFd; 2] = Default::default();
  unsafe { libc::pipe(pipe.as_mut_ptr()) };
  pipe
});

pub struct MainPipe<'a> {
  pub env: JNIEnv<'a>,
  pub activity: GlobalRef,
  pub webview: Option<GlobalRef>,
  pub webchrome_client: GlobalRef,
}

impl<'a> MainPipe<'a> {
  pub(crate) fn send(message: WebViewMessage) {
    let size = std::mem::size_of::<bool>();
    if let Ok(()) = CHANNEL.0.send(message) {
      unsafe { libc::write(MAIN_PIPE[1], &true as *const _ as *const _, size) };
    }
  }

  pub fn recv(&mut self) -> JniResult<()> {
    let activity = self.activity.as_obj();
    if let Ok(message) = CHANNEL.1.recv() {
      match message {
        WebViewMessage::CreateWebView(attrs) => {
          let CreateWebViewAttributes {
            url,
            html,
            #[cfg(any(debug_assertions, feature = "devtools"))]
            devtools,
            transparent,
            background_color,
            headers,
            on_webview_created,
            autoplay,
            user_agent,
            ..
          } = attrs;
          // Create webview
          let rust_webview_class = find_class(
            &mut self.env,
            activity,
            format!("{}/RustWebView", PACKAGE.get().unwrap()),
          )?;
          let webview = self.env.new_object(
            &rust_webview_class,
            "(Landroid/content/Context;)V",
            &[activity.into()],
          )?;

          // set media autoplay
          self
            .env
            .call_method(&webview, "setAutoPlay", "(Z)V", &[autoplay.into()])?;

          // set user-agent
          if let Some(user_agent) = user_agent {
            let user_agent = self.env.new_string(user_agent)?;
            self.env.call_method(
              &webview,
              "setUserAgent",
              "(Ljava/lang/String;)V",
              &[(&user_agent).into()],
            )?;
          }

          self.env.call_method(
            activity,
            "setWebView",
            format!("(L{}/RustWebView;)V", PACKAGE.get().unwrap()),
            &[(&webview).into()],
          )?;

          // Navigation
          if let Some(u) = url {
            if let Ok(url) = self.env.new_string(u) {
              load_url(&mut self.env, &webview, &url, headers, true)?;
            }
          } else if let Some(h) = html {
            if let Ok(html) = self.env.new_string(h) {
              load_html(&mut self.env, &webview, &html)?;
            }
          }

          // Enable devtools
          #[cfg(any(debug_assertions, feature = "devtools"))]
          self.env.call_static_method(
            &rust_webview_class,
            "setWebContentsDebuggingEnabled",
            "(Z)V",
            &[devtools.into()],
          )?;

          if transparent {
            set_background_color(&mut self.env, &webview, (0, 0, 0, 0))?;
          } else if let Some(color) = background_color {
            set_background_color(&mut self.env, &webview, color)?;
          }

          // Create and set webview client
          let rust_webview_client_class = find_class(
            &mut self.env,
            activity,
            format!("{}/RustWebViewClient", PACKAGE.get().unwrap()),
          )?;
          let webview_client = self.env.new_object(
            &rust_webview_client_class,
            "(Landroid/content/Context;)V",
            &[activity.into()],
          )?;
          self.env.call_method(
            &webview,
            "setWebViewClient",
            "(Landroid/webkit/WebViewClient;)V",
            &[(&webview_client).into()],
          )?;

          // set webchrome client
          self.env.call_method(
            &webview,
            "setWebChromeClient",
            "(Landroid/webkit/WebChromeClient;)V",
            &[self.webchrome_client.as_obj().into()],
          )?;

          // Add javascript interface (IPC)
          let ipc_class = find_class(
            &mut self.env,
            activity,
            format!("{}/Ipc", PACKAGE.get().unwrap()),
          )?;
          let ipc = self.env.new_object(ipc_class, "()V", &[])?;
          let ipc_str = self.env.new_string("ipc")?;
          self.env.call_method(
            &webview,
            "addJavascriptInterface",
            "(Ljava/lang/Object;Ljava/lang/String;)V",
            &[(&ipc).into(), (&ipc_str).into()],
          )?;

          // Set content view
          self.env.call_method(
            activity,
            "setContentView",
            "(Landroid/view/View;)V",
            &[(&webview).into()],
          )?;

          if let Some(on_webview_created) = on_webview_created {
            if let Err(e) = on_webview_created(super::Context {
              env: &mut self.env,
              activity,
              webview: &webview,
            }) {
              log::warn!("failed to run webview created hook: {e}");
            }
          }

          let webview = self.env.new_global_ref(webview)?;

          self.webview = Some(webview);
        }
        WebViewMessage::Eval(script) => {
          if let Some(webview) = &self.webview {
            let s = self.env.new_string(script)?;
            self.env.call_method(
              webview.as_obj(),
              "evaluateJavascript",
              "(Ljava/lang/String;Landroid/webkit/ValueCallback;)V",
              &[(&s).into(), JObject::null().as_ref().into()],
            )?;
          }
        }
        WebViewMessage::SetBackgroundColor(background_color) => {
          if let Some(webview) = &self.webview {
            set_background_color(&mut self.env, webview.as_obj(), background_color)?;
          }
        }
        WebViewMessage::GetWebViewVersion(tx) => {
          match self
            .env
            .call_method(activity, "getVersion", "()Ljava/lang/String;", &[])
            .and_then(|v| v.l())
            .and_then(|s| {
              let s = JString::from(s);
              self
                .env
                .get_string(&s)
                .map(|v| v.to_string_lossy().to_string())
            }) {
            Ok(version) => {
              tx.send(Ok(version)).unwrap();
            }
            Err(e) => tx.send(Err(e.into())).unwrap(),
          }
        }
        WebViewMessage::GetUrl(tx) => {
          if let Some(webview) = &self.webview {
            let url = self
              .env
              .call_method(webview.as_obj(), "getUrl", "()Ljava/lang/String;", &[])
              .and_then(|v| v.l())
              .and_then(|s| {
                let s = JString::from(s);
                self
                  .env
                  .get_string(&s)
                  .map(|v| v.to_string_lossy().to_string())
              })
              .unwrap_or_default();

            tx.send(url).unwrap()
          }
        }
        WebViewMessage::Jni(f) => {
          if let Some(w) = &self.webview {
            f(&mut self.env, activity, w.as_obj());
          } else {
            f(&mut self.env, activity, &JObject::null());
          }
        }
        WebViewMessage::LoadUrl(url, headers) => {
          if let Some(webview) = &self.webview {
            let url = self.env.new_string(url)?;
            load_url(&mut self.env, webview.as_obj(), &url, headers, false)?;
          }
        }
        WebViewMessage::ClearAllBrowsingData => {
          if let Some(webview) = &self.webview {
            self
              .env
              .call_method(webview, "clearAllBrowsingData", "()V", &[])?;
          }
        }
      }
    }
    Ok(())
  }
}

fn load_url<'a>(
  env: &mut JNIEnv<'a>,
  webview: &JObject<'a>,
  url: &JString<'a>,
  headers: Option<http::HeaderMap>,
  main_thread: bool,
) -> JniResult<()> {
  let function = if main_thread {
    "loadUrlMainThread"
  } else {
    "loadUrl"
  };
  if let Some(headers) = headers {
    let obj = env.new_object("java/util/HashMap", "()V", &[])?;
    let headers_map = {
      let headers_map = JMap::from_env(env, &obj)?;
      for (name, value) in headers.iter() {
        let key = env.new_string(name)?;
        let value = env.new_string(value.to_str().unwrap_or_default())?;
        headers_map.put(env, &key, &value)?;
      }
      headers_map
    };
    env.call_method(
      webview,
      function,
      "(Ljava/lang/String;Ljava/util/Map;)V",
      &[url.into(), (&headers_map).into()],
    )?;
  } else {
    env.call_method(webview, function, "(Ljava/lang/String;)V", &[url.into()])?;
  }
  Ok(())
}

fn load_html<'a>(env: &mut JNIEnv<'a>, webview: &JObject<'a>, html: &JString<'a>) -> JniResult<()> {
  env.call_method(
    webview,
    "loadHTMLMainThread",
    "(Ljava/lang/String;)V",
    &[html.into()],
  )?;
  Ok(())
}

fn set_background_color<'a>(
  env: &mut JNIEnv<'a>,
  webview: &JObject<'a>,
  background_color: RGBA,
) -> JniResult<()> {
  let color_class = env.find_class("android/graphics/Color")?;
  let color = env.call_static_method(
    color_class,
    "argb",
    "(IIII)I",
    &[
      background_color.3.into(),
      background_color.0.into(),
      background_color.1.into(),
      background_color.2.into(),
    ],
  )?;
  env.call_method(webview, "setBackgroundColor", "(I)V", &[color.borrow()])?;
  Ok(())
}

pub(crate) enum WebViewMessage {
  CreateWebView(CreateWebViewAttributes),
  Eval(String),
  SetBackgroundColor(RGBA),
  GetWebViewVersion(Sender<Result<String, Error>>),
  GetUrl(Sender<String>),
  Jni(Box<dyn FnOnce(&mut JNIEnv, &JObject, &JObject) + Send>),
  LoadUrl(String, Option<http::HeaderMap>),
  ClearAllBrowsingData,
}

pub(crate) struct CreateWebViewAttributes {
  pub url: Option<String>,
  pub html: Option<String>,
  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub devtools: bool,
  pub transparent: bool,
  pub background_color: Option<RGBA>,
  pub headers: Option<http::HeaderMap>,
  pub autoplay: bool,
  pub on_webview_created: Option<Box<dyn Fn(super::Context) -> JniResult<()> + Send>>,
  pub user_agent: Option<String>,
}
