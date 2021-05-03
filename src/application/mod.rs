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

pub use tao::*;
