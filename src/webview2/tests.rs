// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::sync::{
  atomic::{AtomicUsize, Ordering},
  Mutex,
};

use windows::{
  core::w,
  Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
      Shell::{RemoveWindowSubclass, SetWindowSubclass},
      WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, SendMessageW, CW_USEDEFAULT, WINDOW_EX_STYLE,
        WM_ENTERSIZEMOVE, WM_MOVE, WM_MOVING, WM_SETFOCUS, WM_SIZE, WS_OVERLAPPED,
      },
    },
  },
};

use super::{InnerWebView, PARENT_SUBCLASS_ID};

const OBSERVER_SUBCLASS_ID: usize = PARENT_SUBCLASS_ID as usize + 0x100;
const OBSERVER_RESULT: LRESULT = LRESULT(0x57_52_59);
static WINDOW_SUBCLASS_TEST_LOCK: Mutex<()> = Mutex::new(());

unsafe extern "system" fn observer_subclass_proc(
  _hwnd: HWND,
  _msg: u32,
  _wparam: WPARAM,
  _lparam: LPARAM,
  _uidsubclass: usize,
  dwrefdata: usize,
) -> LRESULT {
  let calls = &*(dwrefdata as *const AtomicUsize);
  calls.fetch_add(1, Ordering::SeqCst);
  OBSERVER_RESULT
}

struct HiddenStaticWindow {
  hwnd: HWND,
  observer_installed: bool,
  wry_installed: bool,
}

impl HiddenStaticWindow {
  unsafe fn create() -> Self {
    let hwnd = CreateWindowExW(
      WINDOW_EX_STYLE::default(),
      w!("STATIC"),
      w!("wry-null-refdata-regression"),
      WS_OVERLAPPED,
      CW_USEDEFAULT,
      CW_USEDEFAULT,
      1,
      1,
      None,
      None,
      None,
      None,
    )
    .expect("a hidden Win32 STATIC window should be created");

    Self {
      hwnd,
      observer_installed: false,
      wry_installed: false,
    }
  }

  unsafe fn install_subclasses(&mut self, observer_calls: &AtomicUsize) {
    // SetWindowSubclass inserts the newest callback at the front. Install the
    // observer first so Wry runs first and can reach it only via
    // DefSubclassProc. Returning a sentinel makes that delegation observable
    // at the SendMessageW boundary.
    assert!(
      SetWindowSubclass(
        self.hwnd,
        Some(observer_subclass_proc),
        OBSERVER_SUBCLASS_ID,
        observer_calls as *const AtomicUsize as usize,
      )
      .as_bool(),
      "observer subclass should be installed"
    );
    self.observer_installed = true;

    assert!(
      SetWindowSubclass(
        self.hwnd,
        Some(InnerWebView::parent_subclass_proc),
        PARENT_SUBCLASS_ID as usize,
        0,
      )
      .as_bool(),
      "Wry parent subclass should be installed with cleared refdata"
    );
    self.wry_installed = true;
  }
}

impl Drop for HiddenStaticWindow {
  fn drop(&mut self) {
    unsafe {
      if self.wry_installed {
        let _ = RemoveWindowSubclass(
          self.hwnd,
          Some(InnerWebView::parent_subclass_proc),
          PARENT_SUBCLASS_ID as usize,
        );
      }
      if self.observer_installed {
        let _ = RemoveWindowSubclass(
          self.hwnd,
          Some(observer_subclass_proc),
          OBSERVER_SUBCLASS_ID,
        );
      }
      let _ = DestroyWindow(self.hwnd);
    }
  }
}

#[test]
fn cleared_parent_controller_refdata_delegates_controller_messages() {
  // Win32 subclass helpers are thread-affine. Keep this native window and all
  // synchronous dispatch on the test thread, and serialize this test if more
  // subclass regressions are added later.
  let _serial = WINDOW_SUBCLASS_TEST_LOCK
    .lock()
    .unwrap_or_else(|poisoned| poisoned.into_inner());
  let observer_calls = AtomicUsize::new(0);
  let mut window = unsafe { HiddenStaticWindow::create() };
  unsafe { window.install_subclasses(&observer_calls) };

  for (index, message) in [WM_SIZE, WM_SETFOCUS, WM_ENTERSIZEMOVE, WM_MOVE, WM_MOVING]
    .into_iter()
    .enumerate()
  {
    let result = unsafe { SendMessageW(window.hwnd, message, Some(WPARAM(0)), Some(LPARAM(0))) };

    assert_eq!(
      result, OBSERVER_RESULT,
      "message {message:#x} should reach the next subclass via DefSubclassProc"
    );
    assert_eq!(
      observer_calls.load(Ordering::SeqCst),
      index + 1,
      "observer should run exactly once for message {message:#x}"
    );
  }
}
