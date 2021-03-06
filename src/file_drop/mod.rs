#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux::*;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::*;
#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "windows")]
pub(crate) use win::*;

use core::fmt;
use std::{cell::Cell, path::PathBuf, sync::Arc};

#[derive(Debug, Serialize, Clone)]
/// The status of a file drop event.
pub enum FileDropStatus {
    /// The file(s) have been dragged onto the window, but have not been dropped yet.
    Hovered(Vec<PathBuf>),

    /// The file(s) have been dropped onto the window.
    Dropped(Vec<PathBuf>),

    /// The file(s) drop was aborted.
    Cancelled(Vec<PathBuf>),
}

// This needs to be defined because internally the respective events do not always yield a PathBuf.
// We can easily remember what was cancelled though, as Hovered and Dropped events will always yield a PathBuf which we will save ourselves for later reference.
pub(crate) enum FileDropEvent {
    Hovered,
    Dropped,
    Cancelled
}

#[derive(Clone)]
pub struct FileDropHandler {
    f: Arc<Box<dyn Fn(FileDropStatus) -> bool + Send + Sync + 'static>>
}
impl FileDropHandler {
    /// Initializes a new file drop handler.
    ///
    /// Example: FileDropHandler:new(|status: FileDropStatus| ...)
    ///
    /// ### Blocking OS Default Behavior
    /// Return `true` in the callback to block the OS' default behavior of handling a file drop.
    ///
    /// Note, that if you do block this behavior, it won't be possible to drop files on `<input type="file">` forms.
    /// Also note, that it's not possible to manually set the value of a `<input type="file">` via JavaScript for security reasons.
    pub fn new<T>(f: T) -> FileDropHandler
    where
        T: Fn(FileDropStatus) -> bool + Send + Sync + 'static
    {
        FileDropHandler { f: Arc::new(Box::new(f)) }
    }

    /// Manually invokes the file drop handler closures with a provided FileDropStatus(Vec<PathBuf>)
    pub fn call(&self, status: FileDropStatus) -> bool {
        (self.f)(status)
    }
}
impl fmt::Debug for FileDropHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FileDropHandler")
    }
}

pub(crate) struct FileDropListener {
    pub(crate) handlers: (Option<FileDropHandler>, Option<FileDropHandler>),
    pub(crate) active_file_drop: Cell<Option<FileDropStatus>>
}
impl FileDropListener {
    pub(crate) fn new(handlers: (Option<FileDropHandler>, Option<FileDropHandler>)) -> FileDropListener {
        debug_assert!(handlers.0.is_some() || handlers.1.is_some(), "Tried to create a FileDropListener with no file drop handlers!");
        FileDropListener {
            handlers,
            active_file_drop: Cell::new(None),
        }
    }

    // Called when a file drop event occurs. Bubbles the event up to the handler.
    // Return true to prevent the OS' default action for the file drop.
    pub(crate) fn file_drop(&self, event: FileDropEvent, paths: Option<Vec<PathBuf>>) -> bool {
        let paths = match event {

            FileDropEvent::Hovered => {
                if paths.is_none() || paths.as_ref().unwrap().is_empty() {
                    debug_assert!(false, "FileDropEvent::Hovered received with missing or empty paths list!");
                    return false;
                }
                paths.unwrap()
            },

            _ => match paths {

                Some(paths) => paths,

                None => match self.active_file_drop.take() {
                    None => {
                        debug_assert!(false, "Failed to retrieve paths list from memory for this file drop event!");
                        return false;
                    },
                    Some(status) => match status {
                        FileDropStatus::Hovered(paths) => paths,
                        FileDropStatus::Dropped(paths) => paths,
                        FileDropStatus::Cancelled(paths) => paths
                    }
                }

            }

        };

        let new_status = match event {
            FileDropEvent::Hovered => FileDropStatus::Hovered(paths),
            FileDropEvent::Dropped => FileDropStatus::Dropped(paths),
            FileDropEvent::Cancelled => FileDropStatus::Cancelled(paths)
        };

        self.active_file_drop.set(Some(new_status.clone()));
        self.call(new_status)
    }

    fn call(&self, status: FileDropStatus) -> bool {
        // Kind of silly, but the most memory efficient
        let mut prevent_default = false;
        match self.handlers.0 {
            Some(ref webview_file_drop_handler) => {
                match self.handlers.1 {
                    Some(ref app_file_drop_handler) => {
                        prevent_default = webview_file_drop_handler.call(status.clone()) | app_file_drop_handler.call(status);
                    },
                    None => prevent_default = webview_file_drop_handler.call(status)
                }
            },
            None => {
                match self.handlers.1 {
                    Some(ref app_file_drop_handler) => prevent_default = app_file_drop_handler.call(status),
                    None => {}
                }
            }
        }
        prevent_default
    }
}
