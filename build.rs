// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#[cfg(target_os = "linux")]
fn main() {}

#[cfg(target_os = "macos")]
fn main() {
  println!("cargo:rustc-link-lib=framework=WebKit");
}

#[cfg(target_os = "windows")]
fn main() {}
