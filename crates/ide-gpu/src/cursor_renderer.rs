//! Blinking text cursor renderer.
//!
//! A deliberately tiny sibling to `TextRenderer`: one solid-color quad, no
//! texture, no bind group. Its vertex buffer is dynamic (COPY_DST) since
//! the cursor moves every frame (or blinks on/off) - unlike the glyph
//! atlas, which is only rebuilt when the text itself changes.

use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CursorVertex {
    position: [f32; 2], // NDC space
}

pub struct CursorRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
}

impl CursorRenderer {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cursor-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("cursor.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("cursor-pipeline-layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("cursor-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<CursorVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: wgpu::VertexFormat::Float32x2,
                    }],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cursor-vertex-buffer"),
            size: (4 * std::mem::size_of::<CursorVertex>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cursor-index-buffer"),
            contents: bytemuck::cast_slice(&[0u16, 1, 2, 0, 2, 3]),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
        }
    }

    pub fn update(
        &self,
        queue: &wgpu::Queue,
        rect_px: (f32, f32, f32, f32),
        window_size: (u32, u32),
    ) {
        let (x0, y0, x1, y1) = rect_px;
        let (window_w, window_h) = (window_size.0 as f32, window_size.1 as f32);

        let to_ndc = |x: f32, y: f32| -> [f32; 2] {
            [(x / window_w) * 2.0 - 1.0, 1.0 - (y / window_h) * 2.0]
        };

        let vertices = [
            CursorVertex { position: to_ndc(x0, y0) },
            CursorVertex { position: to_ndc(x1, y0) },
            CursorVertex { position: to_ndc(x1, y1) },
            CursorVertex { position: to_ndc(x0, y1) },
        ];

        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
    }

    pub fn render<'pass>(&'pass self, render_pass: &mut wgpu::RenderPass<'pass>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..6, 0, 0..1);
    }
}
