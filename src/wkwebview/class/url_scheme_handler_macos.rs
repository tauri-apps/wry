// Copyright 2020-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{
  borrow::Cow,
  collections::HashMap,
  ffi::{CStr, c_void},
  panic::AssertUnwindSafe,
  ptr::NonNull,
  sync::{Arc, Mutex, Weak},
};

use block2::RcBlock;
use http::{Request, Response, StatusCode, header::CONTENT_LENGTH};
use objc2::{
  AllocAnyThread, ClassType, DeclaredClass, Message,
  rc::Retained,
  runtime::{AnyClass, AnyObject, ClassBuilder, ProtocolObject, Sel},
};
use objc2_foundation::{
  MainThreadMarker, NSData, NSHTTPURLResponse, NSMutableDictionary, NSObject, NSOperationQueue,
  NSString, NSURL,
};
use objc2_web_kit::{WKURLSchemeTask, WKWebView};

use crate::{Error, RequestAsyncResponder, Result, WebViewId, WryWebView, wkwebview::COUNTER};

type Provider = dyn Fn(WebViewId, Request<Vec<u8>>, RequestAsyncResponder) + Send + Sync;

pub(crate) struct ProtocolViewState {
  id: String,
  tasks: Mutex<Option<HashMap<usize, Arc<()>>>>,
}

impl ProtocolViewState {
  pub(crate) fn new(id: String) -> Self {
    Self {
      id,
      tasks: Mutex::new(Some(HashMap::new())),
    }
  }

  fn start(&self, key: usize) -> Option<Arc<()>> {
    let token = Arc::new(());
    self
      .tasks
      .lock()
      .unwrap()
      .as_mut()?
      .insert(key, token.clone());
    Some(token)
  }

  /// Task may not live longer than async custom protocol handler.
  ///
  /// There are roughly 2 ways to cause segfault:
  /// 1. Task has stopped. pointer of the task not valid anymore.
  /// 2. Task had stopped, but the pointer of the task has allocated to a new task.
  ///    Outdated custom handler may call to the new task instance and cause segfault.
  fn validate(&self, key: usize, token: &Arc<()>) -> Result<()> {
    let tasks = self.tasks.lock().unwrap();
    if tasks
      .as_ref()
      .and_then(|tasks| tasks.get(&key))
      .is_some_and(|current| Arc::ptr_eq(current, token))
    {
      Ok(())
    } else {
      Err(Error::CustomProtocolTaskInvalid)
    }
  }

  fn finish(&self, key: usize, token: &Arc<()>) {
    if let Some(tasks) = self.tasks.lock().unwrap().as_mut() {
      if tasks
        .get(&key)
        .is_some_and(|current| Arc::ptr_eq(current, token))
      {
        tasks.remove(&key);
      }
    }
  }

  fn stop(&self, key: usize) {
    if let Some(tasks) = self.tasks.lock().unwrap().as_mut() {
      tasks.remove(&key);
    }
  }

  pub(crate) fn retire(&self) {
    self.tasks.lock().unwrap().take();
  }
}

struct HandlerState {
  provider: Arc<Provider>,
  legacy_origin: Weak<ProtocolViewState>,
}

pub(crate) fn new(
  name: &str,
  provider: Box<Provider>,
  origin: &Arc<ProtocolViewState>,
) -> Retained<AnyObject> {
  unsafe {
    // Each dylib must keep its native callbacks in its own code and data segments.
    let unique_id = std::ptr::addr_of!(COUNTER) as usize;
    let name = format!("{name}URLSchemeHandler_{unique_id:x}\0");
    let name = CStr::from_bytes_with_nul(name.as_bytes()).unwrap();
    let cls = match ClassBuilder::new(name, NSObject::class()) {
      Some(mut cls) => {
        cls.add_ivar::<*mut c_void>(c"state");
        cls.add_method(objc2::sel!(dealloc), dealloc as extern "C" fn(_, _));
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
      None => AnyClass::get(name).expect("Failed to get the class definition"),
    };
    let handler: Retained<AnyObject> = objc2::msg_send![cls, new];
    let state = Box::new(HandlerState {
      provider: Arc::from(provider),
      legacy_origin: Arc::downgrade(origin),
    });
    let ivar = handler.class().instance_variable(c"state").unwrap();
    *ivar.load_ptr::<*mut c_void>(&handler) = Box::into_raw(state).cast();
    handler
  }
}

extern "C" fn dealloc(this: &mut AnyObject, _sel: Sel) {
  unsafe {
    let ivar = this.class().instance_variable(c"state").unwrap();
    let state = std::mem::replace(ivar.load_mut::<*mut c_void>(this), std::ptr::null_mut());
    if !state.is_null() {
      drop(Box::from_raw(state.cast::<HandlerState>()));
    }
    let _: () = objc2::msg_send![super(this, NSObject::class()), dealloc];
  }
}

fn handler_state(this: &AnyObject) -> &HandlerState {
  unsafe {
    let ivar = this.class().instance_variable(c"state").unwrap();
    &*(*ivar.load::<*mut c_void>(this)).cast::<HandlerState>()
  }
}

fn request_state(this: &AnyObject, webview: &WKWebView) -> Option<Arc<ProtocolViewState>> {
  if let Some(webview) = webview.downcast_ref::<WryWebView>() {
    webview.ivars().protocol_state.upgrade()
  } else {
    handler_state(this).legacy_origin.upgrade()
  }
}

extern "C" fn start_task(
  this: &AnyObject,
  _sel: Sel,
  webview: &WKWebView,
  task: &ProtocolObject<dyn WKURLSchemeTask>,
) {
  let Some(state) = request_state(this, webview) else {
    return;
  };
  let key = task as *const _ as usize;
  let Some(token) = state.start(key) else {
    return;
  };
  let provider = handler_state(this).provider.clone();
  let request = unsafe { task.request() };
  let url = request.URL().unwrap();
  let uri = url.absoluteString().unwrap().to_string();
  let method = request.HTTPMethod().unwrap().to_string();
  let mut request_builder = Request::builder().uri(uri).method(method.as_str());
  if let Some(headers) = request.allHTTPHeaderFields() {
    for header in headers.allKeys().iter() {
      let value = headers.valueForKey(&header).unwrap();
      request_builder = request_builder.header(header.to_string(), value.to_string());
    }
  }

  let mut body = Vec::new();
  let mut body_failed = false;
  if let Some(data) = request.HTTPBody() {
    body = data.to_vec();
  } else if let Some(stream) = request.HTTPBodyStream() {
    stream.open();
    let mut buffer = [0u8; 128];
    while stream.hasBytesAvailable() {
      let count =
        unsafe { stream.read_maxLength(NonNull::new(buffer.as_mut_ptr()).unwrap(), buffer.len()) };
      if count <= 0 {
        body_failed = count < 0;
        break;
      }
      body.extend_from_slice(&buffer[..count as usize]);
    }
    stream.close();
  }

  let weak_state = Arc::downgrade(&state);
  let task = task.retain();
  let completion = RequestAsyncResponder {
    responder: Box::new(move |response| {
      let Some(state) = weak_state.upgrade() else {
        return;
      };
      let result = deliver(&state, key, &token, &task, &url, response);
      state.finish(key, &token);
      if let Err(_error) = result {
        #[cfg(feature = "tracing")]
        tracing::warn!("Error responding to custom protocol task: {:?}", _error);
      }
    }),
  };
  let responder = RequestAsyncResponder {
    responder: Box::new(move |response| respond_on_main(completion, response)),
  };
  match (body_failed, request_builder.body(body)) {
    (false, Ok(request)) => provider(&state.id, request, responder),
    _ => responder.respond(
      Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Vec::new())
        .unwrap(),
    ),
  }
}

fn respond_on_main(responder: RequestAsyncResponder, response: Response<Cow<'static, [u8]>>) {
  if MainThreadMarker::new().is_some() {
    responder.respond(response);
    return;
  }
  let pending = Mutex::new(Some((responder, response)));
  let _: &(dyn Send + Sync) = &pending;
  let block = RcBlock::new(move || {
    let work = pending.lock().unwrap().take();
    if let Some((responder, response)) = work {
      responder.respond(response);
    }
  });
  // The block captures only Send work through the existing RequestAsyncResponder boundary.
  unsafe { NSOperationQueue::mainQueue().addOperationWithBlock(&block) };
}

fn deliver(
  state: &ProtocolViewState,
  key: usize,
  token: &Arc<()>,
  task: &ProtocolObject<dyn WKURLSchemeTask>,
  url: &NSURL,
  response: Response<Cow<'static, [u8]>>,
) -> Result<()> {
  state.validate(key, token)?;
  let headers = NSMutableDictionary::new();
  headers.insert(
    &*NSString::from_str(CONTENT_LENGTH.as_str()),
    &*NSString::from_str(&response.body().len().to_string()),
  );
  for (name, value) in response.headers() {
    if let Ok(value) = value.to_str() {
      headers.insert(
        &*NSString::from_str(name.as_str()),
        &*NSString::from_str(value),
      );
    }
  }
  let native_response = NSHTTPURLResponse::initWithURL_statusCode_HTTPVersion_headerFields(
    NSHTTPURLResponse::alloc(),
    url,
    response.status().as_u16() as isize,
    Some(&NSString::from_str(&format!("{:#?}", response.version()))),
    Some(&headers),
  )
  .ok_or(Error::CustomProtocolTaskInvalid)?;
  state.validate(key, token)?;
  objc2::exception::catch(AssertUnwindSafe(|| unsafe {
    task.didReceiveResponse(&native_response);
  }))
  .map_err(|_| Error::CustomProtocolTaskInvalid)?;
  let data = match response.into_body() {
    Cow::Owned(bytes) => NSData::from_vec(bytes),
    Cow::Borrowed(bytes) => NSData::with_bytes(bytes),
  };
  state.validate(key, token)?;
  objc2::exception::catch(AssertUnwindSafe(|| unsafe {
    task.didReceiveData(&data);
  }))
  .map_err(|_| Error::CustomProtocolTaskInvalid)?;
  state.validate(key, token)?;
  objc2::exception::catch(AssertUnwindSafe(|| unsafe {
    task.didFinish();
  }))
  .map_err(|_| Error::CustomProtocolTaskInvalid)
}

extern "C" fn stop_task(
  this: &AnyObject,
  _sel: Sel,
  webview: &WKWebView,
  task: &ProtocolObject<dyn WKURLSchemeTask>,
) {
  if let Some(state) = request_state(this, webview) {
    state.stop(task as *const _ as usize);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn owner_retirement_and_reused_task_addresses_cannot_revive_responses() {
    let source = ProtocolViewState::new("same-id".into());
    let child = ProtocolViewState::new("same-id".into());
    let source_token = source.start(1).unwrap();
    let old_child_token = child.start(1).unwrap();
    child.stop(1);
    assert!(child.validate(1, &old_child_token).is_err());
    let child_token = child.start(1).unwrap();
    child.finish(1, &old_child_token);
    assert!(child.validate(1, &old_child_token).is_err());
    assert!(child.validate(1, &child_token).is_ok());
    source.retire();
    assert!(source.validate(1, &source_token).is_err());
    assert!(source.start(2).is_none());
    assert!(child.validate(1, &child_token).is_ok());
    child.finish(1, &child_token);
    assert!(child.validate(1, &child_token).is_err());
    let token = child.start(1).unwrap();
    child.retire();
    assert!(child.validate(1, &token).is_err());
  }
}
