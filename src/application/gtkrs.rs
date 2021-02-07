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
use gtk::{Application as GtkApp, ApplicationWindow, ApplicationWindowExt};

pub struct Application<T: Clone> {
    webviews: HashMap<u32, WebView>,
    app: GtkApp,
    event_loop_proxy: EventLoopProxy<u32, T>,
    event_loop_proxy_rx: Receiver<Message<u32, T>>,
    message_handler: Option<Box<dyn FnMut(T)>>,
}

pub struct GtkWindow(ApplicationWindow);

impl WindowExt<'_> for GtkWindow {
    type Id = u32;
    fn id(&self) -> Self::Id {
        self.0.get_id()
    }
}

#[derive(Clone)]
struct EventLoopProxy<I, T: Clone>(Arc<Mutex<Sender<Message<I, T>>>>);

pub struct AppDispatcher<I, T: Clone> {
    proxy: EventLoopProxy<I, T>,
}

impl<I, T: Clone> ApplicationDispatcher<I, T> for AppDispatcher<I, T> {
    fn dispatch_message(&self, message: Message<I, T>) -> Result<()> {
        self.proxy.0.lock().unwrap().send(message).unwrap();
        Ok(())
    }
}

impl<T: Clone> ApplicationExt<'_, T> for Application<T> {
    type Window = GtkWindow;
    type Dispatcher = AppDispatcher<u32, T>;

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

    fn create_window(&self, _attributes: AppWindowAttributes) -> Result<Self::Window> {
        //TODO window config
        let window = ApplicationWindow::new(&self.app);
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
        loop {
            for (_, w) in self.webviews.iter() {
                let _ = w.evaluate_script();
            }
            while let Ok(message) = self.event_loop_proxy_rx.try_recv() {
                match message {
                    Message::Script(id, script) => {
                        if let Some(webview) = self.webviews.get(&id) {
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
