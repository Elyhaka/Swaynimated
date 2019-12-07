use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use image::GenericImageView;

const fps: u64 = 5;
const filePath: &str = "/home/ely/.assets/frames/";

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    let size = window.inner_size().to_physical(window.hidpi_factor());

    let surface = wgpu::Surface::create(&window);

    let adapter = wgpu::Adapter::request(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::Default,
        backends: wgpu::BackendBit::PRIMARY,
    })
    .unwrap();

    let (device, mut queue) = adapter.request_device(&wgpu::DeviceDescriptor {
        extensions: wgpu::Extensions {
            anisotropic_filtering: false,
        },
        limits: wgpu::Limits::default(),
    });

    let mut sc_desc = wgpu::SwapChainDescriptor {
        usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        width: size.width.round() as u32,
        height: size.height.round() as u32,
        present_mode: wgpu::PresentMode::Vsync,
    };

    let mut swap_chain = device.create_swap_chain(&surface, &sc_desc);

    let frag_glsl = include_str!("shaders/frag.glsl");
    let vert_glsl = include_str!("shaders/vert.glsl");

    let frag_spriv =
        glsl_to_spirv::compile(frag_glsl, glsl_to_spirv::ShaderType::Fragment).unwrap();
    let vert_spriv = glsl_to_spirv::compile(vert_glsl, glsl_to_spirv::ShaderType::Vertex).unwrap();

    let frag = wgpu::read_spirv(frag_spriv).unwrap();
    let vert = wgpu::read_spirv(vert_spriv).unwrap();

    let frag_mod = device.create_shader_module(&frag);
    let vert_mod = device.create_shader_module(&vert);

    let dir: Vec<_> = std::fs::read_dir(&std::path::Path::new(filePath))
        .unwrap()
        .map(|p| p.unwrap().path())
        .collect();
    let total_frame = dir.len();

    let img = image::open(&dir[0]).unwrap().to_rgba();
    let (width, height) = img.dimensions();
    let texture_extent = wgpu::Extent3d {
        width,
        height,
        depth: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        size: texture_extent,
        array_layer_count: total_frame as u32,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
    });
    let texture_view = texture.create_default_view();

    for (index, entry) in dir.iter().enumerate() {
        let img = image::open(entry).unwrap().to_rgba();

        let mut init_encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });

        let temp_buf = device
            .create_buffer_mapped(img.len(), wgpu::BufferUsage::COPY_SRC)
            .fill_from_slice(&img);

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
        queue.submit(&[init_encoder.finish()]);
    }

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        lod_min_clamp: -100.0,
        lod_max_clamp: 100.0,
        compare_function: wgpu::CompareFunction::Always,
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
    });

    let mut uniform = [0u8, 0, 0, 0];
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
                    range: 0..4,
                },
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        bind_group_layouts: &[&bind_group_layout],
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        layout: &pipeline_layout,
        vertex_stage: wgpu::ProgrammableStageDescriptor {
            module: &vert_mod,
            entry_point: "main",
        },
        fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
            module: &frag_mod,
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
    });

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            window_id,
        } if window_id == window.id() => *control_flow = ControlFlow::Exit,
        Event::EventsCleared => {
            let frame = swap_chain.get_next_texture();

            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });

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
                rpass.set_pipeline(&render_pipeline);
                rpass.set_bind_group(0, &bind_group, &[]);
                rpass.draw(0..6, 0..1);
            }

            queue.submit(&[encoder.finish()]);
            std::thread::sleep(std::time::Duration::from_millis(1000 / fps));
        }
        Event::WindowEvent {
            event: WindowEvent::Resized(size),
            ..
        } => {
            let physical = size.to_physical(window.hidpi_factor());
            sc_desc.width = size.width.round() as u32;
            sc_desc.height = size.height.round() as u32;
            swap_chain = device.create_swap_chain(&surface, &sc_desc);
        }
        _ => *control_flow = ControlFlow::Wait,
    });
}
