use jni::{objects::{JClass, JValue, JObject}, JNIEnv, sys::_jobject, NativeMethod};
use log::Level;
use ndk::{
  input_queue::InputQueue,
  looper::{FdEvent, ForeignLooper, ThreadLooper},
  native_activity::NativeActivity,
  native_window::NativeWindow,
};
use ndk_sys::{AInputQueue, ANativeActivity, ANativeWindow, ARect};
use once_cell::sync::Lazy;
use std::{
  ffi::{CStr, CString},
  fs::File,
  io::{BufRead, BufReader},
  os::{raw, unix::prelude::*},
  ptr::NonNull,
  sync::{Arc, Condvar, Mutex, RwLock, RwLockReadGuard},
  thread,
};
use super::{CHANNEL, WebViewMessage};

pub use android_logger;
pub use log;

/// `ndk-glue` macros register the reading end of an event pipe with the
/// main [`ThreadLooper`] under this `ident`.
/// When returned from [`ThreadLooper::poll_*`](ThreadLooper::poll_once)
/// an event can be retrieved from [`poll_events()`].
pub const NDK_GLUE_LOOPER_EVENT_PIPE_IDENT: i32 = 0;

/// The [`InputQueue`] received from Android is registered with the main
/// [`ThreadLooper`] under this `ident`.
/// When returned from [`ThreadLooper::poll_*`](ThreadLooper::poll_once)
/// an event can be retrieved from [`input_queue()`].
pub const NDK_GLUE_LOOPER_INPUT_QUEUE_IDENT: i32 = 1;

pub fn android_log(level: Level, tag: &CStr, msg: &CStr) {
  let prio = match level {
    Level::Error => ndk_sys::android_LogPriority_ANDROID_LOG_ERROR,
    Level::Warn => ndk_sys::android_LogPriority_ANDROID_LOG_WARN,
    Level::Info => ndk_sys::android_LogPriority_ANDROID_LOG_INFO,
    Level::Debug => ndk_sys::android_LogPriority_ANDROID_LOG_DEBUG,
    Level::Trace => ndk_sys::android_LogPriority_ANDROID_LOG_VERBOSE,
  };
  unsafe {
    ndk_sys::__android_log_write(prio as raw::c_int, tag.as_ptr(), msg.as_ptr());
  }
}

static NATIVE_WINDOW: Lazy<RwLock<Option<NativeWindow>>> = Lazy::new(|| Default::default());
static INPUT_QUEUE: Lazy<RwLock<Option<InputQueue>>> = Lazy::new(|| Default::default());
static CONTENT_RECT: Lazy<RwLock<Rect>> = Lazy::new(|| Default::default());
static LOOPER: Lazy<Mutex<Option<ForeignLooper>>> = Lazy::new(|| Default::default());

pub fn native_window() -> RwLockReadGuard<'static, Option<NativeWindow>> {
  NATIVE_WINDOW.read().unwrap()
}

pub fn input_queue() -> RwLockReadGuard<'static, Option<InputQueue>> {
  INPUT_QUEUE.read().unwrap()
}

pub fn content_rect() -> Rect {
  CONTENT_RECT.read().unwrap().clone()
}

static PIPE: Lazy<[RawFd; 2]> = Lazy::new(|| {
  let mut pipe: [RawFd; 2] = Default::default();
  unsafe { libc::pipe(pipe.as_mut_ptr()) };
  pipe
});

pub fn poll_events() -> Option<Event> {
  unsafe {
    let size = std::mem::size_of::<Event>();
    let mut event = Event::Start;
    if libc::read(PIPE[0], &mut event as *mut _ as *mut _, size) == size as libc::ssize_t {
      Some(event)
    } else {
      None
    }
  }
}

unsafe fn wake(_activity: *mut ANativeActivity, event: Event) {
  log::trace!("{:?}", event);
  let size = std::mem::size_of::<Event>();
  let res = libc::write(PIPE[1], &event as *const _ as *const _, size);
  assert_eq!(res, size as libc::ssize_t);
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Rect {
  pub left: u32,
  pub top: u32,
  pub right: u32,
  pub bottom: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Event {
  Start,
  Resume,
  SaveInstanceState,
  Pause,
  Stop,
  Destroy,
  ConfigChanged,
  LowMemory,
  WindowLostFocus,
  WindowHasFocus,
  WindowCreated,
  WindowResized,
  WindowRedrawNeeded,
  /// If the window is in use by ie. a graphics API, make sure the lock from
  /// [`native_window()`] is held on to until after freeing those resources.
  ///
  /// After receiving this [`Event`] `ndk_glue` will block until that read-lock
  /// is released before returning to Android and allowing it to free up the window.
  WindowDestroyed,
  InputQueueCreated,
  /// After receiving this [`Event`] `ndk_glue` will block until the read-lock from
  /// [`input_queue()`] is released before returning to Android and allowing it to
  /// free up the input queue.
  InputQueueDestroyed,
  ContentRectChanged,
}

pub unsafe fn on_create(
    env: JNIEnv,
    jclass: JClass,
    jobject: JObject,
    main: fn(),
) {
  let url = env.new_string("https://www.google.com").unwrap();
  env.call_method(jobject, "loadUrl", "(Ljava/lang/String;)V", &[url.into()]).unwrap();
  
  let looper = ThreadLooper::for_thread().unwrap().into_foreign();
  looper.add_fd_with_callback(super::PIPE[0], FdEvent::INPUT, move |fd| {
      unsafe {
        let size = std::mem::size_of::<bool>();
        let mut wake = false;
        if libc::read(super::PIPE[0], &mut wake  as *mut _ as *mut _, size) == size as libc::ssize_t {
            if let Ok(message) = CHANNEL.1.recv() {
                match message {
                    WebViewMessage::LoadUrl(url) => {
                      if let Ok(url) = env.new_string(url) {
                        // let webview = env.call_method(activity, "findViewById", "(I)Landroid/view/View;", &[5566.into()]).unwrap().l().unwrap();
                        // env.call_method(webview, "loadUrl", "(Ljava/lang/String;)V", &[url.into()]).unwrap();
                      }
                    },
                    WebViewMessage::None => (),
                }
            }

        }
        true
      }
  });


  let mut logpipe: [RawFd; 2] = Default::default();
  libc::pipe(logpipe.as_mut_ptr());
  libc::dup2(logpipe[1], libc::STDOUT_FILENO);
  libc::dup2(logpipe[1], libc::STDERR_FILENO);
  thread::spawn(move || {
    let tag = CStr::from_bytes_with_nul(b"RustStdoutStderr\0").unwrap();
    let file = File::from_raw_fd(logpipe[0]);
    let mut reader = BufReader::new(file);
    let mut buffer = String::new();
    loop {
      buffer.clear();
      if let Ok(len) = reader.read_line(&mut buffer) {
        if len == 0 {
          break;
        } else if let Ok(msg) = CString::new(buffer.clone()) {
          android_log(Level::Info, tag, &msg);
        }
      }
    }
  });

  let looper_ready = Arc::new(Condvar::new());
  let signal_looper_ready = looper_ready.clone();

  thread::spawn(move || {
    let looper = ThreadLooper::prepare();
    let foreign = looper.into_foreign();
    foreign
      .add_fd(
        PIPE[0],
        NDK_GLUE_LOOPER_EVENT_PIPE_IDENT,
        FdEvent::INPUT,
        std::ptr::null_mut(),
      )
      .unwrap();

    {
      let mut locked_looper = LOOPER.lock().unwrap();
      *locked_looper = Some(foreign);
      signal_looper_ready.notify_one();
    }

    main()
  });

  // Don't return from this function (`ANativeActivity_onCreate`) until the thread
  // has created its `ThreadLooper` and assigned it to the static `LOOPER`
  // variable. It will be used from `on_input_queue_created` as soon as this
  // function returns.
  let locked_looper = LOOPER.lock().unwrap();
  let _mutex_guard = looper_ready
    .wait_while(locked_looper, |looper| looper.is_none())
    .unwrap();
}

/// # Safety
/// `activity` must either be null (resulting in a safe panic)
/// or a pointer to a valid Android `ANativeActivity`.
pub unsafe fn init(
  activity: *mut ANativeActivity,
  _saved_state: *mut u8,
  _saved_state_size: usize,
  main: fn(),
) {}

unsafe extern "C" fn on_start(activity: *mut ANativeActivity) {
  wake(activity, Event::Start);
}

unsafe extern "C" fn on_resume(activity: *mut ANativeActivity) {
  wake(activity, Event::Resume);
}

unsafe extern "C" fn on_save_instance_state(
  activity: *mut ANativeActivity,
  _out_size: *mut ndk_sys::size_t,
) -> *mut raw::c_void {
  // TODO
  wake(activity, Event::SaveInstanceState);
  std::ptr::null_mut()
}

unsafe extern "C" fn on_pause(activity: *mut ANativeActivity) {
  wake(activity, Event::Pause);
}

unsafe extern "C" fn on_stop(activity: *mut ANativeActivity) {
  wake(activity, Event::Stop);
}

unsafe extern "C" fn on_destroy(activity: *mut ANativeActivity) {
  wake(activity, Event::Destroy);
  ndk_context::release_android_context();
}

unsafe extern "C" fn on_configuration_changed(activity: *mut ANativeActivity) {
  wake(activity, Event::ConfigChanged);
}

unsafe extern "C" fn on_low_memory(activity: *mut ANativeActivity) {
  wake(activity, Event::LowMemory);
}

unsafe extern "C" fn on_window_focus_changed(
  activity: *mut ANativeActivity,
  has_focus: raw::c_int,
) {
  let event = if has_focus == 0 {
    Event::WindowLostFocus
  } else {
    Event::WindowHasFocus
  };
  wake(activity, event);
}

unsafe extern "C" fn on_window_created(activity: *mut ANativeActivity, window: *mut ANativeWindow) {
  *NATIVE_WINDOW.write().unwrap() = Some(NativeWindow::from_ptr(NonNull::new(window).unwrap()));
  wake(activity, Event::WindowCreated);
}

unsafe extern "C" fn on_window_resized(
  activity: *mut ANativeActivity,
  _window: *mut ANativeWindow,
) {
  wake(activity, Event::WindowResized);
}

unsafe extern "C" fn on_window_redraw_needed(
  activity: *mut ANativeActivity,
  _window: *mut ANativeWindow,
) {
  wake(activity, Event::WindowRedrawNeeded);
}

unsafe extern "C" fn on_window_destroyed(
  activity: *mut ANativeActivity,
  window: *mut ANativeWindow,
) {
  wake(activity, Event::WindowDestroyed);
  let mut native_window_guard = NATIVE_WINDOW.write().unwrap();
  assert_eq!(native_window_guard.as_ref().unwrap().ptr().as_ptr(), window);
  *native_window_guard = None;
}

unsafe extern "C" fn on_input_queue_created(
  activity: *mut ANativeActivity,
  queue: *mut AInputQueue,
) {
  let input_queue = InputQueue::from_ptr(NonNull::new(queue).unwrap());
  let locked_looper = LOOPER.lock().unwrap();
  // The looper should always be `Some` after `fn init()` returns, unless
  // future code cleans it up and sets it back to `None` again.
  let looper = locked_looper.as_ref().expect("Looper does not exist");
  input_queue.attach_looper(looper, NDK_GLUE_LOOPER_INPUT_QUEUE_IDENT);
  *INPUT_QUEUE.write().unwrap() = Some(input_queue);
  wake(activity, Event::InputQueueCreated);
}

unsafe extern "C" fn on_input_queue_destroyed(
  activity: *mut ANativeActivity,
  queue: *mut AInputQueue,
) {
  wake(activity, Event::InputQueueDestroyed);
  let mut input_queue_guard = INPUT_QUEUE.write().unwrap();
  assert_eq!(input_queue_guard.as_ref().unwrap().ptr().as_ptr(), queue);
  let input_queue = InputQueue::from_ptr(NonNull::new(queue).unwrap());
  input_queue.detach_looper();
  *input_queue_guard = None;
}

unsafe extern "C" fn on_content_rect_changed(activity: *mut ANativeActivity, rect: *const ARect) {
  let rect = Rect {
    left: (*rect).left as _,
    top: (*rect).top as _,
    right: (*rect).right as _,
    bottom: (*rect).bottom as _,
  };
  *CONTENT_RECT.write().unwrap() = rect;
  wake(activity, Event::ContentRectChanged);
}
