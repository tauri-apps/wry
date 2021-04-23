// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#[cfg(target_os = "linux")]
pub use winit::dpi;
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
pub mod window;

#[cfg(not(target_os = "linux"))]
pub use original::*;
#[cfg(not(target_os = "linux"))]
mod original {
    pub use winit::dpi;
    pub use winit::error;
    pub use winit::event;
    pub use winit::event_loop;
    pub use winit::monitor;
    pub use winit::window;

    #[cfg(target_os = "macos")]
    pub use winit::platform;
    #[cfg(target_os = "windows")]
    pub mod platform {
        pub use winit::platform::run_return;

        pub mod windows {
            pub use winit::platform::windows::DeviceIdExtWindows;
            pub use winit::platform::windows::EventLoopExtWindows;
            pub use winit::platform::windows::IconExtWindows;
            pub use winit::platform::windows::MonitorHandleExtWindows;
            pub use winit::platform::windows::WindowBuilderExtWindows;
            use winit::platform::windows::WindowExtWindows as WindowExtWindows_;

            use winit::window::{Icon, Theme, Window};

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
                fn skip_taskbar(&self, taskbar_icon: Option<Icon>);

                /// Returns the current window theme.
                fn theme(&self) -> Theme;
            }

            impl WindowExtWindows for Window {
                #[inline]
                fn hinstance(&self) -> *mut libc::c_void {
                    self.hinstance()
                }

                #[inline]
                fn hwnd(&self) -> *mut libc::c_void {
                    self.hwnd()
                }

                #[inline]
                fn set_taskbar_icon(&self, taskbar_icon: Option<Icon>) {
                    self.set_taskbar_icon(taskbar_icon)
                }

                #[inline]
                fn skip_taskbar(&self, taskbar_icon: Option<Icon>) {
                    todo!()
                }

                #[inline]
                fn theme(&self) -> Theme {
                    self.theme()
                }
            }
        }
    }
}