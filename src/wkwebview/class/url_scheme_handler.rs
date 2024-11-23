// Copyright 2020-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{
  borrow::Cow,
  ffi::{c_char, c_void, CStr},
  panic::AssertUnwindSafe,
  ptr::NonNull,
  slice,
};

use http::{
  header::{CONTENT_LENGTH, CONTENT_TYPE},
  Request, Response as HttpResponse, StatusCode, Version,
};
use objc2::{
  rc::Retained,
  runtime::{AnyClass, AnyObject, ClassBuilder, ProtocolObject},
  ClassType,
};
use objc2_foundation::{
  NSData, NSHTTPURLResponse, NSMutableDictionary, NSObject, NSObjectProtocol, NSString, NSURL,
  NSUUID,
};
use objc2_web_kit::{WKURLSchemeHandler, WKURLSchemeTask};

use crate::{wkwebview::WEBVIEW_IDS, RequestAsyncResponder, WryWebView};

pub fn create(name: &str) -> &AnyClass {
  unsafe {
    let scheme_name = format!("{}URLSchemeHandler", name);
    let cls = ClassBuilder::new(&scheme_name, NSObject::class());
    match cls {
      Some(mut cls) => {
        cls.add_ivar::<*mut c_void>("function");
        cls.add_ivar::<*mut c_char>("webview_id");
        cls.add_method(
          objc2::sel!(webView:startURLSchemeTask:),
          start_task as extern "C" fn(_, _, _, _),
        );
        cls.add_method(
          objc2::sel!(webView:stopURLSchemeTask:),
          stop_task as extern "C" fn(_, _, _, _),
        );
        cls.register()
      }
      None => AnyClass::get(&scheme_name).expect("Failed to get the class definition"),
    }
  }
}

// Task handler for custom protocol
extern "C" fn start_task(
  this: &AnyObject,
  _sel: objc2::runtime::Sel,
  webview: &'static mut WryWebView,
  task: &'static ProtocolObject<dyn WKURLSchemeTask>,
) {
  unsafe {
    #[cfg(feature = "tracing")]
          let span = tracing::info_span!(parent: None, "wry::custom_protocol::handle", uri = tracing::field::Empty)
            .entered();

    let task_key = task.hash(); // hash by task object address
    let task_uuid = webview.add_custom_task_key(task_key);

    let ivar = this.class().instance_variable("webview_id").unwrap();
    let webview_id_ptr: *mut c_char = *ivar.load(this);
    let webview_id = CStr::from_ptr(webview_id_ptr)
      .to_str()
      .ok()
      .unwrap_or_default();

    let ivar = this.class().instance_variable("function").unwrap();
    let function: &*mut c_void = ivar.load(this);
    if !function.is_null() {
      let function = &mut *(*function
        as *mut Box<dyn Fn(crate::WebViewId, Request<Vec<u8>>, RequestAsyncResponder)>);

      // Get url request
      let request = task.request();
      let url = request.URL().unwrap();

      let uri = url.absoluteString().unwrap().to_string();

      #[cfg(feature = "tracing")]
      span.record("uri", uri.clone());

      // Get request method (GET, POST, PUT etc...)
      let method = request.HTTPMethod().unwrap().to_string();

      // Prepare our HttpRequest
      let mut http_request = Request::builder().uri(uri).method(method.as_str());

      // Get body
      let mut sent_form_body = Vec::new();
      let body = request.HTTPBody();
      let body_stream = request.HTTPBodyStream();
      if let Some(body) = body {
        let length = body.length();
        let data_bytes = body.bytes();
        sent_form_body = slice::from_raw_parts(data_bytes.as_ptr(), length).to_vec();
      } else if let Some(body_stream) = body_stream {
        body_stream.open();

        while body_stream.hasBytesAvailable() {
          sent_form_body.reserve(128);
          let p = sent_form_body.as_mut_ptr().add(sent_form_body.len());
          let read_length = sent_form_body.capacity() - sent_form_body.len();
          let count = body_stream.read_maxLength(NonNull::new(p).unwrap(), read_length);
          sent_form_body.set_len(sent_form_body.len() + count as usize);
        }

        body_stream.close();
      }

      // Extract all headers fields
      let all_headers = request.allHTTPHeaderFields();

      // get all our headers values and inject them in our request
      if let Some(all_headers) = all_headers {
        for current_header in all_headers.allKeys().to_vec() {
          let header_value = all_headers.valueForKey(current_header).unwrap();

          // inject the header into the request
          http_request = http_request.header(current_header.to_string(), header_value.to_string());
        }
      }

      let respond_with_404 = || {
        let urlresponse = NSHTTPURLResponse::alloc();
        let response = NSHTTPURLResponse::initWithURL_statusCode_HTTPVersion_headerFields(
          urlresponse,
          &url,
          StatusCode::NOT_FOUND.as_u16().try_into().unwrap(),
          Some(&NSString::from_str(
            format!("{:#?}", Version::HTTP_11).as_str(),
          )),
          None,
        )
        .unwrap();
        task.didReceiveResponse(&response);
        // Finish
        task.didFinish();
      };

      // send response
      match http_request.body(sent_form_body) {
        Ok(final_request) => {
          let responder: Box<dyn FnOnce(HttpResponse<Cow<'static, [u8]>>)> =
            Box::new(move |sent_response| {
              fn check_webview_id_valid(webview_id: &str) -> crate::Result<()> {
                if !WEBVIEW_IDS.lock().unwrap().contains(webview_id) {
                  return Err(crate::Error::CustomProtocolTaskInvalid);
                }
                Ok(())
              }
              /// Task may not live longer than async custom protocol handler.
              ///
              /// There are roughly 2 ways to cause segfault:
              /// 1. Task has stopped. pointer of the task not valid anymore.
              /// 2. Task had stopped, but the pointer of the task has allocated to a new task.
              ///    Outdated custom handler may call to the new task instance and cause segfault.
              fn check_task_is_valid(
                webview: &WryWebView,
                task_key: usize,
                current_uuid: Retained<NSUUID>,
              ) -> crate::Result<()> {
                let latest_task_uuid = webview.get_custom_task_uuid(task_key);
                if let Some(latest_uuid) = latest_task_uuid {
                  if latest_uuid != current_uuid {
                    return Err(crate::Error::CustomProtocolTaskInvalid);
                  }
                } else {
                  return Err(crate::Error::CustomProtocolTaskInvalid);
                }
                Ok(())
              }

              unsafe fn response(
                // FIXME: though we give it a static lifetime, it's not guaranteed to be valid.
                task: &'static ProtocolObject<dyn WKURLSchemeTask>,
                // FIXME: though we give it a static lifetime, it's not guaranteed to be valid.
                webview: &'static mut WryWebView,
                task_key: usize,
                task_uuid: Retained<NSUUID>,
                webview_id: &str,
                url: Retained<NSURL>,
                sent_response: HttpResponse<Cow<'_, [u8]>>,
              ) -> crate::Result<()> {
                check_task_is_valid(&*webview, task_key, task_uuid.clone())?;

                let content = sent_response.body();
                // default: application/octet-stream, but should be provided by the client
                let wanted_mime = sent_response.headers().get(CONTENT_TYPE);
                // default to 200
                let wanted_status_code = sent_response.status().as_u16() as i32;
                // default to HTTP/1.1
                let wanted_version = format!("{:#?}", sent_response.version());

                let mut headers = NSMutableDictionary::new();

                if let Some(mime) = wanted_mime {
                  headers.insert_id(
                    NSString::from_str(CONTENT_TYPE.as_str()).as_ref(),
                    NSString::from_str(mime.to_str().unwrap()),
                  );
                }
                headers.insert_id(
                  NSString::from_str(CONTENT_LENGTH.as_str()).as_ref(),
                  NSString::from_str(&content.len().to_string()),
                );

                // add headers
                for (name, value) in sent_response.headers().iter() {
                  if let Ok(value) = value.to_str() {
                    headers.insert_id(
                      NSString::from_str(name.as_str()).as_ref(),
                      NSString::from_str(value),
                    );
                  }
                }

                let urlresponse = NSHTTPURLResponse::alloc();
                let response = NSHTTPURLResponse::initWithURL_statusCode_HTTPVersion_headerFields(
                  urlresponse,
                  &url,
                  wanted_status_code.try_into().unwrap(),
                  Some(&NSString::from_str(&wanted_version)),
                  Some(&headers),
                )
                .unwrap();

                check_webview_id_valid(webview_id)?;
                check_task_is_valid(&*webview, task_key, task_uuid.clone())?;

                objc2::exception::catch(AssertUnwindSafe(|| {
                  task.didReceiveResponse(&response);
                }))
                .unwrap();

                // Send data
                let bytes = content.as_ptr() as *mut c_void;
                let data = NSData::alloc();
                // MIGRATE NOTE: we copied the content to the NSData because content will be freed
                // when out of scope but NSData will also free the content when it's done and cause doube free.
                let data = NSData::initWithBytes_length(data, bytes, content.len());
                check_webview_id_valid(webview_id)?;
                check_task_is_valid(&*webview, task_key, task_uuid.clone())?;
                objc2::exception::catch(AssertUnwindSafe(|| {
                  task.didReceiveData(&data);
                }))
                .unwrap();

                // Finish
                check_webview_id_valid(webview_id)?;
                check_task_is_valid(&*webview, task_key, task_uuid.clone())?;
                objc2::exception::catch(AssertUnwindSafe(|| {
                  task.didFinish();
                }))
                .unwrap();

                {
                  let ids = WEBVIEW_IDS.lock().unwrap();
                  if ids.contains(webview_id) {
                    webview.remove_custom_task_key(task_key);
                    Ok(())
                  } else {
                    Err(crate::Error::CustomProtocolTaskInvalid)
                  }
                }
              }

              let _ = response(
                task,
                webview,
                task_key,
                task_uuid,
                webview_id,
                url.clone(),
                sent_response,
              );
            });

          #[cfg(feature = "tracing")]
          let _span = tracing::info_span!("wry::custom_protocol::call_handler").entered();
          function(
            webview_id,
            final_request,
            RequestAsyncResponder { responder },
          );
        }
        Err(_) => respond_with_404(),
      };
    } else {
      #[cfg(feature = "tracing")]
      tracing::warn!(
        "Either WebView or WebContext instance is dropped! This handler shouldn't be called."
      );
    }
  }
}
extern "C" fn stop_task(
  _this: &ProtocolObject<dyn WKURLSchemeHandler>,
  _sel: objc2::runtime::Sel,
  webview: &mut WryWebView,
  task: &ProtocolObject<dyn WKURLSchemeTask>,
) {
  webview.remove_custom_task_key(task.hash());
}
