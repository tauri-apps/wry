---
wry: patch
---

On Linux, a container that is a type derived from `gtk::Fixed` gets the same treatment as `gtk::Fixed` itself in `WebViewBuilderExtUnix::build_gtk`, `WebViewExtUnix::new_gtk` and `WebViewExtUnix::reparent`. Only a container of the exact type `gtk::Fixed` did before.
