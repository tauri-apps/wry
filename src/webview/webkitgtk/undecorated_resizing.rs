use crate::application::platform::unix::*;
use gtk::{
  gdk::{Cursor, WindowEdge},
  prelude::*,
};
use webkit2gtk::WebView;

pub fn setup(webview: &WebView) {
  webview.connect_motion_notify_event(|webview, event| {
    // This one should be GtkWindow
    if let Some(widget) = webview.parent() {
      // This one should be GtkWindow
      if let Some(window) = widget.parent() {
        // Safe to unwrap unless this is not from tao
        let window: gtk::Window = window.downcast().unwrap();
        if !window.is_decorated() && window.is_resizable() {
          if let Some(window) = window.window() {
            let (cx, cy) = event.root();
            let edge = hit_test(&window, cx, cy);
            // FIXME: calling `window.begin_resize_drag` seems to revert the cursor back to normal style
            window.set_cursor(
              Cursor::from_name(
                &window.display(),
                match edge {
                  WindowEdge::North => "n-resize",
                  WindowEdge::South => "s-resize",
                  WindowEdge::East => "e-resize",
                  WindowEdge::West => "w-resize",
                  WindowEdge::NorthWest => "nw-resize",
                  WindowEdge::NorthEast => "ne-resize",
                  WindowEdge::SouthEast => "se-resize",
                  WindowEdge::SouthWest => "sw-resize",
                  _ => "default",
                },
              )
              .as_ref(),
            );
          }
        }
      }
    }
    gtk::glib::Propagation::Proceed
  });
  webview.connect_button_press_event(move |webview, event| {
    if event.button() == 1 {
      let (cx, cy) = event.root();
      // This one should be GtkBox
      if let Some(widget) = webview.parent() {
        // This one should be GtkWindow
        if let Some(window) = widget.parent() {
          // Safe to unwrap unless this is not from tao
          let window: gtk::Window = window.downcast().unwrap();
          if !window.is_decorated() && window.is_resizable() {
            if let Some(window) = window.window() {
              // Safe to unwrap since it's a valid GtkWindow
              let result = hit_test(&window, cx, cy);

              // we ignore the `__Unknown` variant so the webview receives the click correctly if it is not on the edges.
              match result {
                WindowEdge::__Unknown(_) => (),
                _ => window.begin_resize_drag(result, 1, cx as i32, cy as i32, event.time()),
              }
            }
          }
        }
      }
    }
    gtk::glib::Propagation::Proceed
  });
  webview.connect_touch_event(|webview, event| {
    // This one should be GtkBox
    if let Some(widget) = webview.parent() {
      // This one should be GtkWindow
      if let Some(window) = widget.parent() {
        // Safe to unwrap unless this is not from tao
        let window: gtk::Window = window.downcast().unwrap();
        if !window.is_decorated() && window.is_resizable() && !window.is_maximized() {
          if let Some(window) = window.window() {
            if let Some((cx, cy)) = event.root_coords() {
              if let Some(device) = event.device() {
                let result = hit_test(&window, cx, cy);

                // we ignore the `__Unknown` variant so the window receives the click correctly if it is not on the edges.
                match result {
                  WindowEdge::__Unknown(_) => (),
                  _ => window.begin_resize_drag_for_device(
                    result,
                    &device,
                    0,
                    cx as i32,
                    cy as i32,
                    event.time(),
                  ),
                }
              }
            }
          }
        }
      }
    }
    gtk::glib::Propagation::Proceed
  });
}
