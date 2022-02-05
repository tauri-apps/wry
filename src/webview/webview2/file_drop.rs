// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::webview::FileDropEvent;

// A silly implementation of file drop handling for Windows!
// This can be pretty much entirely replaced when WebView2 SDK 1.0.721-prerelease becomes stable.
// https://docs.microsoft.com/en-us/microsoft-edge/webview2/releasenotes#10721-prerelease
// https://docs.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2experimentalcompositioncontroller3?view=webview2-1.0.721-prerelease&preserve-view=true

use std::{
  ffi::OsString,
  os::{raw::c_void, windows::ffi::OsStringExt},
  path::PathBuf,
  ptr,
  rc::Rc,
};

use windows::{
  self as Windows,
  Win32::{
    Foundation::{self as win32f, BOOL, DRAGDROP_E_INVALIDHWND, HWND, LPARAM, POINTL, PWSTR},
    System::{
      Com::{IDataObject, DVASPECT_CONTENT, FORMATETC, TYMED_HGLOBAL},
      Ole::{IDropTarget, RegisterDragDrop, RevokeDragDrop, DROPEFFECT_COPY, DROPEFFECT_NONE},
      SystemServices::CF_HDROP,
    },
    UI::{
      Shell::{DragFinish, DragQueryFileW, HDROP},
      WindowsAndMessaging::EnumChildWindows,
    },
  },
};

use windows_macros::implement;

use crate::application::window::Window;

pub(crate) struct FileDropController {
  drop_targets: Vec<IDropTarget>,
}

impl FileDropController {
  pub(crate) fn new() -> Self {
    FileDropController {
      drop_targets: Vec::new(),
    }
  }

  pub(crate) fn listen(
    &mut self,
    hwnd: HWND,
    window: Rc<Window>,
    handler: Box<dyn Fn(&Window, FileDropEvent) -> bool>,
  ) {
    let listener = Rc::new(handler);

    // Enumerate child windows to find the WebView2 "window" and override!
    enumerate_child_windows(hwnd, |hwnd| {
      self.inject(hwnd, window.clone(), listener.clone())
    });
  }

  fn inject(
    &mut self,
    hwnd: HWND,
    window: Rc<Window>,
    listener: Rc<dyn Fn(&Window, FileDropEvent) -> bool>,
  ) -> bool {
    // Safety: WinAPI calls are unsafe
    unsafe {
      let file_drop_handler: IDropTarget = FileDropHandler::new(window, listener).into();

      if RevokeDragDrop(hwnd) != Err(DRAGDROP_E_INVALIDHWND.into())
        && RegisterDragDrop(hwnd, file_drop_handler.clone()).is_ok()
      {
        // Not a great solution. But there is no reliable way to get the window handle of the webview, for whatever reason...
        self.drop_targets.push(file_drop_handler);
      }
    }

    true
  }
}

// https://gist.github.com/application-developer-DA/5a460d9ca02948f1d2bfa53100c941da
// Safety: WinAPI calls are unsafe

fn enumerate_child_windows<F>(hwnd: HWND, mut callback: F)
where
  F: FnMut(HWND) -> bool,
{
  let mut trait_obj: &mut dyn FnMut(HWND) -> bool = &mut callback;
  let closure_pointer_pointer: *mut c_void = unsafe { std::mem::transmute(&mut trait_obj) };
  let lparam = LPARAM(closure_pointer_pointer as _);
  unsafe { EnumChildWindows(hwnd, Some(enumerate_callback), lparam) };
}

unsafe extern "system" fn enumerate_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
  let closure = &mut *(lparam.0 as *mut c_void as *mut &mut dyn FnMut(HWND) -> bool);
  closure(hwnd).into()
}

#[implement(Windows::Win32::System::Ole::IDropTarget)]
pub struct FileDropHandler {
  window: Rc<Window>,
  listener: Rc<dyn Fn(&Window, FileDropEvent) -> bool>,
  cursor_effect: u32,
  hovered_is_valid: bool, /* If the currently hovered item is not valid there must not be any `HoveredFileCancelled` emitted */
}

#[allow(non_snake_case)]
impl FileDropHandler {
  pub fn new(
    window: Rc<Window>,
    listener: Rc<dyn Fn(&Window, FileDropEvent) -> bool>,
  ) -> FileDropHandler {
    Self {
      window,
      listener,
      cursor_effect: DROPEFFECT_NONE,
      hovered_is_valid: false,
    }
  }

  unsafe fn DragEnter(
    &mut self,
    pDataObj: &Option<IDataObject>,
    _grfKeyState: u32,
    _pt: POINTL,
    pdwEffect: *mut u32,
  ) -> windows::core::Result<()> {
    let mut paths = Vec::new();
    let hdrop = Self::collect_paths(pDataObj, &mut paths);
    self.hovered_is_valid = hdrop.is_some();
    self.cursor_effect = if self.hovered_is_valid {
      DROPEFFECT_COPY
    } else {
      DROPEFFECT_NONE
    };
    *pdwEffect = self.cursor_effect;

    (self.listener)(&self.window, FileDropEvent::Hovered(paths));

    Ok(())
  }

  unsafe fn DragOver(
    &self,
    _grfKeyState: u32,
    _pt: POINTL,
    pdwEffect: *mut u32,
  ) -> windows::core::Result<()> {
    *pdwEffect = self.cursor_effect;
    Ok(())
  }

  unsafe fn DragLeave(&self) -> windows::core::Result<()> {
    if self.hovered_is_valid {
      (self.listener)(&self.window, FileDropEvent::Cancelled);
    }
    Ok(())
  }

  unsafe fn Drop(
    &self,
    pDataObj: &Option<IDataObject>,
    _grfKeyState: u32,
    _pt: POINTL,
    _pdwEffect: *mut u32,
  ) -> windows::core::Result<()> {
    let mut paths = Vec::new();
    let hdrop = Self::collect_paths(pDataObj, &mut paths);
    if let Some(hdrop) = hdrop {
      DragFinish(hdrop);
    }

    (self.listener)(&self.window, FileDropEvent::Dropped(paths));

    Ok(())
  }

  unsafe fn collect_paths(
    data_obj: &Option<IDataObject>,
    paths: &mut Vec<PathBuf>,
  ) -> Option<HDROP> {
    let drop_format = FORMATETC {
      cfFormat: CF_HDROP as u16,
      ptd: ptr::null_mut(),
      dwAspect: DVASPECT_CONTENT as u32,
      lindex: -1,
      tymed: TYMED_HGLOBAL as u32,
    };

    match data_obj
      .as_ref()
      .expect("Received null IDataObject")
      .GetData(&drop_format)
    {
      Ok(medium) => {
        let hdrop = HDROP(medium.Anonymous.hGlobal);

        // The second parameter (0xFFFFFFFF) instructs the function to return the item count
        let item_count = DragQueryFileW(hdrop, 0xFFFFFFFF, PWSTR::default(), 0);

        for i in 0..item_count {
          // Get the length of the path string NOT including the terminating null character.
          // Previously, this was using a fixed size array of MAX_PATH length, but the
          // Windows API allows longer paths under certain circumstances.
          let character_count = DragQueryFileW(hdrop, i, PWSTR::default(), 0) as usize;
          let str_len = character_count + 1;

          // Fill path_buf with the null-terminated file name
          let mut path_buf = Vec::with_capacity(str_len);
          DragQueryFileW(hdrop, i, PWSTR(path_buf.as_mut_ptr()), str_len as u32);
          path_buf.set_len(str_len);

          paths.push(OsString::from_wide(&path_buf[0..character_count]).into());
        }

        Some(hdrop)
      }
      Err(error) => {
        log::warn!(
          "{}",
          match error.code() {
            win32f::DV_E_FORMATETC => {
              // If the dropped item is not a file this error will occur.
              // In this case it is OK to return without taking further action.
              "Error occured while processing dropped/hovered item: item is not a file."
            }
            _ => "Unexpected error occured while processing dropped/hovered item.",
          }
        );
        None
      }
    }
  }
}
