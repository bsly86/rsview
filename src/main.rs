use winit:: {
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};
use wgpu::util::DeviceExt;
use bytemuck::*;
use std::sync::Arc;
use cgmath::*;
use std::env;

mod parse;
use parse::{parse_obj, parse_gltf, Mesh};

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
struct Uniforms {
    mvp: [[f32; 4]; 4],
}

struct State<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    rotation: f32,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    model_scale: f32,
    model_center: Vector3<f32>,
    camera_distance: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Vertex {
    position: [f32; 3],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 1] =
        wgpu::vertex_attr_array![0 => Float32x3];

    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }    
}

impl<'a> State<'a> {
    fn create_depth_texture(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> (wgpu::Texture, wgpu::TextureView) {
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        (depth_texture, depth_view)
    }

    fn calculate_model_bounds(vertices: &[[f32; 3]]) -> (Vector3<f32>, Vector3<f32>, Vector3<f32>, f32) {
        let mut min = Vector3::new(f32::MAX, f32::MAX, f32::MAX);
        let mut max = Vector3::new(f32::MIN, f32::MIN, f32::MIN);

        for vertex in vertices {
            min.x = min.x.min(vertex[0]);
            min.y = min.y.min(vertex[1]);
            min.z = min.z.min(vertex[2]);
            
            max.x = max.x.max(vertex[0]);
            max.y = max.y.max(vertex[1]);
            max.z = max.z.max(vertex[2]);
        }

        let center = (min + max) / 2.0;
        let size = max - min;
        let max_dimension = size.x.max(size.y).max(size.z);

        (min, max, center, max_dimension)
    }

    async fn new(window: &'a winit::window::Window, initial_file: Option<String>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::default();
        let surface = unsafe { instance.create_surface(window)}.unwrap();

        let file_to_load = initial_file.unwrap_or_else(|| "test_files/cows".to_string());

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find an appropriate adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: Default::default(),
                    trace: wgpu::Trace::default(),
            },
        )
        .await
        .expect("Failed to create device");

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats[0];
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![surface_format],
            desired_maximum_frame_latency: 2,
        };

        let mesh = load_model(&file_to_load)
            .unwrap_or_else(|e| {
        eprintln!("Failed to load {}: {}", file_to_load, e);
        eprintln!("Loading default model...");
        // Try to load the default model as fallback
        load_model("test_files/cows.obj")
            .expect("Failed to load default model")
        });

        // Calculate model bounds for auto-scaling
        let (_, _, center, max_dimension) = Self::calculate_model_bounds(&mesh.vertices);
        let model_scale = 2.0 / max_dimension; // Scale to fit in a 2-unit cube
        let camera_distance = 3.0; // Adjust this to zoom in/out

        let vertices: Vec<Vertex> = mesh.vertices.into_iter().map(|p| Vertex { position: p }).collect();
        let indices: Vec<u32> = mesh.indices.into_iter().map(|i| i as u32).collect();

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let num_indices = indices.len() as u32;

        // Create uniform buffer
        let uniforms = Uniforms {
            mvp: Matrix4::identity().into(),
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create bind group layout
        let uniform_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }
            ],
            label: Some("uniform_bind_group_layout"),
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                }
            ],
            label: Some("uniform_bind_group"),
        });

        surface.configure(&device, &config);

        // Create depth texture
        let (depth_texture, depth_view) = Self::create_depth_texture(&device, &config);

        let shader = device.create_shader_module(wgpu::include_wgsl!("../shader.wgsl"));

        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            cache: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            vertex_buffer,
            index_buffer,
            num_indices,
            uniform_buffer,
            uniform_bind_group,
            rotation: 0.0,
            depth_texture,
            depth_view,
            model_scale,
            model_center: center,
            camera_distance,
        }
    }

    fn update(&mut self) {
        self.rotation += 0.01;

        let aspect_ratio = self.size.width as f32 / self.size.height as f32;
        
        let model = Matrix4::from_angle_y(Rad(self.rotation)) * 
                    Matrix4::from_scale(self.model_scale) * 
                    Matrix4::from_translation(-self.model_center);
        
        let camera_pos = Point3::new(
            self.camera_distance,
            self.camera_distance * 0.5,
            self.camera_distance
        );
        
        let view = Matrix4::look_at_rh(
            camera_pos,
            Point3::new(0.0, 0.0, 0.0),  // Look at origin
            Vector3::unit_y(),           // Up vector
        );
        
        let proj = perspective(Rad(std::f32::consts::FRAC_PI_4), aspect_ratio, 0.1, 100.0);
        
        let mvp = proj * view * model;

        let uniforms = Uniforms {
            mvp: mvp.into(),
        };
        self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            
            let (depth_texture, depth_view) = Self::create_depth_texture(&self.device, &self.config);
            self.depth_texture = depth_texture;
            self.depth_view = depth_view;
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.update();

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder")
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

fn load_model(path: &str) -> Result<Mesh, String> {
    let path_lower = path.to_lowercase();
    if path_lower.ends_with(".obj") {
        parse_obj(path)
    } else if path_lower.ends_with(".gltf") {
        parse_gltf(path)
    } else {
        Err("Unsupported file format, only .obj and .gltf (NOT GLB) files are supported.".to_string())
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let initial_file = if args.len() > 1 {
        Some(args[1].clone())
    } else {
        None
    };
    
    pollster::block_on(run(initial_file));
}

async fn run(initial_file: Option<String>) {
    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("rsview - Model Viewer")
            .build(&event_loop)
            .unwrap()
    );

    let mut state = State::new(&window, initial_file).await;
    let window_clone = window.clone();

    event_loop.run(move |event, event_loop_window_target| {
        event_loop_window_target.set_control_flow(ControlFlow::Poll);
    
        match event {
            Event::WindowEvent { event, window_id } if window_id == window_clone.id() => match event {
                WindowEvent::CloseRequested => {
                    let _ = event_loop_window_target.exit();
                }
                WindowEvent::RedrawRequested => {
                    state.render().unwrap();
                }
                WindowEvent::Resized(physical_size) => {
                    state.resize(physical_size);
                }
                _ => {}
            },
            Event::AboutToWait => {
                window_clone.request_redraw();
            }
            _ => {}
        }
    });    
}