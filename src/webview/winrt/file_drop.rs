// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use windows::{Abi, Interface};

use windows_webview2::Windows::Win32::{
  Com::{self as com, DVASPECT, TYMED},
  DisplayDevices::POINTL,
  Shell::{self as shell, HDROP},
  SystemServices::{self, BOOL, CLIPBOARD_FORMATS, PWSTR},
  WindowsAndMessaging::{self, HWND, LPARAM},
};

use crate::{application::window::Window, webview::FileDropEvent};

// A silly implementation of file drop handling for Windows!
// This can be pretty much entirely replaced when WebView2 SDK 1.0.721-prerelease becomes stable.
// https://docs.microsoft.com/en-us/microsoft-edge/webview2/releasenotes#10721-prerelease
// https://docs.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2experimentalcompositioncontroller3?view=webview2-1.0.721-prerelease&preserve-view=true

use std::{
  ffi::OsString,
  mem,
  os::{raw::c_void, windows::ffi::OsStringExt},
  path::PathBuf,
  ptr::null_mut,
  rc::Rc,
  sync::atomic::{AtomicU32, Ordering},
};

pub(crate) struct FileDropController {
  drop_targets: Vec<(HWND, *mut DropTarget)>,
}

impl Drop for FileDropController {
  fn drop(&mut self) {
    // Safety: this could dereference a null ptr.
    // This should never be a null ptr unless something goes wrong in Windows.
    unsafe {
      for (hwnd, ptr) in &self.drop_targets {
        let _ = com::RevokeDragDrop(hwnd);
        DropTarget::Release(*ptr as *mut _);
      }
    }
  }
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
    // Safety: Win32 calls are unsafe
    unsafe {
      if com::RevokeDragDrop(hwnd).0 != SystemServices::DRAGDROP_E_INVALIDHWND.0 {
        let mut drop_target = Box::new(DropTarget::new(hwnd, window, listener));
        let interface: windows::Result<com::IDropTarget> =
          from_abi(drop_target.as_mut() as *mut _ as *mut _);
        if let Ok(interface) = interface {
          if com::RegisterDragDrop(hwnd, interface).is_ok() {
            // Not a great solution. But there is no reliable way to get the window handle of the webview, for whatever reason...
            self.drop_targets.push((hwnd, Box::into_raw(drop_target)));
          }
        }
      }
    }

    true
  }
}

// https://gist.github.com/application-developer-DA/5a460d9ca02948f1d2bfa53100c941da
// Safety: Win32 calls are unsafe

fn enumerate_child_windows<F>(hwnd: HWND, mut callback: F)
where
  F: FnMut(HWND) -> bool,
{
  let mut trait_obj: &mut dyn FnMut(HWND) -> bool = &mut callback;
  let closure_pointer_pointer: *mut c_void = unsafe { mem::transmute(&mut trait_obj) };

  let lparam = LPARAM(closure_pointer_pointer as _);
  unsafe { WindowsAndMessaging::EnumChildWindows(hwnd, Some(enumerate_callback), lparam) };
}

extern "system" fn enumerate_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
  unsafe {
    let closure = &mut *(lparam.0 as *mut c_void as *mut &mut dyn FnMut(HWND) -> bool);
    closure(hwnd).into()
  }
}

unsafe fn from_abi<I: Interface>(this: windows::RawPtr) -> windows::Result<I> {
  let unknown = windows::IUnknown::from_abi(this)?;
  unknown.vtable().1(unknown.abi()); // AddRef to balance the Release called in IUnknown::drop
  unknown.cast()
}

// The below code has been ripped from Winit - if only they'd `pub use` this!
// https://github.com/rust-windowing/winit/blob/b9f3d333e41464457f6e42640793bf88b9563727/src/platform_impl/windows/drop_handler.rs
// Safety: Win32 calls are unsafe

#[allow(non_camel_case_types)]
#[repr(C)]
struct DropTarget {
  vtable: *const IDropTarget_vtable,
  listener: Rc<dyn Fn(&Window, FileDropEvent) -> bool>,
  refcount: AtomicU32,
  hwnd: HWND,
  window: Rc<Window>,
  cursor_effect: u32,
  hovered_is_valid: bool, /* If the currently hovered item is not valid there must not be any `HoveredFileCancelled` emitted */
}

#[allow(non_snake_case)]
impl DropTarget {
  fn new(
    hwnd: HWND,
    window: Rc<Window>,
    listener: Rc<dyn Fn(&Window, FileDropEvent) -> bool>,
  ) -> DropTarget {
    DropTarget {
      vtable: &DROP_TARGET_VTBL,
      listener,
      refcount: AtomicU32::new(1),
      hwnd,
      window,
      cursor_effect: com::DROPEFFECT_NONE,
      hovered_is_valid: false,
    }
  }

  // Implement IUnknown
  pub unsafe extern "system" fn QueryInterface(
    this: windows::RawPtr,
    iid: &windows::Guid,
    interface: *mut windows::RawPtr,
  ) -> windows::ErrorCode {
    if interface.is_null() {
      windows::ErrorCode::E_POINTER
    } else {
      match *iid {
        windows::IUnknown::IID | com::IDropTarget::IID => {
          DropTarget::AddRef(this);
          *interface = this;
          windows::ErrorCode::S_OK
        }
        _ => windows::ErrorCode::E_NOINTERFACE,
      }
    }
  }

  pub unsafe extern "system" fn AddRef(this: windows::RawPtr) -> u32 {
    let drop_target = Self::from_interface(this);
    drop_target.refcount.fetch_add(1, Ordering::Release) + 1
  }

  pub unsafe extern "system" fn Release(this: windows::RawPtr) -> u32 {
    let drop_target = Self::from_interface(this);
    let count = drop_target.refcount.fetch_sub(1, Ordering::Release) - 1;
    if count == 0 {
      // Destroy the underlying data
      Box::from_raw(drop_target);
    }
    count
  }

  pub unsafe extern "system" fn DragEnter(
    this: windows::RawPtr,
    pDataObj: windows::RawPtr,
    _grfKeyState: u32,
    _pt: *const POINTL,
    pdwEffect: *mut u32,
  ) -> windows::ErrorCode {
    let mut paths = Vec::new();

    let drop_handler = Self::from_interface(this);
    let data_obj = from_abi(pDataObj);
    if let Ok(data_obj) = data_obj {
      let hdrop = Self::collect_paths(&data_obj, &mut paths);

      drop_handler.hovered_is_valid = hdrop.is_some();
      drop_handler.cursor_effect = if drop_handler.hovered_is_valid {
        com::DROPEFFECT_COPY
      } else {
        com::DROPEFFECT_NONE
      };
      *pdwEffect = drop_handler.cursor_effect;

      (drop_handler.listener)(&drop_handler.window, FileDropEvent::Hovered(paths));
    }

    windows::ErrorCode::S_OK
  }

  pub unsafe extern "system" fn DragOver(
    this: windows::RawPtr,
    _grfKeyState: u32,
    _pt: *const POINTL,
    pdwEffect: *mut u32,
  ) -> windows::ErrorCode {
    let drop_handler = Self::from_interface(this);
    *pdwEffect = drop_handler.cursor_effect;

    windows::ErrorCode::S_OK
  }

  pub unsafe extern "system" fn DragLeave(this: windows::RawPtr) -> windows::ErrorCode {
    let drop_handler = Self::from_interface(this);
    if drop_handler.hovered_is_valid {
      (drop_handler.listener)(&drop_handler.window, FileDropEvent::Cancelled);
    }

    windows::ErrorCode::S_OK
  }

  pub unsafe extern "system" fn Drop(
    this: windows::RawPtr,
    pDataObj: windows::RawPtr,
    _grfKeyState: u32,
    _pt: *const POINTL,
    _pdwEffect: *mut u32,
  ) -> windows::ErrorCode {
    let mut paths = Vec::new();

    let drop_handler = Self::from_interface(this);
    let data_obj = from_abi(pDataObj);
    if let Ok(data_obj) = data_obj {
      let hdrop = Self::collect_paths(&data_obj, &mut paths);
      if let Some(hdrop) = hdrop {
        shell::DragFinish(hdrop);
      }
    }

    (drop_handler.listener)(&drop_handler.window, FileDropEvent::Dropped(paths));

    windows::ErrorCode::S_OK
  }

  unsafe fn from_interface<'a>(this: windows::RawPtr) -> &'a mut DropTarget {
    &mut *(this as *mut _)
  }

  unsafe fn collect_paths(data_obj: &com::IDataObject, paths: &mut Vec<PathBuf>) -> Option<HDROP> {
    let mut drop_format = com::FORMATETC {
      cfFormat: CLIPBOARD_FORMATS::CF_HDROP.0 as u16,
      ptd: null_mut(),
      dwAspect: DVASPECT::DVASPECT_CONTENT.0 as u32,
      lindex: -1,
      tymed: TYMED::TYMED_HGLOBAL.0 as u32,
    };

    let mut medium = mem::zeroed();
    let get_data_result = data_obj.GetData(&mut drop_format, &mut medium);
    if get_data_result.is_ok() {
      let hglobal = medium.Anonymous.hGlobal;
      let hdrop = HDROP(hglobal);

      // The second parameter (0xFFFFFFFF) instructs the function to return the item count
      let item_count = shell::DragQueryFileW(hdrop, 0xFFFFFFFF, PWSTR(null_mut()), 0);

      for i in 0..item_count {
        // Get the length of the path string NOT including the terminating null character.
        // Previously, this was using a fixed size array of MAX_PATH length, but the
        // Windows API allows longer paths under certain circumstances.
        let character_count = shell::DragQueryFileW(hdrop, i, PWSTR(null_mut()), 0) as usize;
        let str_len = character_count + 1;

        // Fill path_buf with the null-terminated file name
        let mut path_buf = Vec::with_capacity(str_len);
        shell::DragQueryFileW(hdrop, i, PWSTR(path_buf.as_mut_ptr()), str_len as u32);
        path_buf.set_len(str_len);

        paths.push(OsString::from_wide(&path_buf[0..character_count]).into());
      }

      Some(hdrop)
    } else if get_data_result.0 == SystemServices::DV_E_FORMATETC.0 {
      // If the dropped item is not a file this error will occur.
      // In this case it is OK to return without taking further action.
      log::warn!("Error occured while processing dropped/hovered item: item is not a file.");
      None
    } else {
      log::warn!("Unexpected error occured while processing dropped/hovered item.");
      None
    }
  }
}

#[repr(C)]
#[allow(non_snake_case)]
struct IDropTarget_vtable {
  QueryInterface: unsafe extern "system" fn(
    this: windows::RawPtr,
    iid: &windows::Guid,
    interface: *mut windows::RawPtr,
  ) -> windows::ErrorCode,

  AddRef: unsafe extern "system" fn(this: windows::RawPtr) -> u32,

  Release: unsafe extern "system" fn(this: windows::RawPtr) -> u32,

  DragEnter: unsafe extern "system" fn(
    this: windows::RawPtr,
    pDataObj: windows::RawPtr,
    grfKeyState: u32,
    pt: *const POINTL,
    pdwEffect: *mut u32,
  ) -> windows::ErrorCode,

  DragOver: unsafe extern "system" fn(
    this: windows::RawPtr,
    grfKeyState: u32,
    pt: *const POINTL,
    pdwEffect: *mut u32,
  ) -> windows::ErrorCode,

  DragLeave: unsafe extern "system" fn(this: windows::RawPtr) -> windows::ErrorCode,

  Drop: unsafe extern "system" fn(
    this: windows::RawPtr,
    pDataObj: windows::RawPtr,
    grfKeyState: u32,
    pt: *const POINTL,
    pdwEffect: *mut u32,
  ) -> windows::ErrorCode,
}

static DROP_TARGET_VTBL: IDropTarget_vtable = IDropTarget_vtable {
  QueryInterface: DropTarget::QueryInterface,
  AddRef: DropTarget::AddRef,
  Release: DropTarget::Release,
  DragEnter: DropTarget::DragEnter,
  DragOver: DropTarget::DragOver,
  DragLeave: DropTarget::DragLeave,
  Drop: DropTarget::Drop,
};
