// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Types useful for interacting with a user's monitors.
//!
//! If you want to get basic information about a monitor, you can use the [`MonitorHandle`][monitor_handle]
//! type. This is retrieved from one of the following methods, which return an iterator of
//! [`MonitorHandle`][monitor_handle]:
//! - [`EventLoopWindowTarget::available_monitors`][loop_get]
//! - [`Window::available_monitors`][window_get].
//!
//! [monitor_handle]: crate::monitor::MonitorHandle
//! [loop_get]: crate::event_loop::EventLoopWindowTarget::available_monitors
//! [window_get]: crate::window::Window::available_monitors

/// Describes a fullscreen video mode of a monitor.
///
/// Can be acquired with:
/// - [`MonitorHandle::video_modes`][monitor_get].
///
/// [monitor_get]: crate::monitor::MonitorHandle::video_modes
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VideoMode {}

/// Handle to a monitor.
///
/// Allows you to retrieve information about a given monitor and can be used in [`Window`] creation.
///
/// [`Window`]: crate::window::Window
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MonitorHandle {}

// TODO impl methods
