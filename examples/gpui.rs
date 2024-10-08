use std::rc::Rc;

use gpui::*;
use wry::{
  dpi::{self, LogicalSize},
  Rect,
};

struct WebViewContainer {
  focus_handle: FocusHandle,
  webview: View<WebView>,
  pressed_keys: Vec<String>,
}

impl WebViewContainer {
  fn new(cx: &mut WindowContext) -> Self {
    let focus_handle = cx.focus_handle();
    focus_handle.focus(cx);

    Self {
      focus_handle,
      webview: cx.new_view(|cx| WebView::new(cx)),
      pressed_keys: vec![],
    }
  }
}

impl Render for WebViewContainer {
  fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
    div().flex().bg(rgb(0xF0F0F0)).size_full().p_10().child(
      div()
        .id("webview-container")
        .track_focus(&self.focus_handle)
        .on_key_down(cx.listener(|view, e: &KeyDownEvent, cx| {
          if view.pressed_keys.len() >= 10 {
            view.pressed_keys.remove(0);
          }

          view.pressed_keys.push(format!("{}", e.keystroke));
          cx.notify();
        }))
        .flex()
        .flex_col()
        .size_full()
        .justify_center()
        .items_center()
        .gap_4()
        .child("Wry WebView Demo")
        .child(self.pressed_keys.join(", "))
        .child(self.webview.clone()),
    )
  }
}

fn main() {
  App::new().run(|cx: &mut AppContext| {
    let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
    let window = cx
      .open_window(
        WindowOptions {
          window_bounds: Some(WindowBounds::Windowed(bounds)),
          kind: WindowKind::Normal,
          ..Default::default()
        },
        |cx| cx.new_view(|cx| WebViewContainer::new(cx)),
      )
      .unwrap();

    cx.spawn(|mut cx| async move {
      window
        .update(&mut cx, |_, cx| {
          cx.activate_window();
          cx.set_window_title("WebView Example");
          cx.on_release(|_, _, _cx| {
            // exit app
            std::process::exit(0);
          })
          .detach();
        })
        .unwrap();
    })
    .detach();
  });
}

/// A webview element that can display a URL or HTML content.
pub struct WebView {
  focus_handle: FocusHandle,
  view: Rc<wry::WebView>,
}

impl WebView {
  /// Create a new webview element.
  pub fn new(cx: &mut WindowContext) -> Self {
    let view = Rc::new(
      wry::WebViewBuilder::new_as_child(&cx.raw_window_handle())
        .with_url("https://www.google.com")
        .build()
        .expect("Failed to create webview."),
    );

    Self {
      focus_handle: cx.focus_handle(),
      view,
    }
  }
}

impl Render for WebView {
  fn render(&mut self, _cx: &mut ViewContext<Self>) -> impl IntoElement {
    div()
      .id("WebView")
      .track_focus(&self.focus_handle)
      .block()
      .overflow_y_scroll()
      .size_full()
      .justify_center()
      .items_center()
      .border_1()
      .rounded_md()
      .bg(gpui::white())
      .border_color(rgb(0xD0D0D0))
      .child(WebViewElement {
        view: self.view.clone(),
      })
  }
}

struct WebViewElement {
  view: Rc<wry::WebView>,
}
impl IntoElement for WebViewElement {
  type Element = WebViewElement;

  fn into_element(self) -> Self::Element {
    self
  }
}

impl Element for WebViewElement {
  type RequestLayoutState = ();
  type PrepaintState = Option<Hitbox>;

  fn id(&self) -> Option<ElementId> {
    None
  }

  #[allow(clippy::field_reassign_with_default)]
  fn request_layout(
    &mut self,
    _: Option<&GlobalElementId>,
    cx: &mut WindowContext,
  ) -> (LayoutId, Self::RequestLayoutState) {
    let mut style = Style::default();
    style.flex_grow = 1.0;
    style.size = Size::full();
    let id = cx.request_layout(style, []);
    (id, ())
  }

  fn prepaint(
    &mut self,
    _: Option<&GlobalElementId>,
    bounds: Bounds<Pixels>,
    _: &mut Self::RequestLayoutState,
    cx: &mut WindowContext,
  ) -> Self::PrepaintState {
    if bounds.top() > cx.viewport_size().height || bounds.bottom() < Pixels::ZERO {
      self.view.set_visible(false).unwrap();

      None
    } else {
      self.view.set_visible(true).unwrap();

      self
        .view
        .set_bounds(Rect {
          size: dpi::Size::Logical(LogicalSize {
            width: (bounds.size.width.0).into(),
            height: (bounds.size.height.0).into(),
          }),
          position: dpi::Position::Logical(dpi::LogicalPosition::new(
            bounds.origin.x.into(),
            bounds.origin.y.into(),
          )),
        })
        .unwrap();

      // Create a hitbox to handle mouse event
      Some(cx.insert_hitbox(bounds, false))
    }
  }

  fn paint(
    &mut self,
    _: Option<&GlobalElementId>,
    bounds: Bounds<Pixels>,
    _: &mut Self::RequestLayoutState,
    hitbox: &mut Self::PrepaintState,
    cx: &mut WindowContext,
  ) {
    let bounds = hitbox.clone().map(|h| h.bounds).unwrap_or(bounds);
    cx.with_content_mask(Some(ContentMask { bounds }), |cx| {
      let webview = self.view.clone();
      cx.on_mouse_event(move |event: &MouseDownEvent, _, cx| {
        if !bounds.contains(&event.position) {
          // Click white space to blur the input focus
          webview
            .evaluate_script(
              r#"
              document.querySelectorAll("input,textarea").forEach(input => input.blur());
              "#,
            )
            .expect("failed to evaluate_script to blur input");
        } else {
          cx.blur();
        }
      });
    });
  }
}
