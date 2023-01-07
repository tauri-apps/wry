// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::{webview::RGBA, Error};
use crossbeam_channel::*;
use once_cell::sync::Lazy;
use std::os::unix::prelude::*;
use tao::platform::android::ndk_glue::{
  jni::{
    errors::Error as JniError,
    objects::{GlobalRef, JObject, JString},
    JNIEnv,
  },
  PACKAGE,
};

use super::{create_headers_map, find_my_class};

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

impl MainPipe<'_> {
  pub fn send(message: WebViewMessage) {
    let size = std::mem::size_of::<bool>();
    if let Ok(()) = CHANNEL.0.send(message) {
      unsafe { libc::write(MAIN_PIPE[1], &true as *const _ as *const _, size) };
    }
  }

  pub fn recv(&mut self) -> Result<(), JniError> {
    let env = self.env;
    let activity = self.activity.as_obj();
    if let Ok(message) = CHANNEL.1.recv() {
      match message {
        WebViewMessage::CreateWebView(attrs) => {
          let CreateWebViewAttributes {
            url,
            devtools,
            transparent,
            background_color,
            headers,
          } = attrs;
          // Create webview
          let rust_webview_class = find_my_class(
            env,
            activity,
            format!("{}/RustWebView", PACKAGE.get().unwrap()),
          )?;
          let webview = env.new_object(
            rust_webview_class,
            "(Landroid/content/Context;)V",
            &[activity.into()],
          )?;

          // Load URL
          if let Ok(url) = env.new_string(url) {
            load_url(env, webview, url, headers, true)?;
          }

          // Enable devtools
          #[cfg(any(debug_assertions, feature = "devtools"))]
          env.call_static_method(
            rust_webview_class,
            "setWebContentsDebuggingEnabled",
            "(Z)V",
            &[devtools.into()],
          )?;

          if transparent {
            set_background_color(env, webview, (0, 0, 0, 0))?;
          } else {
            if let Some(color) = background_color {
              set_background_color(env, webview, color)?;
            }
          }

          // Create and set webview client
          let rust_webview_client_class = find_my_class(
            env,
            activity,
            format!("{}/RustWebViewClient", PACKAGE.get().unwrap()),
          )?;
          let webview_client = env.new_object(rust_webview_client_class, "()V", &[])?;
          env.call_method(
            webview,
            "setWebViewClient",
            "(Landroid/webkit/WebViewClient;)V",
            &[webview_client.into()],
          )?;

          // set webchrome client
          env.call_method(
            webview,
            "setWebChromeClient",
            "(Landroid/webkit/WebChromeClient;)V",
            &[self.webchrome_client.as_obj().into()],
          )?;

          // Add javascript interface (IPC)
          let ipc_class = find_my_class(env, activity, format!("{}/Ipc", PACKAGE.get().unwrap()))?;
          let ipc = env.new_object(ipc_class, "()V", &[])?;
          let ipc_str = env.new_string("ipc")?;
          env.call_method(
            webview,
            "addJavascriptInterface",
            "(Ljava/lang/Object;Ljava/lang/String;)V",
            &[ipc.into(), ipc_str.into()],
          )?;

          // Set content view
          env.call_method(
            activity,
            "setContentView",
            "(Landroid/view/View;)V",
            &[webview.into()],
          )?;
          let webview = env.new_global_ref(webview)?;
          self.webview = Some(webview);
        }
        WebViewMessage::Eval(script) => {
          if let Some(webview) = &self.webview {
            let s = env.new_string(script)?;
            env.call_method(
              webview.as_obj(),
              "evaluateJavascript",
              "(Ljava/lang/String;Landroid/webkit/ValueCallback;)V",
              &[s.into(), JObject::null().into()],
            )?;
          }
        }
        WebViewMessage::SetBackgroundColor(background_color) => {
          if let Some(webview) = &self.webview {
            set_background_color(env, webview.as_obj(), background_color)?;
          }
        }
        WebViewMessage::GetWebViewVersion(tx) => {
          match env
            .call_method(activity, "getVersion", "()Ljava/lang/String;", &[])
            .and_then(|v| v.l())
            .and_then(|s| env.get_string(s.into()))
          {
            Ok(version) => {
              tx.send(Ok(version.to_string_lossy().into())).unwrap();
            }
            Err(e) => tx.send(Err(e.into())).unwrap(),
          }
        }
        WebViewMessage::GetUrl(tx) => {
          if let Some(webview) = &self.webview {
            let url = env
              .call_method(webview.as_obj(), "getUrl", "()Ljava/lang/String", &[])
              .and_then(|v| v.l())
              .and_then(|s| env.get_string(s.into()))
              .map(|u| u.to_string_lossy().into())
              .unwrap_or_default();

            tx.send(url).unwrap()
          }
        }
        WebViewMessage::Jni(f) => {
          if let Some(webview) = &self.webview {
            f(env, activity, webview.as_obj());
          }
        }
        WebViewMessage::LoadUrl(url, headers) => {
          if let Some(webview) = &self.webview {
            let url = env.new_string(url)?;
            load_url(env, webview.as_obj(), url, headers, false)?;
          }
        }
      }
    }
    Ok(())
  }
}

fn load_url<'a>(
  env: JNIEnv<'a>,
  webview: JObject<'a>,
  url: JString<'a>,
  headers: Option<http::HeaderMap>,
  main_thread: bool,
) -> Result<(), JniError> {
  let function = if main_thread {
    "loadUrlMainThread"
  } else {
    "loadUrl"
  };
  if let Some(headers) = headers {
    let headers_map = create_headers_map(&env, &headers)?;
    env.call_method(
      webview,
      function,
      "(Ljava/lang/String;Ljava/util/Map;)V",
      &[url.into(), headers_map.into()],
    )?;
  } else {
    env.call_method(webview, function, "(Ljava/lang/String;)V", &[url.into()])?;
  }
  Ok(())
}

fn set_background_color<'a>(
  env: JNIEnv<'a>,
  webview: JObject<'a>,
  background_color: RGBA,
) -> Result<(), JniError> {
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
  env.call_method(webview, "setBackgroundColor", "(I)V", &[color])?;
  Ok(())
}

pub enum WebViewMessage {
  CreateWebView(CreateWebViewAttributes),
  Eval(String),
  SetBackgroundColor(RGBA),
  GetWebViewVersion(Sender<Result<String, Error>>),
  GetUrl(Sender<String>),
  Jni(Box<dyn FnOnce(JNIEnv, JObject, JObject) + Send>),
  LoadUrl(String, Option<http::HeaderMap>),
}

#[derive(Debug)]
pub struct CreateWebViewAttributes {
  pub url: String,
  pub devtools: bool,
  pub transparent: bool,
  pub background_color: Option<RGBA>,
  pub headers: Option<http::HeaderMap>,
}
