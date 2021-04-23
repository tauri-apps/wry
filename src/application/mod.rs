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
    pub use winit::*;
}