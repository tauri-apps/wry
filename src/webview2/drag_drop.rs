// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

// A silly implementation of file drop handling for Windows!

use crate::DragDropEvent;

use std::{
  cell::UnsafeCell,
  ffi::OsString,
  os::{raw::c_void, windows::ffi::OsStringExt},
  path::PathBuf,
  ptr,
  rc::{Rc, Weak},
};

use windows::{
  Win32::{
    Foundation::{DRAGDROP_E_INVALIDHWND, HWND, LPARAM, POINT, POINTL},
    Graphics::Gdi::ScreenToClient,
    System::{
      Com::{CoTaskMemFree, DVASPECT_CONTENT, FORMATETC, IDataObject, TYMED_HGLOBAL},
      Ole::{
        CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, IDropTarget, IDropTarget_Impl,
        RegisterDragDrop, ReleaseStgMedium, RevokeDragDrop,
      },
      SystemServices::MODIFIERKEYS_FLAGS,
    },
    UI::{
      Shell::{
        DragQueryFileW, HDROP, IShellItemArray, SHCreateShellItemArrayFromDataObject,
        SIGDN_FILESYSPATH,
      },
      WindowsAndMessaging::{EnumChildWindows, IsChild},
    },
  },
  core::{BOOL, implement},
};

pub(crate) struct DragDropController {
  /// The webview container window. Its descendants are the windows the drop targets are
  /// registered on.
  hwnd: HWND,
  /// The windows we registered a drop target on, and their targets.
  drop_targets: Vec<(HWND, IDropTarget)>,
  /// The only strong reference to the handler. The drop targets hold weak ones, because OLE
  /// can keep a target alive forever: revoking fails with `DRAGDROP_E_NOTREGISTERED` when the
  /// registration on a window owned by the WebView2 browser process is already gone.
  handler: Rc<dyn Fn(DragDropEvent) -> bool>,
}

impl DragDropController {
  #[inline]
  pub(crate) fn new(hwnd: HWND, handler: Box<dyn Fn(DragDropEvent) -> bool>) -> Self {
    let mut controller = DragDropController {
      hwnd,
      drop_targets: Vec::new(),
      handler: Rc::from(handler),
    };

    // Enumerate child windows to find the WebView2 "window" and override!
    {
      let mut callback = |hwnd| controller.inject_in_hwnd(hwnd);
      let mut trait_obj: &mut dyn FnMut(HWND) -> bool = &mut callback;
      let closure_pointer_pointer: *mut c_void = unsafe { std::mem::transmute(&mut trait_obj) };
      let lparam = LPARAM(closure_pointer_pointer as _);
      unsafe extern "system" fn enumerate_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        unsafe {
          let closure = &mut *(lparam.0 as *mut c_void as *mut &mut dyn FnMut(HWND) -> bool);
          closure(hwnd).into()
        }
      }
      let _ = unsafe { EnumChildWindows(Some(hwnd), Some(enumerate_callback), lparam) };
    }

    controller
  }

  #[inline]
  fn inject_in_hwnd(&mut self, hwnd: HWND) -> bool {
    let drag_drop_target: IDropTarget =
      DragDropTarget::new(hwnd, Rc::downgrade(&self.handler)).into();
    if unsafe { RevokeDragDrop(hwnd) } != Err(DRAGDROP_E_INVALIDHWND.into())
      && unsafe { RegisterDragDrop(hwnd, &drag_drop_target) }.is_ok()
    {
      self.drop_targets.push((hwnd, drag_drop_target));
    }

    true
  }
}

impl Drop for DragDropController {
  /// Revokes every drop target registered on a window that is still a descendant of the
  /// container.
  ///
  /// A registered window must be revoked before it is destroyed, otherwise OLE keeps its
  /// reference to the target forever. So this must run while the descendants of the container
  /// still exist: at the latest from the `WM_DESTROY` of the container.
  fn drop(&mut self) {
    for (hwnd, _) in self.drop_targets.drain(..) {
      // A window WebView2 has already destroyed cannot be revoked anymore, and its handle may
      // have been reused by an unrelated window.
      if !unsafe { IsChild(self.hwnd, hwnd) }.as_bool() {
        continue;
      }
      let _ = unsafe { RevokeDragDrop(hwnd) };
    }
  }
}

#[implement(IDropTarget)]
pub struct DragDropTarget {
  hwnd: HWND,
  // Held as a `Weak` on purpose: `RevokeDragDrop` can fail (e.g. with
  // `DRAGDROP_E_NOTREGISTERED` on `Chrome_WidgetWin_1`), in which case this target stays
  // alive. With an `Rc` the handler and everything it captured would stay alive with it.
  listener: Weak<dyn Fn(DragDropEvent) -> bool>,
  cursor_effect: UnsafeCell<DROPEFFECT>,
  enter_is_valid: UnsafeCell<bool>, /* If the currently hovered item is not valid there must not be any `HoveredFileCancelled` emitted */
}

impl DragDropTarget {
  pub fn new(hwnd: HWND, listener: Weak<dyn Fn(DragDropEvent) -> bool>) -> DragDropTarget {
    Self {
      hwnd,
      listener,
      cursor_effect: DROPEFFECT_NONE.into(),
      enter_is_valid: false.into(),
    }
  }

  /// Calls the handler, unless its webview is already gone.
  fn emit(&self, event: DragDropEvent) {
    if let Some(listener) = self.listener.upgrade() {
      listener(event);
    }
  }

  /// Calls `callback` for every dropped file-system path.
  ///
  /// Returns whether the data holds file paths.
  unsafe fn iterate_filenames<F>(
    data_obj: windows_core::Ref<'_, IDataObject>,
    mut callback: F,
  ) -> bool
  where
    F: FnMut(PathBuf),
  {
    unsafe {
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
        Ok(mut medium) => {
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

          // `GetData` hands us the ownership of the medium on every call, `DragEnter` included.
          ReleaseStgMedium(&mut medium);

          true
        }
        // File Explorer cannot build CF_HDROP when any dragged path is
        // MAX_PATH (260) characters or longer: GetData fails with
        // ERROR_INSUFFICIENT_BUFFER (0x8007007A), even with long paths
        // enabled. The shell item array has no such limit.
        Err(_) if Self::iterate_shell_items(&data_obj, &mut callback) => true,
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
          false
        }
      }
    }
  }

  /// Reads file-system paths from the shell item array in `data_obj`.
  /// Returns true when at least one path was found.
  unsafe fn iterate_shell_items<F>(
    data_obj: &windows_core::Ref<'_, IDataObject>,
    callback: &mut F,
  ) -> bool
  where
    F: FnMut(PathBuf),
  {
    let Some(data_obj) = data_obj.as_ref() else {
      return false;
    };
    unsafe {
      let Ok(items) = SHCreateShellItemArrayFromDataObject::<_, IShellItemArray>(data_obj) else {
        return false;
      };
      let count = items.GetCount().unwrap_or(0);

      let mut found = false;
      for i in 0..count {
        let Ok(item) = items.GetItemAt(i) else {
          continue;
        };
        // Fails for items with no file-system path (virtual or device items).
        let Ok(name) = item.GetDisplayName(SIGDN_FILESYSPATH) else {
          continue;
        };
        callback(OsString::from_wide(name.as_wide()).into());
        CoTaskMemFree(Some(name.0 as _));
        found = true;
      }
      found
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

    // Reset the state left by the previous drag. Otherwise a drag with no
    // file paths that follows an accepted one keeps `enter_is_valid` set and
    // still emits `Over` and a `Drop` with no paths.
    unsafe {
      *self.enter_is_valid.get() = false;
      *self.cursor_effect.get() = DROPEFFECT_NONE;
    }

    let mut paths = Vec::new();
    let enter_is_valid =
      unsafe { DragDropTarget::iterate_filenames(pDataObj, |path| paths.push(path)) };

    if !enter_is_valid {
      unsafe { *pdwEffect = DROPEFFECT_NONE };
      return Ok(());
    };

    unsafe {
      *self.enter_is_valid.get() = enter_is_valid;
    }

    self.emit(DragDropEvent::Enter {
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
      self.emit(DragDropEvent::Over {
        position: (pt.x as _, pt.y as _),
      });
    }

    unsafe { *pdwEffect = *self.cursor_effect.get() };
    Ok(())
  }

  fn DragLeave(&self) -> windows::core::Result<()> {
    if unsafe { *self.enter_is_valid.get() } {
      self.emit(DragDropEvent::Leave);
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
      unsafe { DragDropTarget::iterate_filenames(pDataObj, |path| paths.push(path)) };
      self.emit(DragDropEvent::Drop {
        paths,
        position: (pt.x as _, pt.y as _),
      });
    }

    Ok(())
  }
}
