use ide_text::RasterizedGlyph;
use wgpu::util::DeviceExt;


pub struct PositionedGlyph<'a> {
    pub glyph: &'a RasterizedGlyph,
    pub screen_x: f32,
    pub screen_y: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
}

struct PackedRect {
    x: u32,
    y: u32,
}

struct ShelfPacker {
    atlas_width: u32,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
}

impl ShelfPacker {
    fn new(atlas_width: u32) -> Self {
        Self {
            atlas_width,
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
        }
    }

    fn place(&mut self, width: u32, height: u32) -> PackedRect {
        if self.cursor_x + width > self.atlas_width {
            self.cursor_x = 0;
            self.cursor_y += self.row_height;
            self.row_height = 0;
        }
        let rect = PackedRect {
            x: self.cursor_x,
            y: self.cursor_y,
        };
        self.cursor_x += width;
        self.row_height = self.row_height.max(height);
        rect
    }

    fn used_height(&self) -> u32 {
        self.cursor_y + self.row_height
    }
}

pub struct TextRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

impl TextRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        glyphs: &[PositionedGlyph],
        window_size: (u32, u32),
    ) -> Self {
        const ATLAS_WIDTH: u32 = 1024;
        let mut packer = ShelfPacker::new(ATLAS_WIDTH);
        let placements: Vec<PackedRect> = glyphs
            .iter()
            .map(|positioned| packer.place(positioned.glyph.width, positioned.glyph.height))
            .collect();
        let atlas_height = packer.used_height().max(1);

        let mut atlas_pixels = vec![0u8; (ATLAS_WIDTH * atlas_height * 4) as usize];
        for (positioned, rect) in glyphs.iter().zip(placements.iter()) {
            let g = positioned.glyph;
            for row in 0..g.height {
                let src_start = (row * g.width * 4) as usize;
                let src_end = src_start + (g.width * 4) as usize;
                let src_row = &g.pixels[src_start..src_end];

                let dst_y = rect.y + row;
                let dst_start = ((dst_y * ATLAS_WIDTH + rect.x) * 4) as usize;
                let dst_end = dst_start + (g.width * 4) as usize;
                atlas_pixels[dst_start..dst_end].copy_from_slice(src_row);
            }
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph-atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_WIDTH,
                height: atlas_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas_pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(ATLAS_WIDTH * 4),
                rows_per_image: Some(atlas_height),
            },
            wgpu::Extent3d {
                width: ATLAS_WIDTH,
                height: atlas_height,
                depth_or_array_layers: 1,
            },
        );

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glyph-atlas-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let (window_w, window_h) = (window_size.0 as f32, window_size.1 as f32);
        let mut vertices: Vec<Vertex> = Vec::with_capacity(glyphs.len() * 4);
        let mut indices: Vec<u16> = Vec::with_capacity(glyphs.len() * 6);

        let to_ndc = |x: f32, y: f32| -> [f32; 2] {
            [(x / window_w) * 2.0 - 1.0, 1.0 - (y / window_h) * 2.0]
        };

        for (positioned, rect) in glyphs.iter().zip(placements.iter()) {
            let g = positioned.glyph;
            let base = vertices.len() as u16;

            let px0 = positioned.screen_x;
            let py0 = positioned.screen_y;
            let px1 = px0 + g.width as f32;
            let py1 = py0 + g.height as f32;

            let u0 = rect.x as f32 / ATLAS_WIDTH as f32;
            let v0 = rect.y as f32 / atlas_height as f32;
            let u1 = (rect.x + g.width) as f32 / ATLAS_WIDTH as f32;
            let v1 = (rect.y + g.height) as f32 / atlas_height as f32;

            vertices.push(Vertex { position: to_ndc(px0, py0), uv: [u0, v0] });
            vertices.push(Vertex { position: to_ndc(px1, py0), uv: [u1, v0] });
            vertices.push(Vertex { position: to_ndc(px1, py1), uv: [u1, v1] });
            vertices.push(Vertex { position: to_ndc(px0, py1), uv: [u0, v1] });

            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("glyph-vertex-buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("glyph-index-buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("glyph-bind-group-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glyph-bind-group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("glyph-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("glyph.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("glyph-pipeline-layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("glyph-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
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

        Self {
            pipeline,
            bind_group,
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        }
    }

    pub fn render<'pass>(&'pass self, render_pass: &mut wgpu::RenderPass<'pass>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}
