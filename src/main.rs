mod pipeline;
use std::error::Error;
use std::path::PathBuf;
use structopt::StructOpt;

use winit::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::unix::WindowExtUnix,
    window::{Window, WindowBuilder},
};

use std::time::Duration;
use std::time::Instant;
use wayland_client::{protocol::wl_surface::WlSurface, sys::client::wl_proxy, Proxy};
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

fn put_to_background(window: &Window) {
    let sfc = match window.wayland_surface() {
        Some(wayland_surface) => unsafe {
            Proxy::<WlSurface>::from_c_ptr(wayland_surface as *mut wl_proxy)
        },
        None => return,
    };
}

fn main() -> Result<(), Box<dyn Error>> {
    let opt = Opt::from_args();
    println!("{:?}", opt);

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    put_to_background(&window);

    let mut pipeline = pipeline::init(&window, &opt.frame_path)?;

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
            pipeline.go_to_next_frame();
            window.request_redraw();
        }

        Event::WindowEvent {
            event: WindowEvent::RedrawRequested,
            ..
        } => {
            pipeline.render();
            *control_flow = ControlFlow::WaitUntil(next_update)
        }

        Event::WindowEvent {
            event: WindowEvent::Resized(size),
            ..
        } => {
            let physical = size.to_physical(window.hidpi_factor());
            pipeline.resize(physical.width.round() as u32, physical.height.round() as u32);
        }

        _ => *control_flow = ControlFlow::Wait,
    });
}
