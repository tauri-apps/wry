// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use http::{
  header::{HeaderName, HeaderValue, CONTENT_TYPE},
  Request,
};
use tao::platform::android::ndk_glue::jni::{
  errors::Error as JniError,
  objects::{JClass, JMap, JObject, JString, JValue},
  sys::jobject,
  JNIEnv,
};

use super::{create_headers_map, IPC, REQUEST_HANDLER, TITLE_CHANGE_HANDLER};

fn handle_request(env: JNIEnv, request: JObject) -> Result<jobject, JniError> {
  let mut request_builder = Request::builder();

  let uri = env
    .call_method(request, "getUrl", "()Landroid/net/Uri;", &[])?
    .l()?;
  let url: JString = env
    .call_method(uri, "toString", "()Ljava/lang/String;", &[])?
    .l()?
    .into();
  request_builder = request_builder.uri(&env.get_string(url)?.to_string_lossy().to_string());

  let method: JString = env
    .call_method(request, "getMethod", "()Ljava/lang/String;", &[])?
    .l()?
    .into();
  request_builder = request_builder.method(
    env
      .get_string(method)?
      .to_string_lossy()
      .to_string()
      .as_str(),
  );

  let request_headers = env
    .call_method(request, "getRequestHeaders", "()Ljava/util/Map;", &[])?
    .l()?;
  let request_headers = JMap::from_env(&env, request_headers)?;
  for (header, value) in request_headers.iter()? {
    let header = env.get_string(header.into())?;
    let value = env.get_string(value.into())?;
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
      let status_code = status.as_u16();
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
          env.new_string(mime_type)?.into(),
          if let Some(encoding) = encoding {
            env.new_string(&encoding)?.into()
          } else {
            JObject::null().into()
          },
        )
      } else {
        (JObject::null().into(), JObject::null().into())
      };

      let response_headers = create_headers_map(&env, response.headers())?;

      let bytes = response.body();

      let byte_array_input_stream = env.find_class("java/io/ByteArrayInputStream")?;
      let byte_array = env.byte_array_from_slice(&bytes)?;
      let stream = env.new_object(
        byte_array_input_stream,
        "([B)V",
        &[JValue::Object(unsafe { JObject::from_raw(byte_array) })],
      )?;

      let web_resource_response_class = env.find_class("android/webkit/WebResourceResponse")?;
      let web_resource_response = env.new_object(
        web_resource_response_class,
        "(Ljava/lang/String;Ljava/lang/String;ILjava/lang/String;Ljava/util/Map;Ljava/io/InputStream;)V",
        &[mime_type, encoding, (status_code as i32).into(), env.new_string(reason_phrase)?.into(), response_headers.into(), stream.into()],
      )?;

      return Ok(*web_resource_response);
    }
  }
  Ok(*JObject::null())
}

#[allow(non_snake_case)]
pub unsafe fn handleRequest(env: JNIEnv, _: JClass, request: JObject) -> jobject {
  match handle_request(env, request) {
    Ok(response) => response,
    Err(e) => {
      log::warn!("Failed to handle request: {}", e);
      *JObject::null()
    }
  }
}

pub unsafe fn ipc(env: JNIEnv, _: JClass, arg: JString) {
  match env.get_string(arg) {
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
pub unsafe fn handleReceivedTitle(env: JNIEnv, _: JClass, _webview: JObject, title: JString) {
  match env.get_string(title) {
    Ok(title) => {
      let title = title.to_string_lossy().to_string();
      if let Some(w) = TITLE_CHANGE_HANDLER.get() {
        (w.0)(&w.1, title)
      }
    }
    Err(e) => log::warn!("Failed to parse JString: {}", e),
  }
}
