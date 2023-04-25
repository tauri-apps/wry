use super::NSString;
use cocoa::{
  appkit::{NSEvent, NSEventModifierFlags, NSEventType, NSView},
  base::{id, nil},
};
use objc::{
  declare::ClassDecl,
  runtime::{Object, Sel},
};
use std::{ffi::c_void, ptr::null};

pub unsafe fn setup(decl: &mut ClassDecl) {
  decl.add_method(
    sel!(otherMouseDown:),
    other_mouse_down as extern "C" fn(&mut Object, Sel, id),
  );
  decl.add_method(
    sel!(otherMouseUp:),
    other_mouse_up as extern "C" fn(&mut Object, Sel, id),
  );
}

extern "C" fn other_mouse_down(this: &mut Object, _sel: Sel, event: id) {
  unsafe {
    if event.eventType() == NSEventType::NSOtherMouseDown {
      let button_number = event.buttonNumber();
      match button_number {
        // back button
        3 => {
          let js = create_js_mouse_event(this, event, true, true);
          let _: id = msg_send![this, evaluateJavaScript:NSString::new(&js) completionHandler:null::<*const c_void>()];
          return;
        }
        // forward button
        4 => {
          let js = create_js_mouse_event(this, event, true, false);
          let _: id = msg_send![this, evaluateJavaScript:NSString::new(&js) completionHandler:null::<*const c_void>()];
          return;
        }
        _ => {}
      }
    }

    let _: () = msg_send![this, mouseDown: event];
  }
}
extern "C" fn other_mouse_up(this: &mut Object, _sel: Sel, event: id) {
  unsafe {
    if event.eventType() == NSEventType::NSOtherMouseUp {
      let button_number = event.buttonNumber();
      match button_number {
        // back button
        3 => {
          let js = create_js_mouse_event(this, event, false, true);
          let _: id = msg_send![this, evaluateJavaScript:NSString::new(&js) completionHandler:null::<*const c_void>()];
          return;
        }
        // forward button
        4 => {
          let js = create_js_mouse_event(this, event, false, false);
          let _: id = msg_send![this, evaluateJavaScript:NSString::new(&js) completionHandler:null::<*const c_void>()];
          return;
        }
        _ => {}
      }
    }

    let _: () = msg_send![this, mouseUp: event];
  }
}

unsafe fn create_js_mouse_event(view: id, event: id, down: bool, back_button: bool) -> String {
  let event_name = if down { "mousedown" } else { "mouseup" };
  // js equivalent https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent/button
  let button = if back_button { 3 } else { 4 };
  let mods_flags = event.modifierFlags();
  let window_point = event.locationInWindow();
  let view_point = view.convertPoint_fromView_(window_point, nil);
  let x = view_point.x as u32;
  let y = view_point.y as u32;
  // js equivalent https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent/buttons
  let buttons = NSEvent::pressedMouseButtons(event);

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
    detail = event.clickCount(),
    ctrl_key = mods_flags.contains(NSEventModifierFlags::NSControlKeyMask),
    alt_key = mods_flags.contains(NSEventModifierFlags::NSAlternateKeyMask),
    shift_key = mods_flags.contains(NSEventModifierFlags::NSShiftKeyMask),
    meta_key = mods_flags.contains(NSEventModifierFlags::NSCommandKeyMask),
    button = button,
    buttons = buttons,
  )
}
