use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use raw_window_handle::HasWindowHandle;
use servo::{CookieSource, StorageType, UrlRequest};
use tao::{event_loop::EventLoopProxy, window::Window};
use url::Url;

use crate::{Error, Rect, Result, WebViewAttributes, WebViewBuilder, WebViewId, RGBA};

use self::embedder::Embedder;

mod embedder;

fn parse_url(url: &str, description: &str) -> Result<Url> {
  Url::parse(url).map_err(|error| Error::Servo(format!("invalid {description}: {error}")))
}

fn html_url(html: &str) -> Result<Url> {
  parse_url(
    &format!(
      "data:text/html;charset=utf-8,{}",
      utf8_percent_encode(html, NON_ALPHANUMERIC)
    ),
    "HTML data URL",
  )
}

fn servo_color((red, green, blue, alpha): RGBA) -> [f64; 4] {
  [red, green, blue, alpha].map(|component| component as f64 / 255.0)
}

pub(crate) struct InnerWebView {
  id: String,
  embedder: Embedder,
}

impl InnerWebView {
  fn new_servo(
    window: Window,
    proxy: EventLoopProxy<()>,
    attributes: WebViewAttributes<'_>,
    _platform_attributes: super::PlatformSpecificWebViewAttributes,
  ) -> Result<Self> {
    let id = attributes.id.unwrap_or("servo").to_owned();
    let (initial_url, initial_headers) = match attributes.url {
      Some(url) => (parse_url(&url, "initial URL")?, attributes.headers),
      None => (
        if let Some(html) = attributes.html {
          html_url(&html)?
        } else {
          let demo_path = std::env::current_dir()?.join("examples/demo.html");
          Url::from_file_path(&demo_path).map_err(|()| {
            Error::Servo(format!(
              "failed to convert demo path to URL: {}",
              demo_path.display()
            ))
          })?
        },
        None,
      ),
    };

    let background_color = if attributes.transparent {
      Some([0.0; 4])
    } else {
      attributes.background_color.map(servo_color)
    };
    window.set_visible(attributes.visible);

    let embedder = Embedder::new(
      window,
      proxy,
      initial_url,
      initial_headers,
      background_color,
    )?;
    Ok(Self { id, embedder })
  }

  pub fn new<W: HasWindowHandle>(
    _window: &W,
    _attributes: WebViewAttributes<'_>,
    _platform_attributes: super::PlatformSpecificWebViewAttributes,
  ) -> Result<Self> {
    Err(Error::Servo(
      "the Servo backend must be created with WebViewBuilderExtServo::build_servo".into(),
    ))
  }

  pub fn new_as_child<W: HasWindowHandle>(
    _parent: &W,
    _attributes: WebViewAttributes<'_>,
    _platform_attributes: super::PlatformSpecificWebViewAttributes,
  ) -> Result<Self> {
    Err(Error::Servo(
      "child Servo webviews are not implemented yet".into(),
    ))
  }

  pub fn id(&self) -> WebViewId<'_> {
    &self.id
  }

  pub fn print(&self) -> Result<()> {
    Err(Error::Servo(
      "printing is not supported by Servo's embedding API".into(),
    ))
  }

  pub fn url(&self) -> Result<String> {
    Ok(
      self
        .embedder
        .webview()
        .url()
        .map(|url| url.to_string())
        .unwrap_or_default(),
    )
  }

  pub fn eval(
    &self,
    js: &str,
    callback: Option<impl FnOnce(String) + Send + 'static>,
  ) -> Result<()> {
    self
      .embedder
      .webview()
      .evaluate_javascript(js, move |result| {
        if let Some(callback) = callback {
          callback(match result {
            Ok(value) => format!("{value:?}"),
            Err(error) => format!("{error:?}"),
          });
        }
      });
    self.embedder.servo().spin_event_loop();
    Ok(())
  }

  pub fn cookies_for_url(&self, url: &str) -> Result<Vec<cookie::Cookie<'static>>> {
    let url = parse_url(url, "cookie URL")?;
    Ok(
      self
        .embedder
        .servo()
        .site_data_manager()
        .cookies_for_url(url, CookieSource::HTTP),
    )
  }

  pub fn cookies(&self) -> Result<Vec<cookie::Cookie<'static>>> {
    Err(Error::Servo(
      "enumerating every cookie is not supported by Servo; use cookies_for_url instead".into(),
    ))
  }

  pub fn set_cookie(&self, cookie: &cookie::Cookie<'_>) -> Result<()> {
    let url = self.url_for_cookie(cookie)?;
    self
      .embedder
      .servo()
      .site_data_manager()
      .set_cookie_for_url(url, cookie.clone().into_owned(), None);
    self.embedder.servo().spin_event_loop();
    Ok(())
  }

  pub fn delete_cookie(&self, cookie: &cookie::Cookie<'_>) -> Result<()> {
    let url = self.url_for_cookie(cookie)?;
    let mut cookie = cookie.clone().into_owned();
    cookie.make_removal();
    self
      .embedder
      .servo()
      .site_data_manager()
      .set_cookie_for_url(url, cookie, None);
    self.embedder.servo().spin_event_loop();
    Ok(())
  }

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn open_devtools(&self) {}

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn close_devtools(&self) {}

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn is_devtools_open(&self) -> bool {
    false
  }

  pub fn zoom(&self, scale_factor: f64) -> Result<()> {
    self.embedder.webview().set_page_zoom(scale_factor as f32);
    self.embedder.servo().spin_event_loop();
    Ok(())
  }

  pub fn set_background_color(&self, background_color: RGBA) -> Result<()> {
    self
      .embedder
      .set_background_color(servo_color(background_color));
    Ok(())
  }

  pub fn load_url(&self, url: &str) -> Result<()> {
    let url = Url::parse(url).map_err(|error| Error::Servo(format!("invalid URL: {error}")))?;
    self.embedder.webview().load(url);
    self.embedder.servo().spin_event_loop();
    Ok(())
  }

  pub fn reload(&self) -> Result<()> {
    self.embedder.webview().reload();
    self.embedder.servo().spin_event_loop();
    Ok(())
  }

  pub fn go_forward(&self) -> Result<()> {
    if self.embedder.webview().can_go_forward() {
      self.embedder.webview().go_forward(1);
      self.embedder.servo().spin_event_loop();
    }
    Ok(())
  }

  pub fn go_back(&self) -> Result<()> {
    if self.embedder.webview().can_go_back() {
      self.embedder.webview().go_back(1);
      self.embedder.servo().spin_event_loop();
    }
    Ok(())
  }

  pub fn can_go_forward(&self) -> Result<bool> {
    Ok(self.embedder.webview().can_go_forward())
  }

  pub fn can_go_back(&self) -> Result<bool> {
    Ok(self.embedder.webview().can_go_back())
  }

  pub fn load_url_with_headers(&self, url: &str, headers: http::HeaderMap) -> Result<()> {
    let url = parse_url(url, "URL")?;
    self
      .embedder
      .webview()
      .load_request(UrlRequest::new(url).headers(headers));
    self.embedder.servo().spin_event_loop();
    Ok(())
  }

  pub fn load_html(&self, html: &str) -> Result<()> {
    let url = html_url(html)?;
    self.embedder.webview().load(url);
    self.embedder.servo().spin_event_loop();
    Ok(())
  }

  pub fn clear_all_browsing_data(&self) -> Result<()> {
    let servo = self.embedder.servo();
    let site_data_manager = servo.site_data_manager();
    site_data_manager.clear_cookies(None);

    let storage_types = StorageType::Local | StorageType::Session;
    let sites = site_data_manager.site_data(storage_types);
    let site_names = sites.iter().map(|site| site.name()).collect::<Vec<_>>();
    let site_names = site_names.iter().map(String::as_str).collect::<Vec<_>>();
    site_data_manager.clear_site_data(&site_names, storage_types);
    servo.network_manager().clear_cache();
    servo.spin_event_loop();
    Ok(())
  }

  pub fn bounds(&self) -> Result<Rect> {
    let size = self.embedder.window().inner_size();
    Ok(Rect {
      position: dpi::PhysicalPosition::new(0, 0).into(),
      size: dpi::PhysicalSize::new(size.width, size.height).into(),
    })
  }

  pub fn set_bounds(&self, bounds: Rect) -> Result<()> {
    let size = bounds
      .size
      .to_physical::<u32>(self.embedder.window().scale_factor());
    self.embedder.webview().resize(size);
    self.embedder.servo().spin_event_loop();
    Ok(())
  }

  pub fn set_visible(&self, visible: bool) -> Result<()> {
    self.embedder.window().set_visible(visible);
    if visible {
      self.embedder.webview().show();
    } else {
      self.embedder.webview().hide();
    }
    self.embedder.servo().spin_event_loop();
    Ok(())
  }

  pub fn focus(&self) -> Result<()> {
    self.embedder.window().set_focus();
    self.embedder.webview().focus();
    self.embedder.servo().spin_event_loop();
    Ok(())
  }

  pub fn focus_parent(&self) -> Result<()> {
    self.embedder.window().set_focus();
    Ok(())
  }

  fn url_for_cookie(&self, cookie: &cookie::Cookie<'_>) -> Result<Url> {
    let current_url = self
      .embedder
      .webview()
      .url()
      .ok_or_else(|| Error::Servo("cannot set a cookie before Servo has a current URL".into()))?;
    let Some(domain) = cookie.domain() else {
      return Ok(current_url);
    };

    let domain = domain.trim_start_matches('.');
    let current_host_matches = current_url
      .host_str()
      .is_some_and(|host| host == domain || host.ends_with(&format!(".{domain}")));
    let secure = cookie.secure().unwrap_or(false);
    if current_host_matches && (!secure || current_url.scheme() == "https") {
      return Ok(current_url);
    }

    let scheme = if secure { "https" } else { "http" };
    let path = cookie.path().unwrap_or("/");
    parse_url(&format!("{scheme}://{domain}{path}"), "cookie URL")
  }
}

pub fn platform_webview_version() -> Result<String> {
  Ok("Servo main (3f08ca6d)".into())
}

pub trait WebViewBuilderExtServo<'a> {
  fn build_servo(self, window: Window, proxy: EventLoopProxy<()>) -> Result<super::WebView>;
}

impl<'a> WebViewBuilderExtServo<'a> for WebViewBuilder<'a> {
  fn build_servo(self, window: Window, proxy: EventLoopProxy<()>) -> Result<super::WebView> {
    self.error?;
    InnerWebView::new_servo(window, proxy, self.attrs, self.platform_specific)
      .map(|webview| super::WebView { webview })
  }
}

pub trait WebViewExtServo {
  fn servo(&mut self) -> &mut Embedder;
}

impl WebViewExtServo for super::WebView {
  fn servo(&mut self) -> &mut Embedder {
    &mut self.webview.embedder
  }
}

#[cfg(test)]
mod tests {
  use super::{html_url, servo_color};

  #[test]
  fn html_is_encoded_as_an_opaque_data_url() {
    let url = html_url("<p id=\"value\">#100%</p>").unwrap();

    assert_eq!(url.scheme(), "data");
    assert!(matches!(url.origin(), url::Origin::Opaque(_)));
    assert!(url.fragment().is_none());
    assert!(url.as_str().contains("%23"));
    assert!(url.as_str().contains("%25"));
  }

  #[test]
  fn converts_wry_color_components_to_servo_range() {
    assert_eq!(
      servo_color((0, 127, 255, 64)),
      [0.0, 127.0 / 255.0, 1.0, 64.0 / 255.0]
    );
  }
}
