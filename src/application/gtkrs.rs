use crate::{
    AppWindowAttributes, ApplicationDispatcher, ApplicationExt, Callback, Message, Result, WebView,
    WebViewAttributes, WebViewBuilder, WindowExt,
};

use std::{
    collections::HashMap,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
};

use gio::{ApplicationExt as GioApplicationExt, Cancellable};
use gtk::{
    Application as GtkApp, ApplicationWindow, ApplicationWindowExt, GtkWindowExt, Inhibit,
    WidgetExt,
};

pub struct Application<T> {
    webviews: HashMap<u32, WebView>,
    app: GtkApp,
    event_loop_proxy: EventLoopProxy<u32, T>,
    event_loop_proxy_rx: Receiver<Message<u32, T>>,
    message_handler: Option<Box<dyn FnMut(T)>>,
}

pub struct GtkWindow(ApplicationWindow);
pub type WindowId = u32;

impl WindowExt<'_> for GtkWindow {
    type Id = u32;
    fn id(&self) -> Self::Id {
        self.0.get_id()
    }
}

struct EventLoopProxy<I, T>(Arc<Mutex<Sender<Message<I, T>>>>);

impl<I, T> Clone for EventLoopProxy<I, T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[derive(Clone)]
pub struct AppDispatcher<T> {
    proxy: EventLoopProxy<u32, T>,
}

impl<T> ApplicationDispatcher<u32, T> for AppDispatcher<T> {
    fn dispatch_message(&self, message: Message<u32, T>) -> Result<()> {
        self.proxy.0.lock().unwrap().send(message).unwrap();
        Ok(())
    }
}

impl<T> ApplicationExt<'_, T> for Application<T> {
    type Window = GtkWindow;
    type Dispatcher = AppDispatcher<T>;

    fn new() -> Result<Self> {
        let app = GtkApp::new(None, Default::default())?;
        let cancellable: Option<&Cancellable> = None;
        app.register(cancellable)?;
        app.activate();

        let (event_loop_proxy_tx, event_loop_proxy_rx) = channel();

        Ok(Self {
            webviews: HashMap::new(),
            app,
            event_loop_proxy: EventLoopProxy(Arc::new(Mutex::new(event_loop_proxy_tx))),
            event_loop_proxy_rx,
            message_handler: None,
        })
    }

    fn create_window(&self, attributes: AppWindowAttributes) -> Result<Self::Window> {
        //TODO window config (missing transparent)
        let window = ApplicationWindow::new(&self.app);

        window.set_geometry_hints::<ApplicationWindow>(
            None,
            Some(&gdk::Geometry {
                min_width: attributes.min_width.unwrap_or_default() as i32,
                min_height: attributes.min_height.unwrap_or_default() as i32,
                max_width: attributes.max_width.unwrap_or_default() as i32,
                max_height: attributes.max_height.unwrap_or_default() as i32,
                base_width: 0,
                base_height: 0,
                width_inc: 0,
                height_inc: 0,
                min_aspect: 0f64,
                max_aspect: 0f64,
                win_gravity: gdk::Gravity::Center,
            }),
            (if attributes.min_width.is_some() || attributes.min_height.is_some() {
                gdk::WindowHints::MIN_SIZE
            } else {
                gdk::WindowHints::empty()
            }) | (if attributes.max_width.is_some() || attributes.max_height.is_some() {
                gdk::WindowHints::MAX_SIZE
            } else {
                gdk::WindowHints::empty()
            }),
        );

        if attributes.resizable {
            window.set_default_size(attributes.width as i32, attributes.height as i32);
        } else {
            window.set_size_request(attributes.width as i32, attributes.height as i32);
        }

        window.set_resizable(attributes.resizable);
        window.set_title(&attributes.title);
        if attributes.maximized {
            window.maximize();
        }
        window.set_visible(attributes.visible);
        window.set_decorated(attributes.decorations);
        window.set_keep_above(attributes.always_on_top);
        if attributes.fullscreen {
            window.fullscreen();
        }
        if let Some(icon) = attributes.icon {
            let image = image::load_from_memory(&icon.0)?.into_rgba8();
            let (width, height) = image.dimensions();
            let row_stride = image.sample_layout().height_stride;
            let pixbuf = gdk_pixbuf::Pixbuf::from_mut_slice(
                image.into_raw(),
                gdk_pixbuf::Colorspace::Rgb,
                true,
                8,
                width as i32,
                height as i32,
                row_stride as i32,
            );
            window.set_icon(Some(&pixbuf));
        }

        Ok(GtkWindow(window))
    }

    fn create_webview(
        &mut self,
        window: Self::Window,
        attributes: WebViewAttributes,
        callbacks: Option<Vec<Callback>>,
    ) -> Result<()> {
        let mut webview = WebViewBuilder::new(window.0)?;
        for js in attributes.initialization_script {
            webview = webview.initialize_script(&js)?;
        }
        if let Some(cbs) = callbacks {
            for Callback { name, function } in cbs {
                webview = webview.add_callback(&name, function)?;
            }
        }
        webview = match attributes.url {
            Some(url) => webview.load_url(&url)?,
            None => webview,
        };

        let webview = webview.build()?;
        let id = webview.window().get_id();
        self.webviews.insert(id, webview);

        Ok(())
    }

    fn set_message_handler<F: FnMut(T) + 'static>(&mut self, handler: F) {
        self.message_handler.replace(Box::new(handler));
    }

    fn dispatcher(&self) -> Self::Dispatcher {
        AppDispatcher {
            proxy: self.event_loop_proxy.clone(),
        }
    }

    fn run(mut self) {
        let shared_webviews = Arc::new(Mutex::new(self.webviews));
        let shared_webviews_ = shared_webviews.clone();

        {
            let webviews = shared_webviews.lock().unwrap();
            for (id, w) in webviews.iter() {
                let shared_webviews_ = shared_webviews_.clone();
                let id_ = *id;
                w.window().connect_delete_event(move |_window, _event| {
                    shared_webviews_.lock().unwrap().remove(&id_);
                    Inhibit(false)
                });
            }
        }

        loop {
            {
                let webviews = shared_webviews.lock().unwrap();

                if webviews.is_empty() {
                    break;
                }

                for (_, w) in webviews.iter() {
                    let _ = w.evaluate_script();
                }
            }

            while let Ok(message) = self.event_loop_proxy_rx.try_recv() {
                match message {
                    Message::Script(id, script) => {
                        let webviews = shared_webviews.lock().unwrap();
                        if let Some(webview) = webviews.get(&id) {
                            webview.dispatcher().dispatch_script(&script).unwrap();
                        }
                    }
                    Message::Custom(message) => {
                        if let Some(ref mut handler) = self.message_handler {
                            handler(message);
                        }
                    }
                }
            }
            gtk::main_iteration();
        }
    }
}
