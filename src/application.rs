// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Re-exported Tao APIs
//!
//! This module re-export [tao] APIs for user to create application windows. To learn more about
//! how to use tao, please see [its documentation](https://crates.io/crates/tao).
//!
//! [tao]: https://crates.io/crates/tao

#[cfg(not(target_os = "android"))]
pub use tao::*;

// TODO Implement actual Window library of Android
#[cfg(target_os = "android")]
pub use tao::{dpi, error};

#[cfg(target_os = "android")]
pub mod window {
  use tao::dpi::PhysicalSize;
  pub use tao::window::BadIcon;

  pub struct Window;

  impl Window {
    pub fn new() -> Self {
      Self
    }

    pub fn inner_size(&self) -> PhysicalSize<u32> {
      todo!()
    }
  }
}
