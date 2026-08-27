use std::{cell::RefCell, rc::Rc};

use webkit6::prelude::*;
use webkit6::WebView;
use webkit6::{gdk, gio, gtk};

pub fn setup(webview: &WebView) {
  let bf_state = BackForwardState(Rc::new(RefCell::new(0)));

  let gesture = gtk::GestureClick::new();
  // Accept all mouse buttons (0 = any button)
  gesture.set_button(0);

  let bf_state_press = bf_state.clone();
  let webview_press = webview.clone();
  gesture.connect_pressed(move |gesture, n_press, x, y| {
    let button = gesture.current_button();
    let inhibit = match button {
      // back button
      8 => {
        bf_state_press.set(BACK);
        let state = gesture.current_event_state();
        webview_press.evaluate_javascript(
          &create_js_mouse_event(button, n_press, x, y, state, true, &bf_state_press),
          None,
          None,
          None::<&gio::Cancellable>,
          |_| {},
        );
        true
      }
      // forward button
      9 => {
        bf_state_press.set(FORWARD);
        let state = gesture.current_event_state();
        webview_press.evaluate_javascript(
          &create_js_mouse_event(button, n_press, x, y, state, true, &bf_state_press),
          None,
          None,
          None::<&gio::Cancellable>,
          |_| {},
        );
        true
      }
      _ => false,
    };

    if inhibit {
      gesture.set_state(gtk::EventSequenceState::Claimed);
    }
  });

  let bf_state_release = bf_state.clone();
  let webview_release = webview.clone();
  gesture.connect_released(move |gesture, n_press, x, y| {
    let button = gesture.current_button();
    let inhibit = match button {
      // back button
      8 => {
        bf_state_release.remove(BACK);
        let state = gesture.current_event_state();
        webview_release.evaluate_javascript(
          &create_js_mouse_event(button, n_press, x, y, state, false, &bf_state_release),
          None,
          None,
          None::<&gio::Cancellable>,
          |_| {},
        );
        true
      }
      // forward button
      9 => {
        bf_state_release.remove(FORWARD);
        let state = gesture.current_event_state();
        webview_release.evaluate_javascript(
          &create_js_mouse_event(button, n_press, x, y, state, false, &bf_state_release),
          None,
          None,
          None::<&gio::Cancellable>,
          |_| {},
        );
        true
      }
      _ => false,
    };

    if inhibit {
      gesture.set_state(gtk::EventSequenceState::Claimed);
    }
  });

  webview.add_controller(gesture);
}

fn create_js_mouse_event(
  button: u32,
  n_press: i32,
  x: f64,
  y: f64,
  modifier_state: gdk::ModifierType,
  pressed: bool,
  state: &BackForwardState,
) -> String {
  let event_name = if pressed { "mousedown" } else { "mouseup" };
  // js equivalent https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent/button
  let js_button = if button == 8 { 3 } else { 4 };
  let (x, y) = (x as i32, y as i32);
  let mut buttons = 0;
  // left button
  if modifier_state.contains(gdk::ModifierType::BUTTON1_MASK) {
    buttons += 1;
  }
  // right button
  if modifier_state.contains(gdk::ModifierType::BUTTON3_MASK) {
    buttons += 2;
  }
  // middle button
  if modifier_state.contains(gdk::ModifierType::BUTTON2_MASK) {
    buttons += 4;
  }
  // back button
  if state.has(BACK) {
    buttons += 8;
  }
  // forward button
  if state.has(FORWARD) {
    buttons += 16;
  }
  format!(
    r#"(() => {{
        const el = document.elementFromPoint({x},{y});
        const ev = new MouseEvent('{event_name}', {{
          view: window,
          button: {js_button},
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
    detail = n_press,
    ctrl_key = modifier_state.contains(gdk::ModifierType::CONTROL_MASK),
    alt_key = modifier_state.contains(gdk::ModifierType::ALT_MASK),
    shift_key = modifier_state.contains(gdk::ModifierType::SHIFT_MASK),
    meta_key = modifier_state.contains(gdk::ModifierType::SUPER_MASK),
    js_button = js_button,
    buttons = buttons,
  )
}

// Internal modifiers to track whether BACK/FORWARD buttons are pressed
const BACK: u8 = 0b01;
const FORWARD: u8 = 0b10;

/// A single u8 that stores whether [BACK] and [FORWARD] are pressed or not
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct BackForwardState(Rc<RefCell<u8>>);

impl BackForwardState {
  fn set(&self, button: u8) {
    *self.0.borrow_mut() |= button
  }

  fn remove(&self, button: u8) {
    *self.0.borrow_mut() &= !button
  }

  fn has(&self, button: u8) -> bool {
    let state = *self.0.borrow();
    state & !button != state
  }
}
