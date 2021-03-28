fn main() {
    windows::build!(
        Microsoft::Web::WebView2::Core::*,
        Windows::Foundation::*,
        Windows::Storage::Streams::*,
        Windows::Win32::Com::*,
        Windows::Win32::DisplayDevices::{
          POINT,
          POINTL,
          RECT,
          SIZE
        },
        Windows::Win32::Gdi::UpdateWindow,
        Windows::Win32::HiDpi::{
          PROCESS_DPI_AWARENESS,
          SetProcessDpiAwareness
        },
        Windows::Win32::KeyboardAndMouseInput::SetFocus,
        Windows::Win32::MenusAndResources::HMENU,
        Windows::Win32::Shell::{
          DragFinish,
          DragQueryFileW,
          HDROP,
          ITaskbarList,
          TaskbarList
        },
        Windows::Win32::SystemServices::{
          BOOL,
          CLIPBOARD_FORMATS,
          DRAGDROP_E_INVALIDHWND,
          DV_E_FORMATETC,
          GetCurrentThreadId,
          GetModuleHandleA,
          HINSTANCE,
          LRESULT,
          PWSTR,
          userHMETAFILEPICT,
          userHENHMETAFILE,
        },
        Windows::Win32::WindowsAndMessaging::*
    )
  }
  