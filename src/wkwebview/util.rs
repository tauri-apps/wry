// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use objc2_foundation::NSProcessInfo;
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

pub fn operating_system_version() -> (isize, isize, isize) {
  let process_info = NSProcessInfo::processInfo();
  let version = process_info.operatingSystemVersion();
  (
    version.majorVersion,
    version.minorVersion,
    version.patchVersion,
  )
}
