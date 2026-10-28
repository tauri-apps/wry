// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! DirectComposition (visual) hosting for WebView2.
//!
//! Creates the webview through `CreateCoreWebView2CompositionController`
//! targeting a caller-supplied `IDCompositionVisual` and forwards host-window
//! input to it. In this mode WebView2 draws nothing to any HWND: its output
//! goes to the visual, and the host window (which owns the Win32 input queue)
//! must hand-deliver mouse/pointer events and cursor updates. Keyboard input
//! needs no forwarding — once `MoveFocus` is called, WebView2's hidden input
//! window takes Win32 focus and receives keys and IME natively.

use std::{
  cell::{Cell, RefCell},
  collections::HashMap,
  sync::mpsc,
};

use webview2_com::{
  CreateCoreWebView2CompositionControllerCompletedHandler, CursorChangedEventHandler,
  Microsoft::Web::WebView2::Win32::*,
};
use windows::{
  core::{IUnknown, Interface, HSTRING, PCWSTR},
  Win32::{
    Foundation::{E_POINTER, E_UNEXPECTED, HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::ScreenToClient,
    UI::{
      Input::{
        KeyboardAndMouse::{
          ReleaseCapture, SetCapture, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
        },
        Pointer::{GetPointerPenInfo, GetPointerTouchInfo, GetPointerType, POINTER_INFO},
      },
      Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
      WindowsAndMessaging::{
        GetClientRect, LoadCursorW, SendMessageW, SetCursor, HCURSOR, HTCLIENT, IDC_ARROW,
        POINTER_INPUT_TYPE, PT_PEN, PT_TOUCH, SIZE_MINIMIZED, WM_DESTROY, WM_ENTERSIZEMOVE,
        WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDBLCLK, WM_MBUTTONDOWN,
        WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_MOVE, WM_MOVING,
        WM_POINTERDOWN, WM_POINTERUP, WM_POINTERUPDATE, WM_RBUTTONDBLCLK, WM_RBUTTONDOWN,
        WM_RBUTTONUP, WM_SETCURSOR, WM_SETFOCUS, WM_SIZE, WM_USER, WM_XBUTTONDBLCLK,
        WM_XBUTTONDOWN, WM_XBUTTONUP,
      },
    },
  },
};

use crate::{Error, Result};

// Distinct from wry's PARENT_SUBCLASS_ID (WM_USER + 0x64), PARENT_DESTROY_MESSAGE
// (+ 0x65) and MAIN_THREAD_DISPATCHER_SUBCLASS_ID (+ 0x66) so both subclass
// families can coexist on one HWND without colliding.
const COMPOSITION_SUBCLASS_ID: u32 = WM_USER + 0x67;
const COMPOSITION_DESTROY_MESSAGE: u32 = WM_USER + 0x68;

// windows-rs files WM_MOUSELEAVE under `Win32_UI_Controls`; defined locally to
// avoid pulling a whole feature for one message constant.
const WM_MOUSELEAVE: u32 = 0x02A3;

thread_local! {
  /// Composition-visual targets registered per host HWND, for webviews built
  /// by an embedding layer (e.g. tauri-runtime-wry) that constructs the
  /// `WebViewBuilder` internally and offers no way to call
  /// [`with_composition_visual_target`](crate::WebViewBuilderExtWindows::with_composition_visual_target)
  /// on it. The host registers the visual for its HWND *before* asking the
  /// embedder to create the webview; webview creation consumes it (once) when
  /// no builder-supplied target is set. Main-thread only by construction —
  /// registration and webview creation both happen on the UI thread, which is
  /// also the COM STA the visual lives in.
  static PENDING_VISUAL_TARGETS: RefCell<HashMap<isize, IUnknown>> =
    RefCell::new(HashMap::new());
}

/// Registers `visual` (an `IDCompositionVisual`, passed as `IUnknown`) as the
/// composition target for the next webview created on the host window `hwnd`.
///
/// This is the out-of-band counterpart of
/// [`WebViewBuilderExtWindows::with_composition_visual_target`](crate::WebViewBuilderExtWindows::with_composition_visual_target),
/// for callers that do not construct the `WebViewBuilder` themselves (for
/// example applications embedding through `tauri-runtime-wry`, which builds
/// the webview internally). The entry is keyed by HWND so concurrent window
/// creations cannot cross-wire, and is consumed (removed) by the first webview
/// created on that window; registering again replaces any pending entry for
/// the same HWND. A builder-supplied target always takes precedence.
///
/// Must be called from the UI thread, before the webview is built. An entry
/// that is never consumed holds one COM reference to the visual until the
/// thread exits.
pub fn register_composition_visual_target(hwnd: isize, visual: IUnknown) {
  PENDING_VISUAL_TARGETS.with(|targets| {
    targets.borrow_mut().insert(hwnd, visual);
  });
}

/// Takes (and removes) the visual registered for `hwnd`, if any.
pub(crate) fn take_registered_visual_target(hwnd: isize) -> Option<IUnknown> {
  PENDING_VISUAL_TARGETS.with(|targets| targets.borrow_mut().remove(&hwnd))
}

/// Per-host state carried through the subclass `dwrefdata`. Boxed at attach,
/// freed on `WM_DESTROY`/[`COMPOSITION_DESTROY_MESSAGE`] with the same
/// null-out-refdata double-free guard the parent subclass uses.
struct CompositionHost {
  controller: ICoreWebView2Controller,
  comp_controller: ICoreWebView2CompositionController,
  /// `ICoreWebView2Environment3` when available — required to build
  /// `ICoreWebView2PointerInfo` for touch/pen forwarding. `None` disables
  /// pointer forwarding only; mouse forwarding never depends on it.
  env3: Option<ICoreWebView2Environment3>,
  /// Cursor last requested by the webview (`CursorChanged`), applied on
  /// `WM_SETCURSOR` while the hit test is `HTCLIENT`.
  cursor: Cell<HCURSOR>,
  /// Whether a `TrackMouseEvent(TME_LEAVE)` request is outstanding, so
  /// `WM_MOUSELEAVE` is delivered exactly once per hover session.
  mouse_tracking: Cell<bool>,
  cursor_changed_token: i64,
}

/// Creates a WebView2 composition controller parented (for input/ownership
/// purposes) to `hwnd`, sets `visual` as its root visual target, and returns
/// the object's `ICoreWebView2Controller` interface for the shared init path.
///
/// Mirrors `InnerWebView::create_controller`: the environment-10 options path
/// carries incognito + profile name + default background color; otherwise the
/// plain environment-3 creation is used (composition hosting does not exist
/// below environment 3) and the background color is applied post-creation.
pub fn create_composition_controller(
  hwnd: HWND,
  env: &ICoreWebView2Environment,
  incognito: bool,
  background_color: Option<(u8, u8, u8, u8)>,
  profile_name: Option<&str>,
  visual: &IUnknown,
) -> Result<ICoreWebView2Controller> {
  let (tx, rx) = mpsc::channel::<std::result::Result<ICoreWebView2CompositionController, Error>>();

  let handler = CreateCoreWebView2CompositionControllerCompletedHandler::create(Box::new(
    move |error_code, controller| {
      let result = (|| {
        error_code?;
        controller.ok_or_else(|| windows::core::Error::from(E_POINTER).into())
      })();
      tx.send(result)
        .map_err(|_| windows::core::Error::from(E_UNEXPECTED))
    },
  ));

  unsafe {
    if let Ok(env10) = env.cast::<ICoreWebView2Environment10>() {
      let controller_opts = env10.CreateCoreWebView2ControllerOptions()?;

      if let Some((r, g, b, mut a)) = background_color {
        if let Ok(opts3) = controller_opts.cast::<ICoreWebView2ControllerOptions3>() {
          if a != 0 {
            a = 255;
          }
          opts3.SetDefaultBackgroundColor(COREWEBVIEW2_COLOR {
            R: r,
            G: g,
            B: b,
            A: a,
          })?;
        }
      }

      controller_opts.SetIsInPrivateModeEnabled(incognito)?;

      if let Some(name) = profile_name {
        controller_opts.SetProfileName(&HSTRING::from(name))?;
      }

      env10.CreateCoreWebView2CompositionControllerWithOptions(hwnd, &controller_opts, &handler)?;
    } else {
      let env3 = env
        .cast::<ICoreWebView2Environment3>()
        .map_err(windows::core::Error::from)?;
      env3.CreateCoreWebView2CompositionController(hwnd, &handler)?;
    }
  }

  let comp_controller: ICoreWebView2CompositionController = webview2_com::wait_with_pump(rx)??;

  let controller: ICoreWebView2Controller =
    comp_controller.cast().map_err(windows::core::Error::from)?;

  unsafe {
    comp_controller.SetRootVisualTarget(visual)?;

    // The env-3 fallback path had no options object to carry the background
    // color; apply it through the controller like the windowed late path does.
    if let Some(color) = background_color {
      if env.cast::<ICoreWebView2Environment10>().is_err() {
        super::set_background_color(&controller, color)?;
      }
    }
  }

  Ok(controller)
}

/// Subclasses the host window to (a) keep controller bounds synced to the
/// client area, (b) forward mouse and touch/pen input to the composition
/// controller, and (c) apply the webview's cursor. The counterpart of
/// `InnerWebView::attach_parent_subclass` for composition mode.
pub unsafe fn attach_host_subclass(
  parent: HWND,
  controller: &ICoreWebView2Controller,
) -> Result<()> {
  let comp_controller: ICoreWebView2CompositionController =
    controller.cast().map_err(windows::core::Error::from)?;

  // Environment 3 is available by construction (the composition controller
  // could not have been created without it); fetch it back through the
  // webview's environment for pointer-info creation. A failure here only
  // disables touch/pen forwarding.
  let env3: Option<ICoreWebView2Environment3> = controller
    .CoreWebView2()
    .ok()
    .and_then(|webview| webview.cast::<ICoreWebView2_2>().ok())
    .and_then(|webview2| webview2.Environment().ok())
    .and_then(|env| env.cast::<ICoreWebView2Environment3>().ok());

  let default_cursor = LoadCursorW(None, IDC_ARROW).unwrap_or_default();

  let host = Box::new(CompositionHost {
    controller: controller.clone(),
    comp_controller: comp_controller.clone(),
    env3,
    cursor: Cell::new(default_cursor),
    mouse_tracking: Cell::new(false),
    cursor_changed_token: 0,
  });
  let host_ptr = Box::into_raw(host);

  // The webview pushes cursor changes (I-beam over text, pointer over links…);
  // remember the cursor for WM_SETCURSOR and apply it immediately, since the
  // mouse is over the webview when the change fires.
  let mut token = 0i64;
  let register = comp_controller.add_CursorChanged(
    &CursorChangedEventHandler::create(Box::new(move |sender, _args| {
      if let Some(sender) = sender {
        if let Some(cursor) = resolve_cursor(&sender) {
          // SAFETY: the host box outlives the registration — the destroy path
          // removes this handler before freeing the box, on the same thread.
          unsafe { (*host_ptr).cursor.set(cursor) };
          let _ = unsafe { SetCursor(Some(cursor)) };
        }
      }
      Ok(())
    })),
    &mut token,
  );
  if let Err(e) = register {
    drop(Box::from_raw(host_ptr));
    return Err(windows::core::Error::from(e).into());
  }
  (*host_ptr).cursor_changed_token = token;

  let result = SetWindowSubclass(
    parent,
    Some(host_subclass_proc),
    COMPOSITION_SUBCLASS_ID as usize,
    host_ptr as usize,
  );
  if !result.as_bool() {
    let _ = comp_controller.remove_CursorChanged(token);
    drop(Box::from_raw(host_ptr));
    return Err(Error::WebView2Error(webview2_com::Error::WindowsError(
      windows::core::Error::from(E_UNEXPECTED),
    )));
  }

  Ok(())
}

/// The cursor to apply for the webview's current state.
///
/// The `Cursor` property hands back an `HCURSOR` the WebView2 runtime
/// rasterised at the system's base DPI; on a display-scaled monitor Windows
/// stretches that bitmap and every cursor sourced from it reads fuzzy.
/// `SystemCursorId` names the same cursor as its `IDC_*` resource id instead,
/// so `LoadCursorW` yields the OS **shared** cursor, which the system renders
/// crisply at the monitor's DPI. Only a custom CSS cursor — which has no
/// system id (the property reports 0) — falls back to the runtime's bitmap
/// handle.
fn resolve_cursor(sender: &ICoreWebView2CompositionController) -> Option<HCURSOR> {
  let mut id = 0u32;
  if unsafe { sender.SystemCursorId(&mut id) }.is_ok() && id != 0 {
    if let Ok(cursor) = unsafe { LoadCursorW(None, PCWSTR(id as usize as *const u16)) } {
      return Some(cursor);
    }
  }
  let mut cursor = HCURSOR::default();
  unsafe { sender.Cursor(&mut cursor) }.ok().map(|_| cursor)
}

/// The counterpart of `InnerWebView::dettach_parent_subclass`.
pub unsafe fn detach_host_subclass(parent: HWND) {
  SendMessageW(parent, COMPOSITION_DESTROY_MESSAGE, None, None);
  let _ = RemoveWindowSubclass(
    parent,
    Some(host_subclass_proc),
    COMPOSITION_SUBCLASS_ID as usize,
  );
}

unsafe extern "system" fn host_subclass_proc(
  hwnd: HWND,
  msg: u32,
  wparam: WPARAM,
  lparam: LPARAM,
  _uidsubclass: usize,
  dwrefdata: usize,
) -> LRESULT {
  let host_ptr = dwrefdata as *mut CompositionHost;
  if host_ptr.is_null() {
    return DefSubclassProc(hwnd, msg, wparam, lparam);
  }
  let host = &*host_ptr;

  match msg {
    WM_SIZE => {
      if wparam.0 != SIZE_MINIMIZED as usize {
        let mut client_rect = RECT::default();
        if GetClientRect(hwnd, &mut client_rect).is_ok() {
          let _ = host.controller.SetBounds(RECT {
            left: 0,
            top: 0,
            right: client_rect.right - client_rect.left,
            bottom: client_rect.bottom - client_rect.top,
          });
        }
      }
    }

    WM_SETFOCUS | WM_ENTERSIZEMOVE => {
      let _ = host
        .controller
        .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
    }

    msg if msg == WM_MOVE || msg == WM_MOVING => {
      let _ = host.controller.NotifyParentWindowPositionChanged();
    }

    WM_SETCURSOR => {
      // Only own the cursor inside the client area (which the webview covers
      // in composition mode); non-client hits keep the host's resize arrows.
      if (lparam.0 & 0xFFFF) as u32 == HTCLIENT {
        let _ = SetCursor(Some(host.cursor.get()));
        return LRESULT(1);
      }
    }

    WM_MOUSEMOVE | WM_MOUSELEAVE | WM_MOUSEWHEEL | WM_MOUSEHWHEEL | WM_LBUTTONDOWN
    | WM_LBUTTONUP | WM_LBUTTONDBLCLK | WM_RBUTTONDOWN | WM_RBUTTONUP | WM_RBUTTONDBLCLK
    | WM_MBUTTONDOWN | WM_MBUTTONUP | WM_MBUTTONDBLCLK | WM_XBUTTONDOWN | WM_XBUTTONUP
    | WM_XBUTTONDBLCLK => {
      forward_mouse_message(host, hwnd, msg, wparam, lparam);
      // Deliberately fall through to DefSubclassProc: the host's own window
      // proc still sees the message, so host-side window machinery (its event
      // stream, drag/resize bookkeeping) keeps working. The webview and the
      // host both observing input is by design in this hosting model — the
      // host owns the input queue.
    }

    WM_POINTERDOWN | WM_POINTERUPDATE | WM_POINTERUP => {
      forward_pointer_message(host, hwnd, msg, wparam);
    }

    msg if msg == WM_DESTROY || msg == COMPOSITION_DESTROY_MESSAGE => {
      if !(dwrefdata as *mut ()).is_null() {
        let host = Box::from_raw(host_ptr);
        let _ = host
          .comp_controller
          .remove_CursorChanged(host.cursor_changed_token);
        drop(host);

        // Null out `dwrefdata` so a second destroy cannot double-free (same
        // guard as the windowed parent subclass).
        let _ = SetWindowSubclass(
          hwnd,
          Some(host_subclass_proc),
          COMPOSITION_SUBCLASS_ID as usize,
          std::ptr::null::<()>() as usize,
        );
      }
    }

    _ => (),
  }

  DefSubclassProc(hwnd, msg, wparam, lparam)
}

#[inline]
fn get_x_lparam(lparam: LPARAM) -> i32 {
  (lparam.0 & 0xFFFF) as u16 as i16 as i32
}

#[inline]
fn get_y_lparam(lparam: LPARAM) -> i32 {
  ((lparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32
}

/// Translates one `WM_MOUSE*` message into `SendMouseInput`.
///
/// `COREWEBVIEW2_MOUSE_EVENT_KIND` values are defined to equal the `WM_*`
/// message codes, so the message id passes through as the event kind. Wheel
/// messages carry screen coordinates (translated to client); all others are
/// already client-relative. Mouse capture is held across button drags so the
/// webview keeps receiving moves outside the window, and `TME_LEAVE` tracking
/// generates the `WM_MOUSELEAVE` the webview needs to clear hover state.
fn forward_mouse_message(
  host: &CompositionHost,
  hwnd: HWND,
  msg: u32,
  wparam: WPARAM,
  lparam: LPARAM,
) {
  let mut point = windows::Win32::Foundation::POINT {
    x: get_x_lparam(lparam),
    y: get_y_lparam(lparam),
  };
  let mut mouse_data = 0u32;
  // LOWORD(wParam) carries the MK_* modifier state for every forwarded mouse
  // message except WM_MOUSELEAVE (where wParam is unused and stays 0).
  let mut virtual_keys = (wparam.0 & 0xFFFF) as u32;

  match msg {
    WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
      // HIWORD(wParam): signed wheel delta. Wheel coordinates are screen-based.
      mouse_data = ((wparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32 as u32;
      unsafe {
        let _ = ScreenToClient(hwnd, &mut point);
      }
    }
    WM_XBUTTONDOWN | WM_XBUTTONUP | WM_XBUTTONDBLCLK => {
      // HIWORD(wParam): which X button (XBUTTON1/XBUTTON2).
      mouse_data = ((wparam.0 >> 16) & 0xFFFF) as u32;
    }
    WM_MOUSELEAVE => {
      host.mouse_tracking.set(false);
      point = windows::Win32::Foundation::POINT { x: 0, y: 0 };
      virtual_keys = 0;
    }
    WM_MOUSEMOVE => {
      if !host.mouse_tracking.get() {
        let mut tme = TRACKMOUSEEVENT {
          cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
          dwFlags: TME_LEAVE,
          hwndTrack: hwnd,
          dwHoverTime: 0,
        };
        if unsafe { TrackMouseEvent(&mut tme) }.is_ok() {
          host.mouse_tracking.set(true);
        }
      }
    }
    _ => {}
  }

  match msg {
    WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN => unsafe {
      SetCapture(hwnd);
      // The host HWND keeps Win32 focus when its client area is clicked, so
      // hand keyboard/IME focus to the webview's hidden input window here —
      // this is what makes typing and composition reach the page.
      let _ = host
        .controller
        .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
    },
    WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP | WM_XBUTTONUP => unsafe {
      let _ = ReleaseCapture();
    },
    _ => {}
  }

  let _ = unsafe {
    host.comp_controller.SendMouseInput(
      COREWEBVIEW2_MOUSE_EVENT_KIND(msg as i32),
      COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS(virtual_keys as i32),
      mouse_data,
      point,
    )
  };
}

/// Forwards touch/pen `WM_POINTER*` messages through `SendPointerInput`.
///
/// Mouse-sourced pointer messages are ignored (the `WM_MOUSE*` path above
/// already covers the mouse; `EnableMouseInPointer` is never called). The
/// himetric location/contact fields are left zeroed — WebView2 consumes the
/// pixel fields for hit-testing.
fn forward_pointer_message(host: &CompositionHost, hwnd: HWND, msg: u32, wparam: WPARAM) {
  let Some(env3) = &host.env3 else {
    return;
  };

  let pointer_id = (wparam.0 & 0xFFFF) as u32;
  let mut pointer_type = POINTER_INPUT_TYPE::default();
  if unsafe { GetPointerType(pointer_id, &mut pointer_type) }.is_err() {
    return;
  }
  if pointer_type != PT_TOUCH && pointer_type != PT_PEN {
    return;
  }

  let filled: Result<ICoreWebView2PointerInfo> = (|| {
    let info =
      unsafe { env3.CreateCoreWebView2PointerInfo() }.map_err(windows::core::Error::from)?;

    if pointer_type == PT_TOUCH {
      let mut touch = windows::Win32::UI::Input::Pointer::POINTER_TOUCH_INFO::default();
      unsafe { GetPointerTouchInfo(pointer_id, &mut touch)? };
      unsafe {
        fill_common_pointer_info(&info, hwnd, &touch.pointerInfo)?;
        info.SetTouchFlags(touch.touchFlags)?;
        info.SetTouchMask(touch.touchMask)?;
        info.SetTouchOrientation(touch.orientation)?;
        info.SetTouchPressure(touch.pressure)?;
        let mut contact = touch.rcContact;
        client_rect_from_screen(hwnd, &mut contact);
        info.SetTouchContact(contact)?;
      }
    } else {
      let mut pen = windows::Win32::UI::Input::Pointer::POINTER_PEN_INFO::default();
      unsafe { GetPointerPenInfo(pointer_id, &mut pen)? };
      unsafe {
        fill_common_pointer_info(&info, hwnd, &pen.pointerInfo)?;
        info.SetPenFlags(pen.penFlags)?;
        info.SetPenMask(pen.penMask)?;
        info.SetPenPressure(pen.pressure)?;
        info.SetPenRotation(pen.rotation)?;
        info.SetPenTiltX(pen.tiltX)?;
        info.SetPenTiltY(pen.tiltY)?;
      }
    }

    Ok(info)
  })();

  if let Ok(info) = filled {
    let _ = unsafe {
      host
        .comp_controller
        .SendPointerInput(COREWEBVIEW2_POINTER_EVENT_KIND(msg as i32), &info)
    };
  }
}

/// Copies the shared `POINTER_INFO` fields into a `ICoreWebView2PointerInfo`,
/// translating the screen-based pixel locations into host client coordinates.
unsafe fn fill_common_pointer_info(
  info: &ICoreWebView2PointerInfo,
  hwnd: HWND,
  src: &POINTER_INFO,
) -> Result<()> {
  info.SetPointerKind(src.pointerType.0 as u32)?;
  info.SetPointerId(src.pointerId)?;
  info.SetFrameId(src.frameId)?;
  info.SetPointerFlags(src.pointerFlags.0 as u32)?;
  info.SetTime(src.dwTime)?;
  info.SetHistoryCount(src.historyCount)?;
  info.SetInputData(src.InputData)?;
  info.SetKeyStates(src.dwKeyStates)?;
  info.SetPerformanceCount(src.PerformanceCount)?;
  info.SetButtonChangeKind(src.ButtonChangeType.0)?;

  let mut pixel = src.ptPixelLocation;
  let _ = ScreenToClient(hwnd, &mut pixel);
  info.SetPixelLocation(pixel)?;
  let mut pixel_raw = src.ptPixelLocationRaw;
  let _ = ScreenToClient(hwnd, &mut pixel_raw);
  info.SetPixelLocationRaw(pixel_raw)?;

  Ok(())
}

/// Translates a screen-space RECT into client space in place.
fn client_rect_from_screen(hwnd: HWND, rect: &mut RECT) {
  let mut top_left = windows::Win32::Foundation::POINT {
    x: rect.left,
    y: rect.top,
  };
  let mut bottom_right = windows::Win32::Foundation::POINT {
    x: rect.right,
    y: rect.bottom,
  };
  unsafe {
    let _ = ScreenToClient(hwnd, &mut top_left);
    let _ = ScreenToClient(hwnd, &mut bottom_right);
  }
  *rect = RECT {
    left: top_left.x,
    top: top_left.y,
    right: bottom_right.x,
    bottom: bottom_right.y,
  };
}
