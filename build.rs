// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

fn main() {
  let is_macos = std::env::var("TARGET")
    .map(|t| t.ends_with("-darwin"))
    .unwrap_or_default();
  if is_macos {
    println!("cargo:rustc-link-lib=framework=WebKit");
  }
}
