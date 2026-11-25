---
wry: patch
---

On Linux, a container that is a type derived from `gtk::Box` gets the same treatment as `gtk::Box` itself in `WebViewBuilderExtUnix::build_gtk`, `WebViewExtUnix::new_gtk` and `WebViewExtUnix::reparent`. Only a container of the exact type `gtk::Box` did before.
