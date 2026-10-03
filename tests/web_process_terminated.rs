#![cfg(all(target_os = "linux", feature = "os-webview"))]

use std::{
  cell::{Cell, RefCell},
  fs,
  process::{self, Command},
  rc::Rc,
  thread,
  time::{Duration, Instant},
};

use gtk::prelude::*;
use webkit2gtk::WebViewExt;
use wry::{WebView, WebViewBuilder, WebViewBuilderExtUnix, WebViewExtUnix};

const HTML: &str = r#"<script>window.ipc.postMessage('ready')</script>"#;

fn wait_until(condition: impl Fn() -> bool) {
  let deadline = Instant::now() + Duration::from_secs(15);
  while !condition() {
    assert!(
      Instant::now() < deadline,
      "Timed out waiting for the web process"
    );
    gtk::main_iteration_do(false);
    thread::sleep(Duration::from_millis(1));
  }
}

fn kill_web_process() {
  let mut web_processes = Vec::new();
  for task in fs::read_dir(format!("/proc/{}/task", process::id())).unwrap() {
    let children = fs::read_to_string(task.unwrap().path().join("children")).unwrap();
    for pid in children.split_whitespace() {
      if fs::read_link(format!("/proc/{pid}/exe")).is_ok_and(|path| {
        path
          .file_name()
          .is_some_and(|name| name == "WebKitWebProcess")
      }) {
        web_processes.push(pid.to_owned());
      }
    }
  }
  assert_eq!(web_processes.len(), 1);
  assert!(
    Command::new("kill")
      .args(["-KILL", &web_processes[0]])
      .status()
      .unwrap()
      .success()
  );
}

#[test]
#[ignore = "Requires a display and a running D-Bus session"]
fn termination_notifies_on_main_thread_and_allows_recovery() {
  gtk::init().unwrap();
  let window = gtk::Window::new(gtk::WindowType::Toplevel);
  let webview = Rc::new(RefCell::new(None::<WebView>));
  let notifications = Rc::new(Cell::new(0));
  let loads = Rc::new(Cell::new(0));
  let main_thread = thread::current().id();
  let builder = {
    let webview = Rc::downgrade(&webview);
    let notifications = notifications.clone();
    let loads = loads.clone();
    WebViewBuilder::new()
      .with_html(HTML)
      .with_ipc_handler(move |_| loads.set(loads.get() + 1))
      .with_on_web_content_process_terminate_handler(move || {
        assert_eq!(thread::current().id(), main_thread);
        notifications.set(notifications.get() + 1);
        webview
          .upgrade()
          .unwrap()
          .borrow()
          .as_ref()
          .unwrap()
          .load_html(HTML)
          .unwrap();
      })
  };
  *webview.borrow_mut() = Some(builder.build_gtk(&window).unwrap());
  window.show_all();
  wait_until(|| loads.get() == 1);

  for expected in 1..=2 {
    if expected == 1 {
      kill_web_process();
    } else {
      webview
        .borrow()
        .as_ref()
        .unwrap()
        .webview()
        .terminate_web_process();
    }
    wait_until(|| loads.get() == expected + 1);
    assert_eq!(notifications.get(), expected);
  }
}
