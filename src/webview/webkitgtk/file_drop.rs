// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{cell::Cell, path::PathBuf, rc::Rc};

use gtk::prelude::*;
use webkit2gtk::WebView;

use crate::{
  application::{dpi::LogicalPosition, window::Window},
  webview::FileDropEvent,
};

pub(crate) fn connect_drag_event(
  webview: Rc<WebView>,
  window: Rc<Window>,
  handler: Box<dyn Fn(&Window, FileDropEvent) -> bool>,
) {
  let listener = Rc::new((handler, Cell::new(None)));

  let listener_ref = listener.clone();
  let w = window.clone();
  webview.connect_drag_data_received(move |_, _, x, y, data, info, _| {
    if info == 2 {
      let uris = data
        .uris()
        .iter()
        .map(|gstr| {
          let path = gstr.as_str();
          PathBuf::from(path.to_string().strip_prefix("file://").unwrap_or(path))
        })
        .collect::<Vec<PathBuf>>();
      listener_ref.1.set(Some(uris.clone()));

      let scale_factor = w.scale_factor();
      let position = LogicalPosition::new(x, y).to_physical(scale_factor);

      listener_ref.0(
        &w,
        FileDropEvent::Hovered {
          paths: uris,
          position,
        },
      );
    } else {
      // drag_data_received is called twice, so we can ignore this signal
    }
  });

  let listener_ref = listener.clone();
  let w = window.clone();
  webview.connect_drag_drop(move |_, _, x, y, _| {
    let uris = listener_ref.1.take();
    if let Some(uris) = uris {
      let scale_factor = w.scale_factor();
      let position = LogicalPosition::new(x, y).to_physical(scale_factor);

      listener_ref.0(
        &w,
        FileDropEvent::Dropped {
          paths: uris,
          position,
        },
      )
    } else {
      false
    }
  });

  let listener_ref = listener.clone();
  let w = window.clone();
  webview.connect_drag_leave(move |_, _, time| {
    if time == 0 {
      // The user cancelled the drag n drop
      listener_ref.0(&w, FileDropEvent::Cancelled);
    } else {
      // The user dropped the file on the window, but this will be handled in connect_drag_drop instead
    }
  });

  // Called when a drag "fails" - we'll just emit a Cancelled event.
  let listener_ref = listener.clone();
  let w = window;
  webview.connect_drag_failed(move |_, _, _| {
    if listener_ref.0(&w, FileDropEvent::Cancelled) {
      gtk::glib::Propagation::Stop
    } else {
      gtk::glib::Propagation::Proceed
    }
  });
}
