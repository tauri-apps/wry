// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Re-exported winit API with extended features.
//!
//! This module re-export [winit] APIs on macOS and Windows. And since wry uses Gtk to create
//! WebView. This module is a re-implementation of winit APIs for Gtk on Linux. It also extends
//! more methods to some platform specific traits, so we recommended to use this module directly instead of
//! importing another winit dependency.
//!
//! To learn more about how to use winit, please see [its documentation](https://crates.io/crates/winit).
//!
//! # Warning
//! At the time this crate being published, there are still many features missing on Linux. Because
//! we want to cover most use cases in Tauri first. If you find there's a function you need but is
//! missing. Feel free to open an issue or PR.
//!
//! [winit]: https://crates.io/crates/winit
#[cfg(target_os = "linux")]
pub mod dpi;
#[cfg(target_os = "linux")]
pub mod error;
#[cfg(target_os = "linux")]
pub mod event;
#[cfg(target_os = "linux")]
pub mod event_loop;
#[cfg(target_os = "linux")]
mod icon;
#[cfg(target_os = "linux")]
pub mod monitor;
#[cfg(target_os = "linux")]
pub mod platform;
#[cfg(target_os = "linux")]
pub mod window;

#[cfg(not(target_os = "linux"))]
pub use original::*;
#[cfg(not(target_os = "linux"))]
mod original {
  pub use winit::{dpi, error, event, event_loop, monitor, window};

  #[cfg(target_os = "macos")]
  pub use winit::platform;
  #[cfg(target_os = "windows")]
  pub mod platform {
    pub use winit::platform::run_return;

    pub mod windows {
      use winapi::Interface;
      use winit::platform::windows::WindowExtWindows as WindowExtWindows_;
      pub use winit::platform::windows::{
        DeviceIdExtWindows, EventLoopExtWindows, IconExtWindows, MonitorHandleExtWindows,
        WindowBuilderExtWindows,
      };

      #[cfg(feature = "winrt")]
      use windows_webview2::Windows::Win32::{Shell as shell, WindowsAndMessaging::HWND};
      use winit::window::{Icon, Theme, Window};
      #[cfg(feature = "win32")]
      use {
        std::ptr,
        winapi::{
          shared::windef::HWND,
          um::{
            combaseapi::{CoCreateInstance, CLSCTX_SERVER},
            shobjidl_core::{CLSID_TaskbarList, ITaskbarList},
          },
        },
      };

      /// Additional methods on `Window` that are specific to Windows.
      pub trait WindowExtWindows {
        /// Returns the HINSTANCE of the window
        fn hinstance(&self) -> *mut libc::c_void;
        /// Returns the native handle that is used by this window.
        ///
        /// The pointer will become invalid when the native window was destroyed.
        fn hwnd(&self) -> *mut libc::c_void;

        /// This sets `ICON_BIG`. A good ceiling here is 256x256.
        fn set_taskbar_icon(&self, taskbar_icon: Option<Icon>);

        /// This removes taskbar icon of the application.
        fn skip_taskbar(&self);

        /// Returns the current window theme.
        fn theme(&self) -> Theme;
      }

      impl WindowExtWindows for Window {
        #[inline]
        fn hinstance(&self) -> *mut libc::c_void {
          WindowExtWindows_::hinstance(self)
        }

        #[inline]
        fn hwnd(&self) -> *mut libc::c_void {
          WindowExtWindows_::hwnd(self)
        }

        #[inline]
        fn set_taskbar_icon(&self, taskbar_icon: Option<Icon>) {
          WindowExtWindows_::set_taskbar_icon(self, taskbar_icon)
        }

        #[inline]
        fn skip_taskbar(&self) {
          #[cfg(feature = "winrt")]
          unsafe {
            if let Ok(taskbar_list) =
              windows::create_instance::<shell::ITaskbarList>(&shell::TaskbarList)
            {
              let _ = taskbar_list.DeleteTab(HWND(WindowExtWindows_::hwnd(self) as _));
            }
          }
          #[cfg(feature = "win32")]
          unsafe {
            let mut taskbar_list: *mut ITaskbarList = std::mem::zeroed();
            CoCreateInstance(
              &CLSID_TaskbarList,
              ptr::null_mut(),
              CLSCTX_SERVER,
              &ITaskbarList::uuidof(),
              &mut taskbar_list as *mut _ as *mut _,
            );
            (*taskbar_list).DeleteTab(WindowExtWindows_::hwnd(self) as HWND);
            (*taskbar_list).Release();
          }
        }

        #[inline]
        fn theme(&self) -> Theme {
          WindowExtWindows_::theme(self)
        }
      }
    }
  }
}
