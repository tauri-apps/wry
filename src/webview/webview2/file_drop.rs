// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::webview::FileDropEvent;

// A silly implementation of file drop handling for Windows!
// This can be pretty much entirely replaced when WebView2 SDK 1.0.721-prerelease becomes stable.
// https://docs.microsoft.com/en-us/microsoft-edge/webview2/releasenotes#10721-prerelease
// https://docs.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2experimentalcompositioncontroller3?view=webview2-1.0.721-prerelease&preserve-view=true

use std::{
  cell::UnsafeCell,
  ffi::OsString,
  os::{raw::c_void, windows::ffi::OsStringExt},
  path::PathBuf,
  ptr,
  rc::Rc,
};

use windows::Win32::{
  Foundation::{self as win32f, BOOL, DRAGDROP_E_INVALIDHWND, HWND, LPARAM, POINT, POINTL},
  Graphics::Gdi::ScreenToClient,
  System::{
    Com::{IDataObject, DVASPECT_CONTENT, FORMATETC, TYMED_HGLOBAL},
    Ole::{
      IDropTarget, IDropTarget_Impl, RegisterDragDrop, RevokeDragDrop, CF_HDROP, DROPEFFECT,
      DROPEFFECT_COPY, DROPEFFECT_NONE,
    },
    SystemServices::MODIFIERKEYS_FLAGS,
  },
  UI::{
    Shell::{DragFinish, DragQueryFileW, HDROP},
    WindowsAndMessaging::EnumChildWindows,
  },
};

use windows_implement::implement;

use crate::application::{
  dpi::PhysicalPosition, platform::windows::WindowExtWindows, window::Window,
};

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
        && RegisterDragDrop(hwnd, &file_drop_handler).is_ok()
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

#[implement(IDropTarget)]
pub struct FileDropHandler {
  window: Rc<Window>,
  listener: Rc<dyn Fn(&Window, FileDropEvent) -> bool>,
  cursor_effect: UnsafeCell<DROPEFFECT>,
  hovered_is_valid: UnsafeCell<bool>, /* If the currently hovered item is not valid there must not be any `HoveredFileCancelled` emitted */
}

impl FileDropHandler {
  pub fn new(
    window: Rc<Window>,
    listener: Rc<dyn Fn(&Window, FileDropEvent) -> bool>,
  ) -> FileDropHandler {
    Self {
      window,
      listener,
      cursor_effect: DROPEFFECT_NONE.into(),
      hovered_is_valid: false.into(),
    }
  }

  unsafe fn collect_paths(
    data_obj: Option<&IDataObject>,
    paths: &mut Vec<PathBuf>,
  ) -> Option<HDROP> {
    let drop_format = FORMATETC {
      cfFormat: CF_HDROP.0,
      ptd: ptr::null_mut(),
      dwAspect: DVASPECT_CONTENT.0,
      lindex: -1,
      tymed: TYMED_HGLOBAL.0 as u32,
    };

    match data_obj
      .as_ref()
      .expect("Received null IDataObject")
      .GetData(&drop_format)
    {
      Ok(medium) => {
        let hdrop = HDROP(medium.u.hGlobal.0 as _);

        // The second parameter (0xFFFFFFFF) instructs the function to return the item count
        let item_count = DragQueryFileW(hdrop, 0xFFFFFFFF, None);

        for i in 0..item_count {
          // Get the length of the path string NOT including the terminating null character.
          // Previously, this was using a fixed size array of MAX_PATH length, but the
          // Windows API allows longer paths under certain circumstances.
          let character_count = DragQueryFileW(hdrop, i, None) as usize;
          let str_len = character_count + 1;

          // Fill path_buf with the null-terminated file name
          let mut path_buf = Vec::with_capacity(str_len);
          DragQueryFileW(hdrop, i, std::mem::transmute(path_buf.spare_capacity_mut()));
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
              "Error occurred while processing dropped/hovered item: item is not a file."
            }
            _ => "Unexpected error occurred while processing dropped/hovered item.",
          }
        );
        None
      }
    }
  }
}

#[allow(non_snake_case)]
impl IDropTarget_Impl for FileDropHandler {
  fn DragEnter(
    &self,
    pDataObj: Option<&IDataObject>,
    _grfKeyState: MODIFIERKEYS_FLAGS,
    pt: &POINTL,
    pdwEffect: *mut DROPEFFECT,
  ) -> windows::core::Result<()> {
    let mut paths = Vec::new();
    unsafe {
      let hdrop = Self::collect_paths(pDataObj, &mut paths);
      let hovered_is_valid = hdrop.is_some();
      let cursor_effect = if hovered_is_valid {
        DROPEFFECT_COPY
      } else {
        DROPEFFECT_NONE
      };
      *pdwEffect = cursor_effect;
      *self.hovered_is_valid.get() = hovered_is_valid;
      *self.cursor_effect.get() = cursor_effect;

      let mut pt = POINT { x: pt.x, y: pt.y };
      ScreenToClient(HWND(self.window.hwnd() as _), &mut pt);
    }

    (self.listener)(
      &self.window,
      FileDropEvent::Hovered {
        paths,
        position: PhysicalPosition::new(pt.x as _, pt.y as _),
      },
    );

    Ok(())
  }

  fn DragOver(
    &self,
    _grfKeyState: MODIFIERKEYS_FLAGS,
    _pt: &POINTL,
    pdwEffect: *mut DROPEFFECT,
  ) -> windows::core::Result<()> {
    unsafe { *pdwEffect = *self.cursor_effect.get() };
    Ok(())
  }

  fn DragLeave(&self) -> windows::core::Result<()> {
    if unsafe { *self.hovered_is_valid.get() } {
      (self.listener)(&self.window, FileDropEvent::Cancelled);
    }
    Ok(())
  }

  fn Drop(
    &self,
    pDataObj: Option<&IDataObject>,
    _grfKeyState: MODIFIERKEYS_FLAGS,
    pt: &POINTL,
    _pdwEffect: *mut DROPEFFECT,
  ) -> windows::core::Result<()> {
    let mut paths = Vec::new();
    unsafe {
      let hdrop = Self::collect_paths(pDataObj, &mut paths);
      if let Some(hdrop) = hdrop {
        DragFinish(hdrop);
      }

      let mut pt = POINT { x: pt.x, y: pt.y };
      ScreenToClient(HWND(self.window.hwnd() as _), &mut pt);
    }

    (self.listener)(
      &self.window,
      FileDropEvent::Dropped {
        paths,
        position: PhysicalPosition::new(pt.x as _, pt.y as _),
      },
    );

    Ok(())
  }
}
