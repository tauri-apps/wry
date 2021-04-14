// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use wry::{Application, Attributes, Result};

static TEST_HTML: &str = r#"data:text/html,
Drop files onto the window and read the console!<br>
Dropping files onto the following form is also possible:<br><br>
<input type="file"/>
"#;

fn main() -> Result<()> {
  let mut app = Application::new()?;

  app.add_window_with_configs(
    Attributes {
      url: Some(TEST_HTML.to_string()),
      ..Default::default()
    },
    None,
    vec![],
    Some(Box::new(|_, data| {
      println!("Window 1: {:?}", data);
      false // Returning true will block the OS default behaviour.
    })),
  )?;
  app.run();
  Ok(())
}
