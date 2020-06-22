use wayland_client::{
    protocol::{wl_display::WlDisplay, wl_output::WlOutput, wl_surface::WlSurface},
    sys::client::wl_proxy,
    GlobalManager, Proxy,
};
use wayland_protocols::wlr::unstable::layer_shell::v1::client::{
    zwlr_layer_shell_v1, zwlr_layer_surface_v1,
};

use crate::pipeline::PipelineWindow;

use winit::dpi::LogicalSize;
use winit::window::WindowId;
use winit::{
    event_loop::EventLoop,
    platform::unix::{MonitorHandleExtUnix, WindowExtUnix},
};

#[derive(Debug)]
pub enum CustomEvent {
    WindowResized {
        window_id: WindowId,
        new_size: LogicalSize,
    },
}

pub fn put_to_background(
    monitor_handle: &winit::monitor::MonitorHandle,
    event_loop: &EventLoop<CustomEvent>,
    pipeline_window: &PipelineWindow,
) {
    let sfc: WlSurface = match pipeline_window.window.wayland_surface() {
        Some(wayland_surface) => unsafe {
            Proxy::<WlSurface>::from_c_ptr(wayland_surface as *mut wl_proxy)
        },
        None => return,
    }
    .into();

    let display_ptr = pipeline_window.window.wayland_display().unwrap() as _;
    let display: WlDisplay = unsafe { Proxy::from_c_ptr(display_ptr) }.into();

    let output_ptr = monitor_handle.wayland_output().unwrap() as _;
    let output: WlOutput = unsafe { Proxy::from_c_ptr(output_ptr) }.into();

    let manager = GlobalManager::new(&display);

    unsafe { (wayland_sys::client::WAYLAND_CLIENT_HANDLE.wl_display_roundtrip)(display_ptr as _) };

    let shell: zwlr_layer_shell_v1::ZwlrLayerShellV1 = manager
        .instantiate_exact(1, |p| p.implement_dummy())
        .unwrap();

    let window_id = pipeline_window.window.id();
    let event_proxy = event_loop.create_proxy();

    let layer_surface = shell
        .get_layer_surface(
            &sfc,
            Some(&output),
            zwlr_layer_shell_v1::Layer::Background,
            "wallpaper".into(),
            move |p| {
                p.implement_closure(
                    move |e, layer_surface| {
                        match e {
                            zwlr_layer_surface_v1::Event::Configure {
                                serial,
                                width,
                                height,
                            } => {
                                let new_size = LogicalSize::new(width as f64, height as f64);
                                event_proxy
                                    .send_event(CustomEvent::WindowResized {
                                        window_id,
                                        new_size,
                                    })
                                    .unwrap();
                                layer_surface.ack_configure(serial);
                            }
                            zwlr_layer_surface_v1::Event::Closed => println!("CLOSED"),
                            _ => {}
                        }
                        ()
                    },
                    (),
                )
            },
        )
        .unwrap();

    layer_surface.set_anchor(zwlr_layer_surface_v1::Anchor::all());
    layer_surface.set_exclusive_zone(-1);

    sfc.commit();
    unsafe { (wayland_sys::client::WAYLAND_CLIENT_HANDLE.wl_display_roundtrip)(display_ptr as _) };
}
