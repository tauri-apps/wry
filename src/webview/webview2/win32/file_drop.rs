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
  sync::atomic::{AtomicUsize, Ordering},
};

use winapi::shared::windef::HWND;

use crate::application::window::Window;

pub(crate) struct FileDropController {
  drop_targets: Vec<*mut IDropTarget>,
}
impl Drop for FileDropController {
  fn drop(&mut self) {
    // Safety: this could dereference a null ptr.
    // This should never be a null ptr unless something goes wrong in Windows.
    unsafe {
      for ptr in &self.drop_targets {
        Box::from_raw(*ptr);
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
    // Safety: WinAPI calls are unsafe
    unsafe {
      let file_drop_handler = IDropTarget::new(hwnd, window, listener);
      let handler_interface_ptr =
        &mut (*file_drop_handler.data).interface as winapi::um::oleidl::LPDROPTARGET;

      if winapi::um::ole2::RevokeDragDrop(hwnd) != winapi::shared::winerror::DRAGDROP_E_INVALIDHWND
        && winapi::um::ole2::RegisterDragDrop(hwnd, handler_interface_ptr) == S_OK
      {
        // Not a great solution. But there is no reliable way to get the window handle of the webview, for whatever reason...
        self
          .drop_targets
          .push(Box::into_raw(Box::new(file_drop_handler)));
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

  let lparam = closure_pointer_pointer as winapi::shared::minwindef::LPARAM;
  unsafe { winapi::um::winuser::EnumChildWindows(hwnd, Some(enumerate_callback), lparam) };
}

unsafe extern "system" fn enumerate_callback(
  hwnd: HWND,
  lparam: winapi::shared::minwindef::LPARAM,
) -> winapi::shared::minwindef::BOOL {
  let closure = &mut *(lparam as *mut c_void as *mut &mut dyn FnMut(HWND) -> bool);
  if closure(hwnd) {
    winapi::shared::minwindef::TRUE
  } else {
    winapi::shared::minwindef::FALSE
  }
}

// The below code has been ripped from tao - if only they'd `pub use` this!
// https://github.com/rust-windowing/winit/blob/b9f3d333e41464457f6e42640793bf88b9563727/src/platform_impl/windows/drop_handler.rs
// Safety: WinAPI calls are unsafe

use winapi::{
  shared::{
    guiddef::REFIID,
    minwindef::{DWORD, UINT, ULONG},
    windef::POINTL,
    winerror::S_OK,
  },
  um::{
    objidl::IDataObject,
    oleidl::{IDropTarget as NativeIDropTarget, IDropTargetVtbl, DROPEFFECT_COPY, DROPEFFECT_NONE},
    shellapi, unknwnbase,
    winnt::HRESULT,
  },
};

#[allow(non_camel_case_types)]
#[repr(C)]
struct IDropTargetData {
  pub interface: NativeIDropTarget,
  listener: Rc<dyn Fn(&Window, FileDropEvent) -> bool>,
  refcount: AtomicUsize,
  hwnd: HWND,
  window: Rc<Window>,
  cursor_effect: DWORD,
  hovered_is_valid: bool, /* If the currently hovered item is not valid there must not be any `HoveredFileCancelled` emitted */
}

#[allow(non_camel_case_types)]
pub struct IDropTarget {
  data: *mut IDropTargetData,
}

#[allow(non_snake_case)]
impl IDropTarget {
  fn new(
    hwnd: HWND,
    window: Rc<Window>,
    listener: Rc<dyn Fn(&Window, FileDropEvent) -> bool>,
  ) -> IDropTarget {
    let data = Box::new(IDropTargetData {
      listener,
      interface: NativeIDropTarget {
        lpVtbl: &DROP_TARGET_VTBL as *const IDropTargetVtbl,
      },
      refcount: AtomicUsize::new(1),
      hwnd,
      window,
      cursor_effect: DROPEFFECT_NONE,
      hovered_is_valid: false,
    });
    IDropTarget {
      data: Box::into_raw(data),
    }
  }

  // Implement IUnknown
  pub unsafe extern "system" fn QueryInterface(
    _this: *mut unknwnbase::IUnknown,
    _riid: REFIID,
    _ppvObject: *mut *mut winapi::ctypes::c_void,
  ) -> HRESULT {
    // This function doesn't appear to be required for an `IDropTarget`.
    // An implementation would be nice however.
    unimplemented!();
  }

  pub unsafe extern "system" fn AddRef(this: *mut unknwnbase::IUnknown) -> ULONG {
    let drop_handler_data = Self::from_interface(this);
    let count = drop_handler_data.refcount.fetch_add(1, Ordering::Release) + 1;
    count as ULONG
  }

  pub unsafe extern "system" fn Release(this: *mut unknwnbase::IUnknown) -> ULONG {
    let drop_handler = Self::from_interface(this);
    let count = drop_handler.refcount.fetch_sub(1, Ordering::Release) - 1;
    if count == 0 {
      // Destroy the underlying data
      Box::from_raw(drop_handler as *mut IDropTargetData);
    }
    count as ULONG
  }

  pub unsafe extern "system" fn DragEnter(
    this: *mut NativeIDropTarget,
    pDataObj: *const IDataObject,
    _grfKeyState: DWORD,
    _pt: *const POINTL,
    pdwEffect: *mut DWORD,
  ) -> HRESULT {
    let mut paths = Vec::new();

    let drop_handler = Self::from_interface(this);
    let hdrop = Self::collect_paths(pDataObj, &mut paths);
    drop_handler.hovered_is_valid = hdrop.is_some();
    drop_handler.cursor_effect = if drop_handler.hovered_is_valid {
      DROPEFFECT_COPY
    } else {
      DROPEFFECT_NONE
    };
    *pdwEffect = drop_handler.cursor_effect;

    (drop_handler.listener)(&drop_handler.window, FileDropEvent::Hovered(paths));

    S_OK
  }

  pub unsafe extern "system" fn DragOver(
    this: *mut NativeIDropTarget,
    _grfKeyState: DWORD,
    _pt: *const POINTL,
    pdwEffect: *mut DWORD,
  ) -> HRESULT {
    let drop_handler = Self::from_interface(this);
    *pdwEffect = drop_handler.cursor_effect;

    S_OK
  }

  pub unsafe extern "system" fn DragLeave(this: *mut NativeIDropTarget) -> HRESULT {
    let drop_handler = Self::from_interface(this);
    if drop_handler.hovered_is_valid {
      (drop_handler.listener)(&drop_handler.window, FileDropEvent::Cancelled);
    }

    S_OK
  }

  pub unsafe extern "system" fn Drop(
    this: *mut NativeIDropTarget,
    pDataObj: *const IDataObject,
    _grfKeyState: DWORD,
    _pt: *const POINTL,
    _pdwEffect: *mut DWORD,
  ) -> HRESULT {
    let mut paths = Vec::new();

    let drop_handler = Self::from_interface(this);
    let hdrop = Self::collect_paths(pDataObj, &mut paths);
    if let Some(hdrop) = hdrop {
      shellapi::DragFinish(hdrop);
    }

    (drop_handler.listener)(&drop_handler.window, FileDropEvent::Dropped(paths));

    S_OK
  }

  unsafe fn from_interface<'a, InterfaceT>(this: *mut InterfaceT) -> &'a mut IDropTargetData {
    &mut *(this as *mut _)
  }

  unsafe fn collect_paths(
    data_obj: *const IDataObject,
    paths: &mut Vec<PathBuf>,
  ) -> Option<shellapi::HDROP> {
    use winapi::{
      shared::{
        winerror::{DV_E_FORMATETC, SUCCEEDED},
        wtypes::{CLIPFORMAT, DVASPECT_CONTENT},
      },
      um::{
        objidl::{FORMATETC, TYMED_HGLOBAL},
        shellapi::DragQueryFileW,
        winuser::CF_HDROP,
      },
    };

    let drop_format = FORMATETC {
      cfFormat: CF_HDROP as CLIPFORMAT,
      ptd: ptr::null(),
      dwAspect: DVASPECT_CONTENT,
      lindex: -1,
      tymed: TYMED_HGLOBAL,
    };

    let mut medium = std::mem::zeroed();
    let get_data_result = (*data_obj).GetData(&drop_format, &mut medium);
    if SUCCEEDED(get_data_result) {
      let hglobal = (*medium.u).hGlobal();
      let hdrop = (*hglobal) as shellapi::HDROP;

      // The second parameter (0xFFFFFFFF) instructs the function to return the item count
      let item_count = DragQueryFileW(hdrop, 0xFFFFFFFF, ptr::null_mut(), 0);

      for i in 0..item_count {
        // Get the length of the path string NOT including the terminating null character.
        // Previously, this was using a fixed size array of MAX_PATH length, but the
        // Windows API allows longer paths under certain circumstances.
        let character_count = DragQueryFileW(hdrop, i, ptr::null_mut(), 0) as usize;
        let str_len = character_count + 1;

        // Fill path_buf with the null-terminated file name
        let mut path_buf = Vec::with_capacity(str_len);
        DragQueryFileW(hdrop, i, path_buf.as_mut_ptr(), str_len as UINT);
        path_buf.set_len(str_len);

        paths.push(OsString::from_wide(&path_buf[0..character_count]).into());
      }

      Some(hdrop)
    } else if get_data_result == DV_E_FORMATETC {
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

impl Drop for IDropTarget {
  fn drop(&mut self) {
    unsafe {
      IDropTarget::Release(self.data as *mut unknwnbase::IUnknown);
    }
  }
}

static DROP_TARGET_VTBL: IDropTargetVtbl = IDropTargetVtbl {
  parent: unknwnbase::IUnknownVtbl {
    QueryInterface: IDropTarget::QueryInterface,
    AddRef: IDropTarget::AddRef,
    Release: IDropTarget::Release,
  },
  DragEnter: IDropTarget::DragEnter,
  DragOver: IDropTarget::DragOver,
  DragLeave: IDropTarget::DragLeave,
  Drop: IDropTarget::Drop,
};
