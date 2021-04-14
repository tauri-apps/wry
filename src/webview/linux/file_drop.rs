// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{cell::Cell, path::PathBuf, rc::Rc};

use gtk::WidgetExt;
use webkit2gtk::WebView;

use crate::{webview::FileDropHandler, FileDropEvent};

pub(crate) fn connect_drag_event(webview: Rc<WebView>, handler: FileDropHandler) {
  let listener = Rc::new((handler, Cell::new(None)));

  let listener_ref = listener.clone();
  webview.connect_drag_data_received(move |_, _, _, _, data, info, _| {
    if info == 2 {
      let uris = data
        .get_uris()
        .iter()
        .map(|gstr| {
          let path = gstr.as_str();
          PathBuf::from(path.to_string().strip_prefix("file://").unwrap_or(path))
        })
        .collect::<Vec<PathBuf>>();

      listener_ref.1.set(Some(uris.clone()));
      listener_ref.0(FileDropEvent::Hovered(uris));
    } else {
      // drag_data_received is called twice, so we can ignore this signal
    }
  });

  let listener_ref = listener.clone();
  webview.connect_drag_drop(move |_, _, _, _, _| {
    let uris = listener_ref.1.take();
    if let Some(uris) = uris {
      gtk::Inhibit(listener_ref.0(FileDropEvent::Dropped(uris)))
    } else {
      gtk::Inhibit(false)
    }
  });

  let listener_ref = listener.clone();
  webview.connect_drag_leave(move |_, _, time| {
    if time == 0 {
      // The user cancelled the drag n drop
      listener_ref.0(FileDropEvent::Cancelled);
    } else {
      // The user dropped the file on the window, but this will be handled in connect_drag_drop instead
    }
  });

  // Called when a drag "fails" - we'll just emit a Cancelled event.
  let listener_ref = listener.clone();
  webview
    .connect_drag_failed(move |_, _, _| gtk::Inhibit(listener_ref.0(FileDropEvent::Cancelled)));
}
