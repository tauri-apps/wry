use std::path::PathBuf;

#[derive(Debug, Serialize, Clone)]
pub enum FileDropEvent {
  /// The file(s) have been dragged onto the window, but have not been dropped yet.
  Hovered(Vec<PathBuf>),
  /// The file(s) have been dropped onto the window.
  Dropped(Vec<PathBuf>),
  /// The file drop was aborted.
  Cancelled,
}

/// Initializes a new file drop handler.
///
/// Example: `FileDropHandler:new(|data: FileDropData| ...)`
///
/// ### Blocking OS Default Behavior
/// Return `true` in the callback to block the OS' default behavior of handling a file drop.
///
/// Note, that if you do block this behavior, it won't be possible to drop files on `<input type="file">` forms.
/// Also note, that it's not possible to manually set the value of a `<input type="file">` via JavaScript for security reasons.
pub type FileDropHandler = Box<dyn Fn(FileDropEvent) -> bool + Send>;
