// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

// A silly implementation of file drop handling for Windows!

use crate::DragDropEvent;

use std::{
  cell::UnsafeCell, ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf, ptr, rc::Rc,
};

use windows::{
  core::{implement, BOOL},
  Win32::{
    Foundation::{HWND, LPARAM, POINT, POINTL},
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
  },
};

pub(crate) struct DragDropController {
  // Keep (HWND, IDropTarget) pairs so we can reliably revoke the registration per HWND.
  drop_targets: Vec<(HWND, IDropTarget)>,

  // The container HWND that owns the WebView2 child windows.
  parent: HWND,

  // Shared handler so each injected IDropTarget can call back without borrowing `self`.
  handler: Rc<dyn Fn(DragDropEvent) -> bool>,
}

impl DragDropController {
  #[inline]
  pub(crate) fn new(parent: HWND, handler: Box<dyn Fn(DragDropEvent) -> bool>) -> Self {
    let mut controller = DragDropController {
      drop_targets: Vec::new(),
      parent,
      handler: Rc::new(handler),
    };

    // WebView2's internal child HWNDs may not be stable until after show/resize, but we can
    // opportunistically register now and later call `reinit()` when the window actually shows.
    controller.register_targets();
    controller
  }

  #[inline]
  pub(crate) fn reinit(&mut self) {
    // WebView2 can recreate/replace its internal child HWNDs; revoke and re-enumerate to keep
    // the drop target registered on the current live windows.
    for (hwnd, _) in self.drop_targets.drain(..) {
      let _ = unsafe { RevokeDragDrop(hwnd) };
    }

    self.register_targets();
  }

  #[inline]
  pub(crate) fn is_inited(&self) -> bool {
    !self.drop_targets.is_empty()
  }

  #[inline]
  fn register_targets(&mut self) {
    // EnumChildWindows requires a C callback; pass `self` through LPARAM.
    // Safety: EnumChildWindows is synchronous, so `self` stays valid for the duration.
    let this = self as *mut DragDropController;
    let lparam = LPARAM(this as isize);

    unsafe extern "system" fn enumerate_callback(child: HWND, lparam: LPARAM) -> BOOL {
      let controller = &mut *(lparam.0 as *mut DragDropController);
      controller.inject_in_hwnd(child);
      true.into()
    }

    let ok = unsafe { EnumChildWindows(Some(self.parent), Some(enumerate_callback), lparam) };
    if !ok.as_bool() {
      #[cfg(feature = "tracing")]
      tracing::debug!("EnumChildWindows failed for parent {:?}", self.parent);
    }
  }

  #[inline]
  fn inject_in_hwnd(&mut self, hwnd: HWND) -> bool {
    // Avoid double-registering the same HWND.
    if self.drop_targets.iter().any(|(h, _)| *h == hwnd) {
      return true;
    }

    let handler = self.handler.clone();
    let target: IDropTarget = DragDropTarget::new(hwnd, handler).into();

    // Override any existing drop target on that HWND (if present), then register ours.
    let _ = unsafe { RevokeDragDrop(hwnd) };
    if unsafe { RegisterDragDrop(hwnd, &target) }.is_ok() {
      self.drop_targets.push((hwnd, target));
      true
    } else {
      false
    }
  }
}

impl Drop for DragDropController {
  fn drop(&mut self) {
    // Ensure we don't leave HWNDs registered after the webview/controller is dropped.
    for (hwnd, _) in self.drop_targets.drain(..) {
      let _ = unsafe { RevokeDragDrop(hwnd) };
    }
  }
}

#[implement(IDropTarget)]
pub struct DragDropTarget {
  hwnd: HWND,
  listener: Rc<dyn Fn(DragDropEvent) -> bool>,
  cursor_effect: UnsafeCell<DROPEFFECT>,
  enter_is_valid: UnsafeCell<bool>, /* If the currently hovered item is not valid there must not be any `HoveredFileCancelled` emitted */
}

impl DragDropTarget {
  pub fn new(hwnd: HWND, listener: Rc<dyn Fn(DragDropEvent) -> bool>) -> DragDropTarget {
    Self {
      hwnd,
      listener,
      cursor_effect: DROPEFFECT_NONE.into(),
      enter_is_valid: false.into(),
    }
  }

  unsafe fn iterate_filenames<F>(
    data_obj: windows_core::Ref<'_, IDataObject>,
    mut callback: F,
  ) -> Option<HDROP>
  where
    F: FnMut(PathBuf),
  {
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

          // Fill path_buf with the null-terminated file name
          let str_len = character_count + 1;
          let mut path_buf = vec![0; str_len];
          DragQueryFileW(hdrop, i, Some(&mut path_buf));
          callback(OsString::from_wide(&path_buf[0..character_count]).into());
        }

        Some(hdrop)
      }
      Err(_error) => {
        #[cfg(feature = "tracing")]
        tracing::warn!(
          "{}",
          match _error.code() {
            windows::Win32::Foundation::DV_E_FORMATETC => {
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
impl IDropTarget_Impl for DragDropTarget_Impl {
  fn DragEnter(
    &self,
    pDataObj: windows_core::Ref<'_, IDataObject>,
    _grfKeyState: MODIFIERKEYS_FLAGS,
    pt: &POINTL,
    pdwEffect: *mut DROPEFFECT,
  ) -> windows::core::Result<()> {
    let mut pt = POINT { x: pt.x, y: pt.y };
    let _ = unsafe { ScreenToClient(self.hwnd, &mut pt) };

    let mut paths = Vec::new();
    let hdrop = unsafe { DragDropTarget::iterate_filenames(pDataObj, |path| paths.push(path)) };

    let enter_is_valid = hdrop.is_some();

    if !enter_is_valid {
      return Ok(());
    };

    unsafe {
      *self.enter_is_valid.get() = enter_is_valid;
    }

    (self.listener)(DragDropEvent::Enter {
      paths,
      position: (pt.x as _, pt.y as _),
    });

    let cursor_effect = if enter_is_valid {
      DROPEFFECT_COPY
    } else {
      DROPEFFECT_NONE
    };

    unsafe {
      *pdwEffect = cursor_effect;
      *self.cursor_effect.get() = cursor_effect;
    }

    Ok(())
  }

  fn DragOver(
    &self,
    _grfKeyState: MODIFIERKEYS_FLAGS,
    pt: &POINTL,
    pdwEffect: *mut DROPEFFECT,
  ) -> windows::core::Result<()> {
    if unsafe { *self.enter_is_valid.get() } {
      let mut pt = POINT { x: pt.x, y: pt.y };
      let _ = unsafe { ScreenToClient(self.hwnd, &mut pt) };
      (self.listener)(DragDropEvent::Over {
        position: (pt.x as _, pt.y as _),
      });
    }

    unsafe { *pdwEffect = *self.cursor_effect.get() };
    Ok(())
  }

  fn DragLeave(&self) -> windows::core::Result<()> {
    if unsafe { *self.enter_is_valid.get() } {
      (self.listener)(DragDropEvent::Leave);
    }
    Ok(())
  }

  fn Drop(
    &self,
    pDataObj: windows_core::Ref<'_, IDataObject>,
    _grfKeyState: MODIFIERKEYS_FLAGS,
    pt: &POINTL,
    _pdwEffect: *mut DROPEFFECT,
  ) -> windows::core::Result<()> {
    if unsafe { *self.enter_is_valid.get() } {
      let mut pt = POINT { x: pt.x, y: pt.y };
      let _ = unsafe { ScreenToClient(self.hwnd, &mut pt) };

      let mut paths = Vec::new();
      let hdrop = unsafe { DragDropTarget::iterate_filenames(pDataObj, |path| paths.push(path)) };
      (self.listener)(DragDropEvent::Drop {
        paths,
        position: (pt.x as _, pt.y as _),
      });

      if let Some(hdrop) = hdrop {
        unsafe { DragFinish(hdrop) };
      }
    }

    Ok(())
  }
}
