mod pipeline;
use std::error::Error;
use std::path::PathBuf;
use structopt::StructOpt;

use winit::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::unix::WindowExtUnix, platform::unix::WindowBuilderExtUnix,
    window::{Window, WindowBuilder},
};

use std::time::Duration;
use std::time::Instant;
use wayland_client::{protocol::{wl_display::WlDisplay, wl_registry::{self, WlRegistry}, wl_surface::WlSurface}, sys::client::wl_proxy, GlobalManager, Proxy, Interface};
use wayland_protocols::wlr::unstable::layer_shell::v1::client::{
    zwlr_layer_shell_v1, zwlr_layer_surface_v1,
};

#[derive(Debug, StructOpt)]
#[structopt(name = "swaynimated", about = "Animating your wlroots compositor since 2019")]
struct Opt {
    /// Activate debug mode
    #[structopt(short, long)]
    debug: bool,

    /// Number of FPS for the running animation
    #[structopt(short = "f", long = "fps", default_value = "5")]
    fps: u32,

    /// Input folder (where are the frames stored)
    #[structopt(parse(from_os_str))]
    frame_path: PathBuf,
}
use wayland_protocols::xdg_shell::client::xdg_wm_base::XdgWmBase;
use std::cell::RefCell;
use std::rc::Rc;
use crate::pipeline::Pipeline;

const fps: u32 = 60;
const filePath: &str =
    "/home/adrien/Projects/Lenovo-NixOS-Configuration/user-configuration/dotfiles/assets/frames/";

fn put_to_background(window: &Window, pipeline: Rc<RefCell<Pipeline>>) {
    let sfc: WlSurface = match window.wayland_surface() {
        Some(wayland_surface) => unsafe {
            Proxy::<WlSurface>::from_c_ptr(wayland_surface as *mut wl_proxy)
        },
        None => return,
    }
    .into();

    let display_ptr = window.wayland_display().unwrap() as _;
    let display: WlDisplay = unsafe { Proxy::from_c_ptr(display_ptr) }.into();

    let manager = GlobalManager::new(&display);

    unsafe { (wayland_sys::client::WAYLAND_CLIENT_HANDLE.wl_display_roundtrip)(display_ptr as _) };

    let shell: zwlr_layer_shell_v1::ZwlrLayerShellV1 = manager
        .instantiate_range(0, 42, |p| {
            p.implement_dummy()
        })
        .unwrap();

    let layer_surface = shell.get_layer_surface(
        &sfc,
        None,
        zwlr_layer_shell_v1::Layer::Background,
        "wallpaper".into(),
        move |p| p.implement_closure(move |e, layer_surface| {
            match e {
                zwlr_layer_surface_v1::Event::Configure { serial, width, height } => {
                    println!("{:?}", (serial, width, height));
                    pipeline.borrow_mut().resize(width, height);
                    layer_surface.ack_configure(serial);
                }
                zwlr_layer_surface_v1::Event::Closed => println!("CLOSED"),
                _ => {}
            }
            ()
        }, ()),
    ).unwrap();

    layer_surface.set_anchor(zwlr_layer_surface_v1::Anchor::all());
    layer_surface.set_exclusive_zone(-1);

    sfc.commit();
    unsafe { (wayland_sys::client::WAYLAND_CLIENT_HANDLE.wl_display_roundtrip)(display_ptr as _) };
}

fn main() -> Result<(), Box<dyn Error>> {
    let opt = Opt::from_args();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().with_shell(false).build(&event_loop).unwrap();
    let mut pipeline = Rc::new(RefCell::new(pipeline::init(&window, &opt.frame_path)?));
    put_to_background(&window, pipeline.clone());

    let timer_length = Duration::new(0, 1_000_000_000 / opt.fps);
    let mut next_update = Instant::now();

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            window_id,
        } if window_id == window.id() => *control_flow = ControlFlow::Exit,

        Event::EventsCleared => {
            *control_flow = ControlFlow::WaitUntil(next_update);
        }

        Event::NewEvents(StartCause::WaitCancelled {
            requested_resume, ..
        }) => {
            next_update = requested_resume.unwrap_or_else(|| Instant::now() + timer_length);
            *control_flow = ControlFlow::WaitUntil(next_update);
        }

        Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
            next_update = Instant::now() + timer_length;
            *control_flow = ControlFlow::WaitUntil(next_update);
            pipeline.borrow_mut().go_to_next_frame();
            window.request_redraw();
        }

        Event::WindowEvent {
            event: WindowEvent::RedrawRequested,
            ..
        } => {
            pipeline.borrow_mut().render();
            *control_flow = ControlFlow::WaitUntil(next_update)
        }

        Event::WindowEvent {
            event: WindowEvent::Resized(size),
            ..
        } => {
            let physical = size.to_physical(window.hidpi_factor());
            pipeline.borrow_mut().resize(physical.width.round() as u32, physical.height.round() as u32);
        }

        _ => *control_flow = ControlFlow::Wait,
    });
}
