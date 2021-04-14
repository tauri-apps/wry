// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{fs, path::PathBuf};
use wry::{Application, Attributes, Result};

fn main() -> Result<()> {
  let mut app = Application::new()?;

  // Use a sample directory at the root of the project
  let mut test_path = PathBuf::from("./target/webview_data");
  // The directory need to exist or the Webview will panic
  fs::create_dir_all(&test_path)?;
  // We need an absoulte path for the webview
  test_path = fs::canonicalize(&test_path)?;
  // The directory need to exist or the Webview will panic
  println!("Webview storage path: {:#?}", &test_path);

  let attributes = Attributes {
    url: Some("https://tauri.studio/".to_string()),
    title: String::from("Hello World!"),
    // Currently supported only on Windows
    user_data_path: Some(test_path),
    ..Default::default()
  };

  app.add_window(attributes)?;
  app.run();
  Ok(())
}
