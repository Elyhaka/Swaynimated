use wayland_client::{
    protocol::{
        wl_display::WlDisplay,
        wl_output::WlOutput,
        wl_registry::{self, WlRegistry},
        wl_surface::WlSurface,
    },
    sys::client::wl_proxy,
    GlobalManager, Interface, Proxy,
};
use wayland_protocols::wlr::unstable::layer_shell::v1::client::{
    zwlr_layer_shell_v1, zwlr_layer_surface_v1,
};

use crate::pipeline::PipelineWindow;
use std::cell::RefCell;
use std::rc::Rc;

use winit::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::unix::{MonitorHandleExtUnix, WindowBuilderExtUnix, WindowExtUnix},
    window::{Window, WindowBuilder},
};
use winit::dpi::PhysicalSize;

pub fn put_to_background(
    monitor_handle: &winit::monitor::MonitorHandle,
    pipeline_window: Rc<RefCell<PipelineWindow>>,
) {
    let sfc: WlSurface = match pipeline_window.borrow().window.wayland_surface() {
        Some(wayland_surface) => unsafe {
            Proxy::<WlSurface>::from_c_ptr(wayland_surface as *mut wl_proxy)
        },
        None => return,
    }
    .into();

    let display_ptr = pipeline_window.borrow().window.wayland_display().unwrap() as _;
    let display: WlDisplay = unsafe { Proxy::from_c_ptr(display_ptr) }.into();

    let output_ptr = monitor_handle.wayland_output().unwrap() as _;
    let output: WlOutput = unsafe { Proxy::from_c_ptr(output_ptr) }.into();

    let manager = GlobalManager::new(&display);

    unsafe { (wayland_sys::client::WAYLAND_CLIENT_HANDLE.wl_display_roundtrip)(display_ptr as _) };

    let shell: zwlr_layer_shell_v1::ZwlrLayerShellV1 = manager
        .instantiate_exact(1, |p| p.implement_dummy())
        .unwrap();

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
                                println!("{:?}", (serial, width, height));
                                pipeline_window.borrow_mut().resize(PhysicalSize::new(width as f64, height as f64));
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
