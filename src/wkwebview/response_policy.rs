// Copyright 2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

// MIME is a routing hint. The consumer must still validate the downloaded bytes.
#[cfg(all(target_os = "macos", feature = "macos-download-pdf"))]
pub(super) fn force_pdf_download(
  main_frame: bool,
  scheme: Option<&str>,
  mime: Option<&str>,
) -> bool {
  main_frame
    && scheme.is_some_and(|s| s.eq_ignore_ascii_case("http") || s.eq_ignore_ascii_case("https"))
    && mime.is_some_and(|m| {
      m.split(';')
        .next()
        .unwrap_or("")
        .trim()
        .eq_ignore_ascii_case("application/pdf")
    })
}

#[cfg(target_os = "macos")]
pub(super) fn is_attachment(disposition: Option<&str>) -> bool {
  disposition.is_some_and(|value| {
    value
      .split(';')
      .next()
      .unwrap_or("")
      .trim()
      .eq_ignore_ascii_case("attachment")
  })
}

pub(super) fn should_download(has_handler: bool, can_show: bool, force_download: bool) -> bool {
  has_handler && (!can_show || force_download)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  #[cfg(target_os = "macos")]
  fn attachment_disposition_is_independent_of_filename() {
    for value in [
      "attachment",
      "ATTACHMENT",
      " attachment ; filename=fixture.png",
      "attachment; filename*=UTF-8''fixture.png",
    ] {
      assert!(is_attachment(Some(value)));
    }
    for value in [
      None,
      Some(""),
      Some("inline"),
      Some("inline; filename=attachment"),
      Some("attachment-invalid"),
    ] {
      assert!(!is_attachment(value));
    }
  }

  #[test]
  fn upstream_policy_and_handler_requirement() {
    for has_handler in [false, true] {
      for can_show in [false, true] {
        assert_eq!(
          should_download(has_handler, can_show, false),
          has_handler && !can_show
        );
        assert_eq!(should_download(has_handler, can_show, true), has_handler);
      }
    }
  }

  #[cfg(all(target_os = "macos", feature = "macos-download-pdf"))]
  #[test]
  fn only_remote_main_frame_pdf_responses_are_forced() {
    for scheme in ["http", "https", "HTTPS"] {
      for mime in [
        "application/pdf",
        "Application/PDF",
        " application/pdf; parameter=value",
      ] {
        assert!(force_pdf_download(true, Some(scheme), Some(mime)));
        assert!(!force_pdf_download(false, Some(scheme), Some(mime)));
      }
    }
    for scheme in [
      None,
      Some("blob"),
      Some("file"),
      Some("tauri"),
      Some("data"),
    ] {
      assert!(!force_pdf_download(true, scheme, Some("application/pdf")));
    }
    for mime in [
      None,
      Some("text/html"),
      Some("image/png"),
      Some("application/octet-stream"),
      Some("application/pdf-invalid"),
    ] {
      assert!(!force_pdf_download(true, Some("https"), mime));
    }
  }
}
