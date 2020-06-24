#![deny(clippy::all, clippy::pedantic)]

mod pipeline;
mod platform;

use std::error::Error;
use std::path::PathBuf;
use structopt::StructOpt;

use winit::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

use std::time::Duration;
use std::time::Instant;

use crate::pipeline::{Pipeline, PipelineWindows};
use crate::platform::CustomEvent;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "swaynimated",
    about = "Animating your wl-roots compositor since 2019"
)]
pub struct Opt {
    #[structopt(short, long, help = "Enabled debug (verbose) output")]
    debug: bool,

    #[structopt(
        short = "f",
        long = "fps",
        default_value = "5",
        help = "The number of frame per second of the animation."
    )]
    fps: u32,

    #[structopt(
        short = "r",
        long = "rendered_fps",
        default_value = "25",
        help = "The number of frame rendered (interpolate with mix between frames). To disable put the same as the number of frame."
    )]
    rendered_fps: u32,

    #[structopt(
        short = "g",
        long = "custom_fragment",
        help = "Custom GLSL fragment to use instead of the default one."
    )]
    custom_fragment: Option<PathBuf>,

    #[structopt(parse(from_os_str))]
    frame_path: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    let opt = Opt::from_args();

    let event_loop = EventLoop::with_user_event();
    let mut pipeline = Pipeline::new(&opt)?;
    let mut windows = PipelineWindows::new(&event_loop, &pipeline);

    let timer_length = Duration::new(0, 1_000_000_000 / opt.rendered_fps);
    let mut next_update = Instant::now();

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            window_id,
        } => {
            windows.close(window_id);
            if windows.is_empty() {
                *control_flow = ControlFlow::Exit
            }
        }

        Event::WindowEvent {
            event: WindowEvent::Resized(new_size),
            window_id,
        }
        | Event::UserEvent(CustomEvent::WindowResized {
            new_size,
            window_id,
        }) => {
            windows
                .find_mut(window_id)
                .unwrap()
                .resize(new_size, &pipeline);
        }

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
            windows.request_redraw();
        }

        Event::WindowEvent {
            event: WindowEvent::RedrawRequested,
            ..
        } => {
            pipeline.update_shader_globals();
            windows.render(&mut pipeline);
            *control_flow = ControlFlow::WaitUntil(next_update)
        }

        _ => *control_flow = ControlFlow::WaitUntil(next_update),
    });
}
