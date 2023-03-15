use gdk::{EventButton, ModifierType};
use gtk::prelude::*;
use webkit2gtk::{WebView, WebViewExt};

pub fn setup(webview: &WebView) {
  webview.connect_button_press_event(move |webview, event| {
    let mut inhibit = false;
    match event.button() {
      // back button
      8 => {
        inhibit = true;
        webview.run_javascript(
          &create_js_mouse_event(event, true),
          None::<&gio::Cancellable>,
          |_| {},
        );
      }
      // forward button
      9 => {
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
      // back button
      8 => {
        inhibit = true;
        webview.run_javascript(
          &create_js_mouse_event(event, false),
          None::<&gio::Cancellable>,
          |_| {},
        );
      }
      // forward button
      9 => {
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
  // js equivalent https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent/button
  let button = if event.button() == 8 { 3 } else { 4 };
  let (x, y) = event.position();
  let (x, y) = (x as i32, y as i32);
  let modifers_state = event.state();
  let mut buttons = 0;
  if modifers_state.contains(ModifierType::BUTTON1_MASK) {
    buttons += 1;
  }
  // right button
  if modifers_state.contains(ModifierType::BUTTON2_MASK) {
    buttons += 2;
  }
  // middle button
  if modifers_state.contains(ModifierType::BUTTON3_MASK) {
    buttons += 4;
  }
  // back button
  if modifers_state.contains(ModifierType::BUTTON4_MASK) {
    buttons += 9;
  }
  // forward button
  if modifers_state.contains(ModifierType::BUTTON5_MASK) {
    buttons += 16;
  }
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
    meta_key = modifers_state.contains(ModifierType::SUPER_MASK),
    button = button,
    buttons = buttons,
  )
}
