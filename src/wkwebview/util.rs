// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use cocoa::{base::id, foundation::NSOperatingSystemVersion};
use std::sync::atomic::{AtomicU32, Ordering};

pub struct Counter(AtomicU32);

impl Counter {
  #[allow(unused)]
  pub const fn new() -> Self {
    Self(AtomicU32::new(1))
  }

  pub fn next(&self) -> u32 {
    self.0.fetch_add(1, Ordering::Relaxed)
  }
}

pub fn operating_system_version() -> (u64, u64, u64) {
  unsafe {
    let process_info: id = msg_send![class!(NSProcessInfo), processInfo];
    let version: NSOperatingSystemVersion = msg_send![process_info, operatingSystemVersion];
    (
      version.majorVersion,
      version.minorVersion,
      version.patchVersion,
    )
  }
}
