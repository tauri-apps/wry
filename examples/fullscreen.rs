// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use wry::{Application, Attributes, Result};

fn main() -> Result<()> {
  let mut app = Application::new()?;

  let attributes = Attributes {
    url: Some("https://www.wirple.com/".to_string()),
    title: String::from("3D Render Test ^ ^"),
    fullscreen: true,
    transparent: true,
    ..Default::default()
  };

  app.add_window(attributes)?;
  app.run();
  Ok(())
}

// Test Result:
// CPU: i7 9750H || GPU: Intel(R) UHD Graphics 630
// Linux kernel 5.8.18-18-ibryza-standard-xin
// Mesa Mesa 20.2.6
// ================================================
// Canvas score - Test 1: 542 - Test 2: 368
// WebGL score - Test 1: 1390 - Test 2: 1342
// Total score: 3642
