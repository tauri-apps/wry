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

use crate::mimetype::MimeType;

use core::fmt;
use std::{cell::Cell, path::PathBuf, sync::Arc};

#[derive(Debug, Serialize, Clone, Copy)]
pub enum FileDropEvent {
    /// The file(s) have been dragged onto the window, but have not been dropped yet.
    Hovered,

    /// The file(s) have been dropped onto the window.
    Dropped,

    /// The file drop was aborted.
    Cancelled,
}

/// Data about a file that was dropped on a webview.
#[derive(Debug, Serialize, Clone)]
pub struct FileDrop {
    path: PathBuf,
    mime: MimeType,
}
impl From<PathBuf> for FileDrop {
    fn from(path: PathBuf) -> Self {
        let mime = match path.is_dir() {
            true => MimeType::DIRECTORY,
            false => MimeType::parse_from_uri(&path.to_string_lossy().to_string()),
        };
        FileDrop { path, mime }
    }
}

/// Data about a webview file drop event.
#[derive(Debug, Serialize, Clone)]
pub struct FileDropData {
    event: FileDropEvent,
    files: Vec<FileDrop>,
}

#[derive(Clone)]
pub struct FileDropHandler {
    f: Arc<Box<dyn Fn(FileDropData) -> bool + Send + Sync + 'static>>,
}
impl FileDropHandler {
    /// Initializes a new file drop handler.
    ///
    /// Example: `FileDropHandler:new(|data: FileDropData| ...)`
    ///
    /// ### Blocking OS Default Behavior
    /// Return `true` in the callback to block the OS' default behavior of handling a file drop.
    ///
    /// Note, that if you do block this behavior, it won't be possible to drop files on `<input type="file">` forms.
    /// Also note, that it's not possible to manually set the value of a `<input type="file">` via JavaScript for security reasons.
    pub fn new<T>(f: T) -> FileDropHandler
    where
        T: Fn(FileDropData) -> bool + Send + Sync + 'static,
    {
        FileDropHandler {
            f: Arc::new(Box::new(f)),
        }
    }

    /// Manually invokes the file drop handler closures with a provided FileDropData(Vec<PathBuf>)
    pub fn call(&self, data: FileDropData) -> bool {
        (self.f)(data)
    }
}
impl fmt::Debug for FileDropHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FileDropHandler")
    }
}

pub(crate) struct FileDropListener {
    pub(crate) handler: FileDropHandler,
    pub(crate) active_file_drop: Cell<Option<FileDropData>>,
}
impl FileDropListener {
    pub(crate) fn new(handler: FileDropHandler) -> FileDropListener {
        FileDropListener {
            handler,
            active_file_drop: Cell::new(None),
        }
    }

    // Called when a file drop event occurs. Bubbles the event up to the handler.
    // Return true to prevent the OS' default action for the file drop.
    pub(crate) fn file_drop(&self, event: FileDropEvent, paths: Option<Vec<PathBuf>>) -> bool {
        let data: FileDropData = match event {
            FileDropEvent::Hovered => {
                if paths.is_none() || paths.as_ref().unwrap().is_empty() {
                    debug_assert!(
                        false,
                        "FileDropEvent::Hovered received with missing or empty paths list!"
                    );
                    return false;
                }
                FileDropData {
                    files: paths
                        .unwrap()
                        .into_iter()
                        .map(FileDrop::from)
                        .collect::<Vec<FileDrop>>(),
                    event,
                }
            }

            _ => {
                match paths {
                    Some(paths) => FileDropData {
                        files: paths
                            .into_iter()
                            .map(FileDrop::from)
                            .collect::<Vec<FileDrop>>(),
                        event,
                    },

                    None => match self.active_file_drop.take() {
                        None => {
                            debug_assert!(false, "Failed to retrieve paths list from memory for this file drop event!");
                            return false;
                        }
                        Some(mut data) => {
                            data.event = event;
                            data
                        }
                    },
                }
            }
        };

        self.active_file_drop.set(Some(data.clone()));
        self.handler.call(data)
    }
}
