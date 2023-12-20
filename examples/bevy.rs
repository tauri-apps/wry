use bevy::{prelude::*, window::RawHandleWrapper};
use wry::WebViewBuilder;

fn main() {
  #[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
  ))]
  {
    use gtk::prelude::DisplayExtManual;

    gtk::init().unwrap();
    if gtk::gdk::Display::default().unwrap().backend().is_wayland() {
      panic!("This example doesn't support wayland!");
    }

    // we need to ignore this error here otherwise it will be catched by winit and will make the example crash
    winit::platform::x11::register_xlib_error_hook(Box::new(|_display, error| {
      let error = error as *mut x11_dl::xlib::XErrorEvent;
      (unsafe { (*error).error_code }) == 170
    }));
  }

  let mut app = App::new();
  app
    .add_plugins(DefaultPlugins)
    .add_systems(Startup, setup)
    .add_systems(Startup, setup_webview)
    .init_non_send_resource::<WebViewStore>();
  #[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
  ))]
  app.add_systems(Update, advance_gtk);
  app.run();
}

/// set up a simple 3D scene
fn setup(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<StandardMaterial>>,
) {
  // circular base
  commands.spawn(PbrBundle {
    mesh: meshes.add(shape::Circle::new(4.0).into()),
    material: materials.add(Color::WHITE.into()),
    transform: Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ..default()
  });
  // cube
  commands.spawn(PbrBundle {
    mesh: meshes.add(Mesh::from(shape::Cube { size: 1.0 })),
    material: materials.add(Color::rgb_u8(124, 144, 255).into()),
    transform: Transform::from_xyz(0.0, 0.5, 0.0),
    ..default()
  });
  // light
  commands.spawn(PointLightBundle {
    point_light: PointLight {
      intensity: 1500.0,
      shadows_enabled: true,
      ..default()
    },
    transform: Transform::from_xyz(4.0, 8.0, 4.0),
    ..default()
  });
  // camera
  commands.spawn(Camera3dBundle {
    transform: Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ..default()
  });
}

#[derive(Default)]
struct WebViewStore {
  webviews: Vec<wry::WebView>,
}

#[cfg(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd",
))]
fn advance_gtk(_: NonSend<WebViewStore>) {
  while gtk::events_pending() {
    gtk::main_iteration_do(true);
  }
}

fn setup_webview(window: Query<&RawHandleWrapper>, mut store: NonSendMut<WebViewStore>) {
  let handle = unsafe { window.single().get_handle() };
  let webview = WebViewBuilder::new_as_child(&handle)
    .with_bounds(wry::Rect {
      x: 100,
      y: 100,
      width: 200,
      height: 200,
    })
    .with_transparent(true)
    .with_url("https://tauri.app")
    .unwrap()
    .build()
    .unwrap();

  store.as_mut().webviews.push(webview);
}
