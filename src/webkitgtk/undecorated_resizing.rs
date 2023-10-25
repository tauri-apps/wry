use gtk::gdk::{self, Cursor, WindowEdge};
use gtk::{glib, prelude::*};
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
    glib::Propagation::Proceed
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
    glib::Propagation::Proceed
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
    glib::Propagation::Proceed
  });
}

pub fn hit_test(window: &gdk::Window, cx: f64, cy: f64) -> WindowEdge {
  let (left, top) = window.position();
  let (w, h) = (window.width(), window.height());
  let (right, bottom) = (left + w, top + h);
  let (cx, cy) = (cx as i32, cy as i32);

  const LEFT: i32 = 0b0001;
  const RIGHT: i32 = 0b0010;
  const TOP: i32 = 0b0100;
  const BOTTOM: i32 = 0b1000;
  const TOPLEFT: i32 = TOP | LEFT;
  const TOPRIGHT: i32 = TOP | RIGHT;
  const BOTTOMLEFT: i32 = BOTTOM | LEFT;
  const BOTTOMRIGHT: i32 = BOTTOM | RIGHT;

  let inset = 5 * window.scale_factor();
  #[rustfmt::skip]
  let result =
      (LEFT * (if cx < (left + inset) { 1 } else { 0 }))
    | (RIGHT * (if cx >= (right - inset) { 1 } else { 0 }))
    | (TOP * (if cy < (top + inset) { 1 } else { 0 }))
    | (BOTTOM * (if cy >= (bottom - inset) { 1 } else { 0 }));

  match result {
    LEFT => WindowEdge::West,
    TOP => WindowEdge::North,
    RIGHT => WindowEdge::East,
    BOTTOM => WindowEdge::South,
    TOPLEFT => WindowEdge::NorthWest,
    TOPRIGHT => WindowEdge::NorthEast,
    BOTTOMLEFT => WindowEdge::SouthWest,
    BOTTOMRIGHT => WindowEdge::SouthEast,
    // we return `WindowEdge::__Unknown` to be ignored later.
    // we must return 8 or bigger, otherwise it will be the same as one of the other 7 variants of `WindowEdge` enum.
    _ => WindowEdge::__Unknown(8),
  }
}
