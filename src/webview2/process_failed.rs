use std::rc::Rc;

use webview2_com::{Microsoft::Web::WebView2::Win32::*, ProcessFailedEventHandler, take_pwstr};
use windows::Win32::Foundation::E_POINTER;
use windows::core::{Interface, PWSTR};

/// The kind of WebView2 process failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WebView2ProcessFailedKind {
  /// The browser process exited.
  BrowserProcessExited,
  /// The main frame's renderer process exited.
  RenderProcessExited,
  /// The main frame's renderer process is unresponsive.
  RenderProcessUnresponsive,
  /// A renderer hosting a subframe exited.
  FrameRenderProcessExited,
  /// The GPU process exited.
  GpuProcessExited,
  /// A utility process exited.
  UtilityProcessExited,
  /// The sandbox helper process exited.
  SandboxHelperProcessExited,
  /// A PPAPI plugin process exited.
  PpapiPluginProcessExited,
  /// A PPAPI broker process exited.
  PpapiBrokerProcessExited,
  /// The runtime reported another failure kind.
  Unknown,
}

/// The reason reported by WebView2 for a process failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WebView2ProcessFailedReason {
  /// An unexpected failure occurred.
  Unexpected,
  /// The process became unresponsive.
  Unresponsive,
  /// The process was terminated.
  Terminated,
  /// The process crashed.
  Crashed,
  /// The process could not be launched.
  LaunchFailed,
  /// The process ran out of memory.
  OutOfMemory,
  /// The profile was deleted.
  ProfileDeleted,
  /// The runtime reported another failure reason.
  Unknown,
}

/// Details of a WebView2 process failure.
///
/// Optional diagnostics are `None` when the runtime does not support the extended event
/// arguments or cannot provide the corresponding value. See
/// [`WebViewBuilderExtWindows::with_process_failed_handler`](crate::WebViewBuilderExtWindows::with_process_failed_handler).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct WebView2ProcessFailedInfo {
  /// The process and failure kind.
  pub kind: WebView2ProcessFailedKind,
  /// The failure reason, if available.
  pub reason: Option<WebView2ProcessFailedReason>,
  /// The process exit code, if available.
  pub exit_code: Option<i32>,
  /// A description of the failed process, if available.
  pub process_description: Option<String>,
}

pub(super) struct ProcessFailedHandler {
  webview: ICoreWebView2,
  token: i64,
}

impl ProcessFailedHandler {
  pub(super) fn register(
    webview: &ICoreWebView2,
    handler: Rc<dyn Fn(WebView2ProcessFailedInfo)>,
  ) -> windows::core::Result<Self> {
    let mut token = 0;
    unsafe {
      webview.add_ProcessFailed(
        &ProcessFailedEventHandler::create(Box::new(move |_, args| {
          let args = args.ok_or_else(|| windows::core::Error::from(E_POINTER))?;
          handler(process_failed_info(&args)?);
          Ok(())
        })),
        &mut token,
      )?;
    }
    Ok(Self {
      webview: webview.clone(),
      token,
    })
  }
}

impl Drop for ProcessFailedHandler {
  fn drop(&mut self) {
    let _ = unsafe { self.webview.remove_ProcessFailed(self.token) };
  }
}

fn process_failed_info(
  args: &ICoreWebView2ProcessFailedEventArgs,
) -> windows::core::Result<WebView2ProcessFailedInfo> {
  let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
  unsafe { args.ProcessFailedKind(&mut kind)? };
  let mut info = WebView2ProcessFailedInfo {
    kind: process_failed_kind(kind),
    reason: None,
    exit_code: None,
    process_description: None,
  };
  if let Ok(args) = args.cast::<ICoreWebView2ProcessFailedEventArgs2>() {
    let mut reason = COREWEBVIEW2_PROCESS_FAILED_REASON::default();
    info.reason = unsafe { args.Reason(&mut reason) }
      .ok()
      .map(|()| process_failed_reason(reason));
    let mut exit_code = 0;
    info.exit_code = unsafe { args.ExitCode(&mut exit_code) }
      .ok()
      .map(|()| exit_code);
    let mut description = PWSTR::null();
    if unsafe { args.ProcessDescription(&mut description) }.is_ok() && !description.is_null() {
      info.process_description = Some(take_pwstr(description));
    }
  }
  Ok(info)
}

fn process_failed_kind(kind: COREWEBVIEW2_PROCESS_FAILED_KIND) -> WebView2ProcessFailedKind {
  match kind {
    COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED => {
      WebView2ProcessFailedKind::BrowserProcessExited
    }
    COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED => {
      WebView2ProcessFailedKind::RenderProcessExited
    }
    COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE => {
      WebView2ProcessFailedKind::RenderProcessUnresponsive
    }
    COREWEBVIEW2_PROCESS_FAILED_KIND_FRAME_RENDER_PROCESS_EXITED => {
      WebView2ProcessFailedKind::FrameRenderProcessExited
    }
    COREWEBVIEW2_PROCESS_FAILED_KIND_GPU_PROCESS_EXITED => {
      WebView2ProcessFailedKind::GpuProcessExited
    }
    COREWEBVIEW2_PROCESS_FAILED_KIND_UTILITY_PROCESS_EXITED => {
      WebView2ProcessFailedKind::UtilityProcessExited
    }
    COREWEBVIEW2_PROCESS_FAILED_KIND_SANDBOX_HELPER_PROCESS_EXITED => {
      WebView2ProcessFailedKind::SandboxHelperProcessExited
    }
    COREWEBVIEW2_PROCESS_FAILED_KIND_PPAPI_PLUGIN_PROCESS_EXITED => {
      WebView2ProcessFailedKind::PpapiPluginProcessExited
    }
    COREWEBVIEW2_PROCESS_FAILED_KIND_PPAPI_BROKER_PROCESS_EXITED => {
      WebView2ProcessFailedKind::PpapiBrokerProcessExited
    }
    _ => WebView2ProcessFailedKind::Unknown,
  }
}

fn process_failed_reason(
  reason: COREWEBVIEW2_PROCESS_FAILED_REASON,
) -> WebView2ProcessFailedReason {
  match reason {
    COREWEBVIEW2_PROCESS_FAILED_REASON_UNEXPECTED => WebView2ProcessFailedReason::Unexpected,
    COREWEBVIEW2_PROCESS_FAILED_REASON_UNRESPONSIVE => WebView2ProcessFailedReason::Unresponsive,
    COREWEBVIEW2_PROCESS_FAILED_REASON_TERMINATED => WebView2ProcessFailedReason::Terminated,
    COREWEBVIEW2_PROCESS_FAILED_REASON_CRASHED => WebView2ProcessFailedReason::Crashed,
    COREWEBVIEW2_PROCESS_FAILED_REASON_LAUNCH_FAILED => WebView2ProcessFailedReason::LaunchFailed,
    COREWEBVIEW2_PROCESS_FAILED_REASON_OUT_OF_MEMORY => WebView2ProcessFailedReason::OutOfMemory,
    COREWEBVIEW2_PROCESS_FAILED_REASON_PROFILE_DELETED => {
      WebView2ProcessFailedReason::ProfileDeleted
    }
    _ => WebView2ProcessFailedReason::Unknown,
  }
}

#[cfg(test)]
mod tests {
  use std::cell::Cell;

  use super::*;
  use crate::{WebViewBuilder, WebViewBuilderExtWindows};

  #[test]
  fn distinguishes_failure_kinds_and_unknown_values() {
    assert_eq!(
      process_failed_kind(COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED),
      WebView2ProcessFailedKind::BrowserProcessExited
    );
    assert_eq!(
      process_failed_kind(COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED),
      WebView2ProcessFailedKind::RenderProcessExited
    );
    assert_eq!(
      process_failed_kind(COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE),
      WebView2ProcessFailedKind::RenderProcessUnresponsive
    );
    assert_eq!(
      process_failed_kind(COREWEBVIEW2_PROCESS_FAILED_KIND_FRAME_RENDER_PROCESS_EXITED),
      WebView2ProcessFailedKind::FrameRenderProcessExited
    );
    assert_eq!(
      process_failed_kind(COREWEBVIEW2_PROCESS_FAILED_KIND_GPU_PROCESS_EXITED),
      WebView2ProcessFailedKind::GpuProcessExited
    );
    assert_eq!(
      process_failed_kind(COREWEBVIEW2_PROCESS_FAILED_KIND(-1)),
      WebView2ProcessFailedKind::Unknown
    );
  }

  #[test]
  fn distinguishes_failure_reasons_and_unknown_values() {
    assert_eq!(
      process_failed_reason(COREWEBVIEW2_PROCESS_FAILED_REASON_CRASHED),
      WebView2ProcessFailedReason::Crashed
    );
    assert_eq!(
      process_failed_reason(COREWEBVIEW2_PROCESS_FAILED_REASON_OUT_OF_MEMORY),
      WebView2ProcessFailedReason::OutOfMemory
    );
    assert_eq!(
      process_failed_reason(COREWEBVIEW2_PROCESS_FAILED_REASON(-1)),
      WebView2ProcessFailedReason::Unknown
    );
  }

  #[test]
  fn handler_is_opt_in_and_per_webview() {
    assert!(
      WebViewBuilder::new()
        .platform_specific
        .process_failed_handler
        .is_none()
    );
    let first_count = Rc::new(Cell::new(0));
    let second_count = Rc::new(Cell::new(0));
    let first = WebViewBuilder::new().with_process_failed_handler({
      let count = first_count.clone();
      move |_| count.set(count.get() + 1)
    });
    let second = WebViewBuilder::new().with_process_failed_handler({
      let count = second_count.clone();
      move |_| count.set(count.get() + 1)
    });
    let info = WebView2ProcessFailedInfo {
      kind: WebView2ProcessFailedKind::RenderProcessExited,
      reason: None,
      exit_code: None,
      process_description: None,
    };
    first.platform_specific.process_failed_handler.unwrap()(info.clone());
    assert_eq!(first_count.get(), 1);
    assert_eq!(second_count.get(), 0);
    second.platform_specific.process_failed_handler.unwrap()(info);
    assert_eq!(first_count.get(), 1);
    assert_eq!(second_count.get(), 1);
  }
}
