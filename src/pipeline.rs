use crate::{
    Opt,
    platform::CustomEvent
};
use image::{
    gif::GifDecoder,
    GenericImageView,
    RgbaImage,
    AnimationDecoder,
    ImageDecoder
};
use std::{
    error::Error,
    fs,
    fs::File,
    path::{Path, PathBuf},
    time::SystemTime
};
use winit::{
    dpi::{PhysicalSize, LogicalSize},
    event_loop::EventLoop,
    monitor::MonitorHandle,
    platform::unix::WindowBuilderExtUnix,
    window::{Window, WindowBuilder, WindowId},
};
use log::info;
use rayon::prelude::*;

pub struct Pipeline {
    position: f32,
    total_frame: u32,
    increment: f32,
    use_timestamp: bool,
    started_at: SystemTime,
    device: wgpu::Device,
    queue: wgpu::Queue,
    bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
    uniform: Vec<u8>,
    uniform_buf: wgpu::Buffer,
}

fn create_shader_module(
    device: &wgpu::Device,
    code: String,
    shader_type: shaderc::ShaderKind,
) -> Result<wgpu::ShaderModule, Box<dyn Error>> {
    let mut compiler = shaderc::Compiler::new().unwrap();
    let options = shaderc::CompileOptions::new().unwrap();

    let binary_result = compiler
        .compile_into_spirv(&code, shader_type, "file.glsl", "main", Some(&options))
        .unwrap();

    Ok(device.create_shader_module(binary_result.as_binary()))
}

fn create_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        lod_min_clamp: -100.0,
        lod_max_clamp: 100.0,
        compare_function: wgpu::CompareFunction::Always,
    })
}

fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        bindings: &[
            wgpu::BindGroupLayoutBinding {
                binding: 0,
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::Sampler,
            },
            wgpu::BindGroupLayoutBinding {
                binding: 1,
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::SampledTexture {
                    multisampled: false,
                    dimension: wgpu::TextureViewDimension::D2Array,
                },
            },
            wgpu::BindGroupLayoutBinding {
                binding: 2,
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::UniformBuffer { dynamic: false },
            },
        ],
    })
}

fn get_shaders(
    device: &wgpu::Device,
    custom_fragment: &Option<PathBuf>,
) -> Result<(wgpu::ShaderModule, wgpu::ShaderModule), Box<dyn Error>> {
    let frag_code = match custom_fragment {
        Some(path) => fs::read_to_string(path).expect("Cannot find custom shader"),
        None => String::from(include_str!("shaders/frag.glsl")),
    };

    let frag = create_shader_module(&device, frag_code, shaderc::ShaderKind::Fragment)?;

    let vert = create_shader_module(
        &device,
        String::from(include_str!("shaders/vert.glsl")),
        shaderc::ShaderKind::Vertex,
    )?;

    Ok((frag, vert))
}

fn create_swap_chain(
    device: &wgpu::Device,
    surface: &wgpu::Surface,
    size: PhysicalSize,
) -> wgpu::SwapChain {
    let sc_desc = wgpu::SwapChainDescriptor {
        usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        width: size.width.round() as u32,
        height: size.height.round() as u32,
        present_mode: wgpu::PresentMode::Vsync,
    };

    device.create_swap_chain(&surface, &sc_desc)
}

fn create_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    total_frame: u32,
) -> (wgpu::Extent3d, wgpu::Texture) {
    let extent = wgpu::Extent3d {
        width,
        height,
        depth: 1,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        size: extent,
        array_layer_count: total_frame,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
    });

    (extent, texture)
}

fn load_textures(
    frames_path: &Path,
    device: &wgpu::Device,
    queue: &mut wgpu::Queue,
) -> Result<(wgpu::TextureView, u32), Box<dyn Error>> {
    let pathmd = std::fs::metadata(frames_path).unwrap();

    if pathmd.is_dir() {
        load_textures_from_path(frames_path, device, queue)
    } else {
        load_textures_from_gif(frames_path, device, queue)
    }
}

fn load_textures_in_gpu(
    frames: &Vec<&RgbaImage>,
    total_frame: usize,
    width: u32,
    height: u32,
    device: &wgpu::Device,
    queue: &mut wgpu::Queue,
) -> (wgpu::Texture, usize) {
    info!("Loading frames");

    let (texture_extent, texture) = create_texture(&device, width, height, total_frame as u32);

    let commands = frames.par_iter().enumerate().map(|(index, frame)| {
        let mut init_encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });

        let temp_buf = device
            .create_buffer_mapped(frame.len(), wgpu::BufferUsage::COPY_SRC)
            .fill_from_slice(&frame);

        init_encoder.copy_buffer_to_texture(
            wgpu::BufferCopyView {
                buffer: &temp_buf,
                offset: 0,
                row_pitch: 4 * width,
                image_height: height,
            },
            wgpu::TextureCopyView {
                texture: &texture,
                mip_level: 0,
                array_layer: index as u32,
                origin: wgpu::Origin3d {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
            },
            texture_extent,
        );
        Ok(init_encoder.finish())
    });
    let commands_vec: Result<Vec<_>, Box<dyn Error + Send>> = commands.collect();
    queue.submit(&commands_vec.unwrap()); // FIXME : Remove unwrap
    info!("Finished loading frames");

    return (texture, total_frame);
}

fn load_textures_from_gif(
    gif_path: &Path,
    device: &wgpu::Device,
    queue: &mut wgpu::Queue,
) -> Result<(wgpu::TextureView, u32), Box<dyn Error>> {
    let file_in = File::open(gif_path)?;
    let decoder = GifDecoder::new(file_in).unwrap();
    let (width, height) = decoder.dimensions();
    let frames = decoder
        .into_frames()
        .collect_frames()
        .expect("error decoding gif");
    let rgba_frames: Vec<_> = frames.par_iter().map(|frame| frame.buffer()).collect();

    let (texture, total_frame) =
        load_textures_in_gpu(&rgba_frames, frames.len(), width, height, device, queue);

    Ok((texture.create_default_view(), total_frame as u32))
}

fn load_textures_from_path(
    frames_path: &Path,
    device: &wgpu::Device,
    queue: &mut wgpu::Queue,
) -> Result<(wgpu::TextureView, u32), Box<dyn Error>> {
    let dir: Result<Vec<_>, Box<dyn Error>> = std::fs::read_dir(frames_path)?
        .map(|p| Ok(p?.path()))
        .collect();
    let mut dir = dir?;
    dir.sort_by(|a, b| {
        natord::compare(
            a.file_name().unwrap().to_str().unwrap(),
            b.file_name().unwrap().to_str().unwrap(),
        )
    });

    let img = image::open(&dir[0])?;
    let (width, height) = img.dimensions();

    let rgba_frames: Vec<_> = dir
        .par_iter()
        .map(|entry| image::open(entry).unwrap().to_rgba())
        .collect();

    let rgba_frames: Vec<_> = rgba_frames.par_iter().map(|i| i).collect();

    let (texture, total_frame) =
        load_textures_in_gpu(&rgba_frames, dir.len(), width, height, device, queue);

    Ok((texture.create_default_view(), total_frame as u32))
}

fn create_pipeline(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    custom_fragment: &Option<PathBuf>,
) -> wgpu::RenderPipeline {
    let (frag, vert) = get_shaders(&device, custom_fragment).unwrap();

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        bind_group_layouts: &[&bind_group_layout],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        layout: &pipeline_layout,
        vertex_stage: wgpu::ProgrammableStageDescriptor {
            module: &vert,
            entry_point: "main",
        },
        fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
            module: &frag,
            entry_point: "main",
        }),
        rasterization_state: Some(wgpu::RasterizationStateDescriptor {
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: wgpu::CullMode::None,
            depth_bias: 0,
            depth_bias_slope_scale: 0.0,
            depth_bias_clamp: 0.0,
        }),
        primitive_topology: wgpu::PrimitiveTopology::TriangleList,
        color_states: &[wgpu::ColorStateDescriptor {
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            color_blend: wgpu::BlendDescriptor::REPLACE,
            alpha_blend: wgpu::BlendDescriptor::REPLACE,
            write_mask: wgpu::ColorWrite::ALL,
        }],
        depth_stencil_state: None,
        index_format: wgpu::IndexFormat::Uint16,
        vertex_buffers: &[],
        sample_count: 1,
        sample_mask: !0,
        alpha_to_coverage_enabled: false,
    })
}

impl Pipeline {
    pub fn new(options: &Opt) -> Result<Self, Box<dyn Error>> {
        let adapter = wgpu::Adapter::request(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            backends: wgpu::BackendBit::PRIMARY,
        })
        .unwrap(); // FIXME: Should use Result

        let (device, mut queue) = adapter.request_device(&wgpu::DeviceDescriptor {
            extensions: wgpu::Extensions {
                anisotropic_filtering: false,
            },
            limits: wgpu::Limits::default(),
        });

        let (texture_view, total_frame) = load_textures(&options.frame_path, &device, &mut queue)?;
        let sampler = create_sampler(&device);
        let bind_group_layout = create_bind_group_layout(&device);
        let render_pipeline =
            create_pipeline(&device, &bind_group_layout, &options.custom_fragment);

        let uniform = [total_frame.to_ne_bytes(), [0u8, 0, 0, 0]].concat();

        let uniform_buf = device
            .create_buffer_mapped(
                uniform.len(),
                wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            )
            .fill_from_slice(&uniform);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            bindings: &[
                wgpu::Binding {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::Binding {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::Binding {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer {
                        buffer: &uniform_buf,
                        range: 0..8,
                    },
                },
            ],
        });

        let pipeline = Pipeline {
            position: 0.0,
            total_frame,
            increment: options.fps as f32 / options.rendered_fps as f32,
            use_timestamp: options.custom_fragment.is_some(),
            started_at: SystemTime::now(),
            device,
            queue,
            bind_group,
            render_pipeline,
            uniform,
            uniform_buf,
        };

        Ok(pipeline)
    }

    pub fn go_to_next_frame(&mut self) {
        self.position = if self.use_timestamp {
            SystemTime::now()
                .duration_since(self.started_at)
                .expect("Time went backwards")
                .as_secs_f32()
        } else {
            (self.position + self.increment) % self.total_frame as f32
        };
    }

    pub fn update_shader_globals(&mut self) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });

        let uniform = [self.total_frame.to_ne_bytes(), self.position.to_le_bytes()].concat();

        let temp_buf = self
            .device
            .create_buffer_mapped(self.uniform.len(), wgpu::BufferUsage::COPY_SRC)
            .fill_from_slice(&uniform);

        encoder.copy_buffer_to_buffer(
            &temp_buf,
            0,
            &self.uniform_buf,
            0,
            uniform.len() as wgpu::BufferAddress,
        );
        self.queue.submit(&[encoder.finish()]);
    }
}

pub struct PipelineWindows {
    windows: Vec<PipelineWindow>,
}

impl PipelineWindows {
    pub fn new(event_loop: &EventLoop<CustomEvent>, pipeline: &Pipeline) -> Self {
        let windows = event_loop
            .available_monitors()
            .map(|monitor| PipelineWindow::new(&pipeline.device, &event_loop, &monitor))
            .collect();

        Self { windows }
    }

    pub fn render(&mut self, pipeline: &mut Pipeline) {
        self.windows.iter_mut().for_each(|w| w.render(pipeline));
    }

    pub fn find_mut(&mut self, window_id: WindowId) -> Option<&mut PipelineWindow> {
        self.windows.iter_mut().find(|w| w.window.id() == window_id)
    }

    pub fn close(&mut self, window_id: WindowId) {
        let (i, _) = self
            .windows
            .iter()
            .enumerate()
            .find(|(_, w)| w.window.id() == window_id)
            .unwrap();
        self.windows.swap_remove(i);
    }

    pub fn is_empty(&self) -> bool {
        return self.windows.is_empty();
    }

    pub fn request_redraw(&self) {
        self.windows.iter().for_each(|w| w.window.request_redraw());
    }
}

pub struct PipelineWindow {
    pub(crate) window: Window,
    swap_chain: wgpu::SwapChain,
    surface: wgpu::Surface,
}

impl PipelineWindow {
    pub fn new(
        device: &wgpu::Device,
        event_loop: &EventLoop<crate::platform::CustomEvent>,
        monitor: &MonitorHandle,
    ) -> Self {
        let window = WindowBuilder::new()
            .with_shell(false)
            .disable_input_region(true)
            .build(&event_loop)
            .unwrap();
        let surface = wgpu::Surface::create(&window);
        let swap_chain = create_swap_chain(
            device,
            &surface,
            window.inner_size().to_physical(window.hidpi_factor()),
        );

        let pipeline_window = Self {
            window,
            swap_chain,
            surface,
        };

        crate::platform::put_to_background(monitor, event_loop, &pipeline_window);
        pipeline_window
    }

    pub fn resize(&mut self, size: LogicalSize, pipeline: &Pipeline) {
        let size = size.to_physical(self.window.hidpi_factor());
        self.swap_chain = create_swap_chain(&pipeline.device, &self.surface, size);
        self.window.request_redraw();
    }

    fn render(&mut self, pipeline: &mut Pipeline) {
        let frame = self.swap_chain.get_next_texture();
        let mut encoder = pipeline
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &frame.view,
                    resolve_target: None,
                    load_op: wgpu::LoadOp::Clear,
                    store_op: wgpu::StoreOp::Store,
                    clear_color: wgpu::Color::BLACK,
                }],
                depth_stencil_attachment: None,
            });
            rpass.set_pipeline(&pipeline.render_pipeline);
            rpass.set_bind_group(0, &pipeline.bind_group, &[]);
            rpass.draw(0..6, 0..1);
        }

        pipeline.queue.submit(&[encoder.finish()]);
    }
}
