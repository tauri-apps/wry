// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use http::{
  header::{HeaderName, HeaderValue, CONTENT_TYPE},
  Request,
};
pub use tao::platform::android::ndk_glue::jni::sys::{jboolean, jstring};
use tao::platform::android::ndk_glue::jni::{
  errors::Error as JniError,
  objects::{JClass, JMap, JObject, JString},
  sys::jobject,
  JNIEnv,
};

use super::{
  ASSET_LOADER_DOMAIN, IPC, ON_LOAD_HANDLER, REQUEST_HANDLER, TITLE_CHANGE_HANDLER,
  URL_LOADING_OVERRIDE, WITH_ASSET_LOADER,
};

use crate::webview::PageLoadEvent;

fn handle_request(env: &mut JNIEnv, request: JObject) -> Result<jobject, JniError> {
  let mut request_builder = Request::builder();

  let uri = env
    .call_method(&request, "getUrl", "()Landroid/net/Uri;", &[])?
    .l()?;
  let url: JString = env
    .call_method(&uri, "toString", "()Ljava/lang/String;", &[])?
    .l()?
    .into();
  request_builder = request_builder.uri(&env.get_string(&url)?.to_string_lossy().to_string());

  let method: JString = env
    .call_method(&request, "getMethod", "()Ljava/lang/String;", &[])?
    .l()?
    .into();
  request_builder = request_builder.method(
    env
      .get_string(&method)?
      .to_string_lossy()
      .to_string()
      .as_str(),
  );

  let request_headers = env
    .call_method(request, "getRequestHeaders", "()Ljava/util/Map;", &[])?
    .l()?;
  let request_headers = JMap::from_env(env, &request_headers)?;
  let mut iter = request_headers.iter(env)?;
  while let Some((header, value)) = iter.next(env)? {
    let header = JString::from(header);
    let value = JString::from(value);
    let header = env.get_string(&header)?;
    let value = env.get_string(&value)?;
    if let (Ok(header), Ok(value)) = (
      HeaderName::from_bytes(header.to_bytes()),
      HeaderValue::from_bytes(value.to_bytes()),
    ) {
      request_builder = request_builder.header(header, value);
    }
  }

  if let Some(handler) = REQUEST_HANDLER.get() {
    let final_request = match request_builder.body(Vec::new()) {
      Ok(req) => req,
      Err(e) => {
        log::warn!("Failed to build response: {}", e);
        return Ok(*JObject::null());
      }
    };
    let response = (handler.0)(final_request);
    if let Some(response) = response {
      let status = response.status();
      let status_code = status.as_u16() as i32;
      let status_err = if status_code < 100 {
        Some("Status code can't be less than 100")
      } else if status_code > 599 {
        Some("statusCode can't be greater than 599.")
      } else if status_code > 299 && status_code < 400 {
        Some("statusCode can't be in the [300, 399] range.")
      } else {
        None
      };
      if let Some(err) = status_err {
        log::warn!("{}", err);
        return Ok(*JObject::null());
      }

      let reason_phrase = status.canonical_reason().unwrap_or("OK");
      let (mime_type, encoding) = if let Some(content_type) = response.headers().get(CONTENT_TYPE) {
        let content_type = content_type.to_str().unwrap().trim();
        let mut s = content_type.split(';');
        let mime_type = s.next().unwrap().trim();
        let mut encoding = None;
        for token in s {
          let token = token.trim();
          if token.starts_with("charset=") {
            encoding.replace(token.split('=').nth(1).unwrap());
            break;
          }
        }
        (
          env.new_string(mime_type)?,
          if let Some(encoding) = encoding {
            env.new_string(encoding)?
          } else {
            JString::default()
          },
        )
      } else {
        (JString::default(), JString::default())
      };

      let headers = response.headers();
      let obj = env.new_object("java/util/HashMap", "()V", &[])?;
      let response_headers = {
        let headers_map = JMap::from_env(env, &obj)?;
        for (name, value) in headers.iter() {
          let key = env.new_string(name)?;
          let value = env.new_string(value.to_str().unwrap_or_default())?;
          headers_map.put(env, &key, &value)?;
        }
        headers_map
      };

      let bytes = response.body();

      let byte_array_input_stream = env.find_class("java/io/ByteArrayInputStream")?;
      let byte_array = env.byte_array_from_slice(bytes)?;
      let stream = env.new_object(byte_array_input_stream, "([B)V", &[(&byte_array).into()])?;

      let reason_phrase = env.new_string(reason_phrase)?;

      let web_resource_response_class = env.find_class("android/webkit/WebResourceResponse")?;
      let web_resource_response = env.new_object(
        web_resource_response_class,
        "(Ljava/lang/String;Ljava/lang/String;ILjava/lang/String;Ljava/util/Map;Ljava/io/InputStream;)V",
        &[(&mime_type).into(), (&encoding).into(), status_code.into(), (&reason_phrase).into(), (&response_headers).into(), (&stream).into()],
      )?;

      return Ok(*web_resource_response);
    }
  }
  Ok(*JObject::null())
}

#[allow(non_snake_case)]
pub unsafe fn handleRequest(mut env: JNIEnv, _: JClass, request: JObject) -> jobject {
  match handle_request(&mut env, request) {
    Ok(response) => response,
    Err(e) => {
      log::warn!("Failed to handle request: {}", e);
      JObject::null().as_raw()
    }
  }
}

#[allow(non_snake_case)]
pub unsafe fn shouldOverride(mut env: JNIEnv, _: JClass, url: JString) -> jboolean {
  match env.get_string(&url) {
    Ok(url) => {
      let url = url.to_string_lossy().to_string();
      URL_LOADING_OVERRIDE
        .get()
        // We negate the result of the function because the logic for the android
        // client is different from how the navigation_handler is defined.
        //
        // https://developer.android.com/reference/android/webkit/WebViewClient#shouldOverrideUrlLoading(android.webkit.WebView,%20android.webkit.WebResourceRequest)
        .map(|f| !f.0(url))
        .unwrap_or(false)
    }
    Err(e) => {
      log::warn!("Failed to parse JString: {}", e);
      false
    }
  }
  .into()
}

pub unsafe fn ipc(mut env: JNIEnv, _: JClass, arg: JString) {
  match env.get_string(&arg) {
    Ok(arg) => {
      let arg = arg.to_string_lossy().to_string();
      if let Some(w) = IPC.get() {
        (w.0)(&w.1, arg)
      }
    }
    Err(e) => log::warn!("Failed to parse JString: {}", e),
  }
}

#[allow(non_snake_case)]
pub unsafe fn handleReceivedTitle(mut env: JNIEnv, _: JClass, _webview: JObject, title: JString) {
  match env.get_string(&title) {
    Ok(title) => {
      let title = title.to_string_lossy().to_string();
      if let Some(w) = TITLE_CHANGE_HANDLER.get() {
        (w.0)(&w.1, title)
      }
    }
    Err(e) => log::warn!("Failed to parse JString: {}", e),
  }
}

#[allow(non_snake_case)]
pub unsafe fn withAssetLoader(_: JNIEnv, _: JClass) -> jboolean {
  (*WITH_ASSET_LOADER.get().unwrap_or(&false)).into()
}

#[allow(non_snake_case)]
pub unsafe fn assetLoaderDomain(env: JNIEnv, _: JClass) -> jstring {
  if let Some(domain) = ASSET_LOADER_DOMAIN.get() {
    env.new_string(domain).unwrap().as_raw()
  } else {
    env.new_string("wry.assets").unwrap().as_raw()
  }
}

#[allow(non_snake_case)]
pub unsafe fn onPageLoading(mut env: JNIEnv, _: JClass, url: JString) {
  match env.get_string(&url) {
    Ok(url) => {
      let url = url.to_string_lossy().to_string();
      if let Some(h) = ON_LOAD_HANDLER.get() {
        (h.0)(PageLoadEvent::Started, url)
      }
    }
    Err(e) => log::warn!("Failed to parse JString: {}", e),
  }
}

#[allow(non_snake_case)]
pub unsafe fn onPageLoaded(mut env: JNIEnv, _: JClass, url: JString) {
  match env.get_string(&url) {
    Ok(url) => {
      let url = url.to_string_lossy().to_string();
      if let Some(h) = ON_LOAD_HANDLER.get() {
        (h.0)(PageLoadEvent::Finished, url)
      }
    }
    Err(e) => log::warn!("Failed to parse JString: {}", e),
  }
}
