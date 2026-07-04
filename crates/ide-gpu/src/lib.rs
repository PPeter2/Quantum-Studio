use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GpuError {
    #[error("failed to create surface: {0}")]
    SurfaceCreation(#[from] wgpu::CreateSurfaceError),

    #[error("no suitable GPU adapter found")]
    NoAdapter,

    #[error("failed to request device: {0}")]
    DeviceRequest(#[from] wgpu::RequestDeviceError),
}

pub const BACKGROUND_COLOR: wgpu::Color = wgpu::Color {
    r: 0.086, // #16
    g: 0.090, // #17
    b: 0.098, // #19
    a: 1.0,
};

pub struct GpuContext {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: (u32, u32),
}

impl GpuContext {
    pub async fn new(
        window: Arc<winit::window::Window>,
        width: u32,
        height: u32,
    ) -> Result<Self, GpuError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window)?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or(GpuError::NoAdapter)?;

        tracing::info!(adapter = ?adapter.get_info(), "selected GPU adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("quantum-studio-device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size: (width, height),
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return; // window minimized — skip, don't crash on a 0x0 surface
        }
        self.size = (width, height);
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }
    pub fn render(&self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("quantum-studio-encoder"),
            });

        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(BACKGROUND_COLOR),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}