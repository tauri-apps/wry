use wry::{webview::WebViewBuilder, Result};

use cairo::*;
use gtk::*;

fn main() -> Result<()> {
  gtk::init()?;
  let window = Window::new(WindowType::Toplevel);

  window.show_all();
  // TODO add to webview

  gtk::main();
  Ok(())
}
