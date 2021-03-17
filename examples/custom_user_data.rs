use std::path::PathBuf;
use wry::{Application, Attributes, Result};

fn main() -> Result<()> {
  let mut app = Application::new()?;

  let test_path = PathBuf::from(env!("OUT_DIR"));

  println!("Webview storage path: {:#?}", test_path);

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
