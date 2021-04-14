// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use wry::{Application, Attributes, Result};

fn main() -> Result<()> {
  let mut app = Application::new()?;

  let attributes = Attributes {
    decorations: false,
    transparent: true,
    url: Some(
      r#"data:text/html,
            <!doctype html>
            <html>
              <body style="background-color:rgba(87,87,87,0.);">hello</body>
              <script>
                window.onload = function() {
                  document.body.innerText = `hello, ${navigator.userAgent}`;
                };
              </script>
            </html>"#
        .to_string(),
    ),
    ..Default::default()
  };

  app.add_window(attributes)?;
  app.run();
  Ok(())
}
