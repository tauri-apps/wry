// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use cocoa::{base::id, foundation::NSOperatingSystemVersion};

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
