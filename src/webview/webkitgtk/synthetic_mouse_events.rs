use gdk::{EventButton, ModifierType};
use gtk::prelude::*;
use webkit2gtk::{WebView, WebViewExt};

/// Mouse Back button
const MOUSE_BUTTON4: u32 = 8;
/// Mouse Forward button
const MOUSE_BUTTON5: u32 = 9;

pub fn setup(webview: &WebView) {
  webview.connect_button_press_event(move |webview, event| {
    let mut inhibit = false;
    match event.button() {
      MOUSE_BUTTON4 => {
        inhibit = true;
        webview.run_javascript(
          &create_js_mouse_event(event, true),
          None::<&gio::Cancellable>,
          |_| {},
        );
      }
      MOUSE_BUTTON5 => {
        inhibit = true;
        webview.run_javascript(
          &create_js_mouse_event(event, true),
          None::<&gio::Cancellable>,
          |_| {},
        );
      }
      _ => {}
    }

    Inhibit(inhibit)
  });

  webview.connect_button_release_event(move |webview, event| {
    let mut inhibit = false;
    match event.button() {
      MOUSE_BUTTON4 => {
        inhibit = true;
        webview.run_javascript(
          &create_js_mouse_event(event, false),
          None::<&gio::Cancellable>,
          |_| {},
        );
      }
      MOUSE_BUTTON5 => {
        inhibit = true;
        webview.run_javascript(
          &create_js_mouse_event(event, false),
          None::<&gio::Cancellable>,
          |_| {},
        );
      }
      _ => {}
    }
    Inhibit(inhibit)
  });
}

fn create_js_mouse_event(event: &EventButton, pressed: bool) -> String {
  let event_name = if pressed { "mousedown" } else { "mouseup" };
  // js events equivalent for mouse back/forward buttons
  let (button, buttons) = if event.button() == MOUSE_BUTTON4 {
    (3, 8)
  } else {
    (4, 16)
  };
  let (x, y) = event.position();
  let (x, y) = (x as i32, y as i32);
  let modifers_state = event.state();
  format!(
    r#"(() => {{
        const el = document.elementFromPoint({x},{y});
        const ev = new MouseEvent('{event_name}', {{
          view: window,
          button: {button},
          buttons: {buttons},
          x: {x},
          y: {y},
          bubbles: true,
          detail: {detail},
          cancelBubble: false,
          cancelable: true,
          clientX: {x},
          clientY: {y},
          composed: true,
          layerX: {x},
          layerY: {y},
          pageX: {x},
          pageY: {y},
          screenX: window.screenX + {x},
          screenY: window.screenY + {y},
          ctrlKey: {ctrl_key},
          metaKey: {meta_key},
          shiftKey: {shift_key},
          altKey: {alt_key},
        }});
        el.dispatchEvent(ev)
        if (!ev.defaultPrevented && "{event_name}" === "mouseup") {{
          if (ev.button === 3) {{
            window.history.back();
          }}
          if (ev.button === 4) {{
            window.history.forward();
          }}
        }}
      }})()"#,
    event_name = event_name,
    x = x,
    y = y,
    detail = event.click_count().unwrap_or(1),
    ctrl_key = modifers_state.contains(ModifierType::CONTROL_MASK),
    alt_key = modifers_state.contains(ModifierType::MOD1_MASK),
    shift_key = modifers_state.contains(ModifierType::SHIFT_MASK),
    meta_key = modifers_state.contains(ModifierType::META_MASK),
    button = button,
    buttons = buttons,
  )
}
