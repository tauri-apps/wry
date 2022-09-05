// Copyright 2020-2022 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::webview::RGBA;
use crossbeam_channel::*;
use once_cell::sync::Lazy;
use std::os::unix::prelude::*;
use tao::platform::android::ndk_glue::{
  jni::{
    errors::Error as JniError,
    objects::{GlobalRef, JClass, JObject},
    JNIEnv,
  },
  PACKAGE,
};

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
            env.call_method(
              webview,
              "loadUrlMainThread",
              "(Ljava/lang/String;)V",
              &[url.into()],
            )?;
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
      }
    }
    Ok(())
  }
}

fn find_my_class<'a>(
  env: JNIEnv<'a>,
  activity: JObject<'a>,
  name: String,
) -> Result<JClass<'a>, JniError> {
  let class_name = env.new_string(name.replace('/', "."))?;
  let my_class = env
    .call_method(
      activity,
      "getAppClass",
      "(Ljava/lang/String;)Ljava/lang/Class;",
      &[class_name.into()],
    )?
    .l()?;
  Ok(my_class.into())
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

#[derive(Debug)]
pub enum WebViewMessage {
  CreateWebView(CreateWebViewAttributes),
  Eval(String),
  SetBackgroundColor(RGBA),
}

#[derive(Debug)]
pub struct CreateWebViewAttributes {
  pub url: String,
  pub devtools: bool,
  pub transparent: bool,
  pub background_color: Option<RGBA>,
}
