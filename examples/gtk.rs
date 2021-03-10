

#[cfg(not(target_os = "linux"))]
fn main() {}

#[cfg(target_os = "linux")]
fn main() -> wry::Result<()> {
  use wry::webview::WebViewBuilder;
  use cairo::*;
  use gtk::*;

  gtk::init()?;
  let window = Window::new(WindowType::Toplevel);

  window.show_all();
  // TODO add to webview

  gtk::main();
  Ok(())
}
