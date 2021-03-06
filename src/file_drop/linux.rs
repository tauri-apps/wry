use super::{FileDropEvent, FileDropHandler, FileDropListener};

use std::{path::PathBuf, rc::Rc};

use webkit2gtk::WebView;
use gtk::WidgetExt;

pub(crate) struct FileDropController;
impl FileDropController {
	pub(crate) fn new(webview: Rc<WebView>, handlers: (Option<FileDropHandler>, Option<FileDropHandler>)) {
		if handlers.0.is_none() && handlers.1.is_none() { return }

		let listener = Rc::new(FileDropListener::new(handlers));

		let listener_ref = listener.clone();
		webview.connect_drag_data_received(move |_, _, _, _, data, info, _| {
			if info == 2 {
				let uris = data.get_uris().iter().map(|gstr| {
					let path = gstr.as_str();
					PathBuf::from(path.to_string().strip_prefix("file://").unwrap_or(path))
				}).collect::<Vec<PathBuf>>();
	
				listener_ref.file_drop(FileDropEvent::Hovered, Some(uris));
			} else {
				// drag_data_received is called twice, so we can ignore this signal
			}
		});
	
		let listener_ref = listener.clone();
		webview.connect_drag_drop(move |_, _, x, y, time| {
			gtk::Inhibit(listener_ref.file_drop(FileDropEvent::Dropped, None))
		});
	
		let listener_ref = listener.clone();
		webview.connect_drag_leave(move |_, _, time| {
			if time == 0 {
				// The user cancelled the drag n drop
				listener_ref.file_drop(FileDropEvent::Cancelled, None);
			} else {
				// The user dropped the file on the window, but this will be handled in connect_drag_drop instead
			}
		});
	
		let listener_ref = listener.clone();
		webview.connect_drag_failed(move |_, _, _| {
			gtk::Inhibit(listener_ref.file_drop(FileDropEvent::Cancelled, None))
		});
	}
}