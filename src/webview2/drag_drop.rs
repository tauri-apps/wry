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
  rc::Rc,
};

use windows::{
  Win32::{
    Foundation::{DRAGDROP_E_INVALIDHWND, HWND, LPARAM, POINT, POINTL},
    Graphics::Gdi::ScreenToClient,
    System::{
      Com::{CoTaskMemFree, DVASPECT_CONTENT, FORMATETC, IDataObject, TYMED_HGLOBAL},
      Ole::{
        CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, IDropTarget, IDropTarget_Impl,
        RegisterDragDrop, RevokeDragDrop,
      },
      SystemServices::MODIFIERKEYS_FLAGS,
    },
    UI::{
      Shell::{
        DragFinish, DragQueryFileW, HDROP, IShellItemArray, SHCreateShellItemArrayFromDataObject,
        SIGDN_FILESYSPATH,
      },
      WindowsAndMessaging::EnumChildWindows,
    },
  },
  core::{BOOL, implement},
};

#[derive(Default)]
pub(crate) struct DragDropController {
  drop_targets: Vec<IDropTarget>,
}

impl DragDropController {
  #[inline]
  pub(crate) fn new(hwnd: HWND, handler: Box<dyn Fn(DragDropEvent) -> bool>) -> Self {
    let mut controller = DragDropController::default();

    let handler = Rc::new(handler);

    // Enumerate child windows to find the WebView2 "window" and override!
    {
      let mut callback = |hwnd| controller.inject_in_hwnd(hwnd, handler.clone());
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
  fn inject_in_hwnd(&mut self, hwnd: HWND, handler: Rc<dyn Fn(DragDropEvent) -> bool>) -> bool {
    let drag_drop_target: IDropTarget = DragDropTarget::new(hwnd, handler).into();
    if unsafe { RevokeDragDrop(hwnd) } != Err(DRAGDROP_E_INVALIDHWND.into())
      && unsafe { RegisterDragDrop(hwnd, &drag_drop_target) }.is_ok()
    {
      self.drop_targets.push(drag_drop_target);
    }

    true
  }
}

enum FileDropSource {
  HDrop(HDROP),
  ShellItemArray,
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

  /// Calls `callback` for every dropped file-system path.
  ///
  /// Returns the source of the paths, or `None` when the data holds no file paths.
  /// A `FileDropSource::HDrop` handle must be released with `DragFinish` on drop.
  unsafe fn iterate_filenames<F>(
    data_obj: windows_core::Ref<'_, IDataObject>,
    mut callback: F,
  ) -> Option<FileDropSource>
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

          Some(FileDropSource::HDrop(hdrop))
        }
        // File Explorer cannot build CF_HDROP when any dragged path is
        // MAX_PATH (260) characters or longer: GetData fails with
        // ERROR_INSUFFICIENT_BUFFER (0x8007007A), even with long paths
        // enabled. The shell item array has no such limit.
        Err(_) if Self::iterate_shell_items(&data_obj, &mut callback) => {
          Some(FileDropSource::ShellItemArray)
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
    let source = unsafe { DragDropTarget::iterate_filenames(pDataObj, |path| paths.push(path)) };

    let enter_is_valid = source.is_some();

    if !enter_is_valid {
      unsafe { *pdwEffect = DROPEFFECT_NONE };
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
      let source = unsafe { DragDropTarget::iterate_filenames(pDataObj, |path| paths.push(path)) };
      (self.listener)(DragDropEvent::Drop {
        paths,
        position: (pt.x as _, pt.y as _),
      });

      if let Some(FileDropSource::HDrop(hdrop)) = source {
        unsafe { DragFinish(hdrop) };
      }
    }

    Ok(())
  }
}
