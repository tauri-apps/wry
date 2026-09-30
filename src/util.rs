use std::sync::atomic::{AtomicU32, Ordering};

#[cfg(any(target_os = "macos", test))]
pub(crate) fn create_panel_or_cancel<T>(
  create: impl FnOnce() -> Option<T>,
  cancel: impl FnOnce(),
) -> Option<T> {
  match create() {
    Some(panel) => Some(panel),
    None => {
      cancel();
      None
    }
  }
}

pub struct Counter(AtomicU32);

impl Counter {
  pub const fn new() -> Self {
    Self(AtomicU32::new(1))
  }

  pub fn next(&self) -> u32 {
    self.0.fetch_add(1, Ordering::Relaxed)
  }
}

#[cfg(test)]
mod tests {
  use super::create_panel_or_cancel;

  #[test]
  fn a_missing_file_panel_completes_the_request_as_cancelled() {
    let mut cancelled = false;

    let panel = create_panel_or_cancel(|| None::<()>, || cancelled = true);

    assert!(panel.is_none());
    assert!(cancelled);
  }

  #[test]
  fn a_created_file_panel_does_not_cancel_the_request() {
    let mut cancelled = false;

    let panel = create_panel_or_cancel(|| Some("panel"), || cancelled = true);

    assert_eq!(panel, Some("panel"));
    assert!(!cancelled);
  }
}
