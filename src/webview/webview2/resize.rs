#![allow(non_snake_case)]

use once_cell::sync::Lazy;
use windows::{
  core::HRESULT,
  Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{
      GetDC, GetDeviceCaps, MonitorFromWindow, HMONITOR, LOGPIXELSX, MONITOR_DEFAULTTONEAREST,
    },
    UI::{
      HiDpi::{MDT_EFFECTIVE_DPI, MONITOR_DPI_TYPE},
      Input::KeyboardAndMouse::ReleaseCapture,
      WindowsAndMessaging::{
        GetWindowRect, IsProcessDPIAware, PostMessageW, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT,
        HTCLIENT, HTLEFT, HTNOWHERE, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT,
      },
    },
  },
};

use super::get_function;

#[inline]
pub fn MAKELPARAM(x: i16, y: i16) -> LPARAM {
  LPARAM(((x as u16 as u32) | ((y as u16 as u32) << 16)) as usize as _)
}

#[inline]
pub fn begin_resize_drag(
  hwnd: isize,
  edge: isize,
  button: u32,
  x: i32,
  y: i32,
) -> windows::core::Result<()> {
  unsafe {
    let w_param = WPARAM(edge as _);
    let l_param = MAKELPARAM(x as i16, y as i16);

    ReleaseCapture()?;
    PostMessageW(HWND(hwnd), button, w_param, l_param)
  }
}

type GetDpiForWindow = unsafe extern "system" fn(hwnd: HWND) -> u32;
type GetDpiForMonitor = unsafe extern "system" fn(
  hmonitor: HMONITOR,
  dpi_type: MONITOR_DPI_TYPE,
  dpi_x: *mut u32,
  dpi_y: *mut u32,
) -> HRESULT;

static GET_DPI_FOR_WINDOW: Lazy<Option<GetDpiForWindow>> =
  Lazy::new(|| get_function!("user32.dll", GetDpiForWindow));
static GET_DPI_FOR_MONITOR: Lazy<Option<GetDpiForMonitor>> =
  Lazy::new(|| get_function!("shcore.dll", GetDpiForMonitor));

const BASE_DPI: u32 = 96;
fn dpi_to_scale_factor(dpi: u32) -> f64 {
  dpi as f64 / BASE_DPI as f64
}

unsafe fn hwnd_dpi(hwnd: HWND) -> u32 {
  let hdc = GetDC(hwnd);
  if hdc.is_invalid() {
    panic!("[tao] `GetDC` returned null!");
  }
  if let Some(GetDpiForWindow) = *GET_DPI_FOR_WINDOW {
    // We are on Windows 10 Anniversary Update (1607) or later.
    match GetDpiForWindow(hwnd) {
      0 => BASE_DPI, // 0 is returned if hwnd is invalid
      dpi => dpi as u32,
    }
  } else if let Some(GetDpiForMonitor) = *GET_DPI_FOR_MONITOR {
    // We are on Windows 8.1 or later.
    let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
    if monitor.is_invalid() {
      return BASE_DPI;
    }

    let mut dpi_x = 0;
    let mut dpi_y = 0;
    if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).is_ok() {
      dpi_x as u32
    } else {
      BASE_DPI
    }
  } else {
    // We are on Vista or later.
    if IsProcessDPIAware().as_bool() {
      // If the process is DPI aware, then scaling must be handled by the application using
      // this DPI value.
      GetDeviceCaps(hdc, LOGPIXELSX) as u32
    } else {
      // If the process is DPI unaware, then scaling is performed by the OS; we thus return
      // 96 (scale factor 1.0) to prevent the window from being re-scaled by both the
      // application and the WM.
      BASE_DPI
    }
  }
}

const BORDERLESS_RESIZE_INSET: i32 = 5;

pub fn hit_test(hwnd: isize, cx: i32, cy: i32) -> LRESULT {
  let hwnd = HWND(hwnd);
  let mut window_rect = RECT::default();
  unsafe {
    if GetWindowRect(hwnd, &mut window_rect).is_ok() {
      const CLIENT: isize = 0b0000;
      const LEFT: isize = 0b0001;
      const RIGHT: isize = 0b0010;
      const TOP: isize = 0b0100;
      const BOTTOM: isize = 0b1000;
      const TOPLEFT: isize = TOP | LEFT;
      const TOPRIGHT: isize = TOP | RIGHT;
      const BOTTOMLEFT: isize = BOTTOM | LEFT;
      const BOTTOMRIGHT: isize = BOTTOM | RIGHT;

      let RECT {
        left,
        right,
        bottom,
        top,
      } = window_rect;

      let dpi = hwnd_dpi(hwnd);
      let scale_factor = dpi_to_scale_factor(dpi);
      let inset = (BORDERLESS_RESIZE_INSET as f64 * scale_factor) as i32;

      #[rustfmt::skip]
      let result =
          (LEFT * (if cx < (left + inset) { 1 } else { 0 }))
        | (RIGHT * (if cx >= (right - inset) { 1 } else { 0 }))
        | (TOP * (if cy < (top + inset) { 1 } else { 0 }))
        | (BOTTOM * (if cy >= (bottom - inset) { 1 } else { 0 }));

      LRESULT(match result {
        CLIENT => HTCLIENT,
        LEFT => HTLEFT,
        RIGHT => HTRIGHT,
        TOP => HTTOP,
        BOTTOM => HTBOTTOM,
        TOPLEFT => HTTOPLEFT,
        TOPRIGHT => HTTOPRIGHT,
        BOTTOMLEFT => HTBOTTOMLEFT,
        BOTTOMRIGHT => HTBOTTOMRIGHT,
        _ => HTNOWHERE,
      } as _)
    } else {
      LRESULT(HTNOWHERE as _)
    }
  }
}
