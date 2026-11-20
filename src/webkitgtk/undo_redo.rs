// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Restores the undo and redo shortcuts that WebKitGTK lacks on GTK 3.
//!
//! WebKitGTK's GTK 3 key handling has no bindings for undo and redo, so Ctrl+Z,
//! Ctrl+Shift+Z and Ctrl+Y reach the page without doing anything. This listener
//! runs the editing command of the key once the event has been dispatched, and
//! skips it when the page handled the event itself, so editors that implement
//! their own undo keep working.

pub(crate) const INIT_SCRIPT: &str = r#"
(function () {
  if (window.__WRY_UNDO_REDO_SHORTCUTS__) {
    return;
  }
  Object.defineProperty(window, '__WRY_UNDO_REDO_SHORTCUTS__', { value: true });

  document.addEventListener('keydown', function (event) {
    if (event.defaultPrevented || event.isComposing || event.altKey || event.metaKey || !event.ctrlKey) {
      return;
    }

    var key = (event.key || '').toLowerCase();
    var command = null;
    if (key === 'z') {
      command = event.shiftKey ? 'redo' : 'undo';
    } else if (key === 'y' && !event.shiftKey) {
      command = 'redo';
    }
    if (!command) {
      return;
    }

    // The page's own handlers run during the same dispatch, so wait for it to end
    // before deciding whether the page kept the shortcut for itself.
    setTimeout(function () {
      if (!event.defaultPrevented) {
        document.execCommand(command);
      }
    }, 0);
  });
})();
"#;
