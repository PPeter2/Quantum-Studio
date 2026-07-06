use std::sync::Arc;

use ide_gpu::{GpuContext, PositionedGlyph, TextRenderer};
use ide_text::TextSystem;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn build_demo_text_renderer(
    gpu: &GpuContext,
    text_system: &mut TextSystem,
    window_size: (u32, u32),
) -> anyhow::Result<TextRenderer> {
    let glyphs = text_system.shape_line("Quantum Studio", 48.0, 60.0)?;
    let origin = (48.0_f32, 120.0_f32);
    let positioned: Vec<PositionedGlyph> = glyphs
        .iter()
        .map(|g| PositionedGlyph {
            glyph: g,
            screen_x: origin.0 + g.x as f32,
            screen_y: origin.1 + g.y as f32,
        })
        .collect();

    Ok(TextRenderer::new(
        gpu.device(),
        gpu.queue(),
        gpu.surface_format(),
        &positioned,
        window_size,
    ))
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!("starting Quantum Studio");

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let window = Arc::new(
        WindowBuilder::new()
            .with_title("Quantum Studio")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0))
            .with_min_inner_size(winit::dpi::LogicalSize::new(480.0, 320.0))
            .with_decorations(true)
            .build(&event_loop)?,
    );

    let size = window.inner_size();
    let mut gpu = pollster::block_on(GpuContext::new(window.clone(), size.width, size.height))?;

    let mut text_system = TextSystem::new();
    let mut text_renderer =
        build_demo_text_renderer(&gpu, &mut text_system, (size.width, size.height))?;

    event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent { event, window_id } if window_id == window.id() => {
                match event {
                    WindowEvent::CloseRequested => {
                        tracing::info!("close requested, shutting down");
                        elwt.exit();
                    }
                    WindowEvent::Resized(new_size) => {
                        gpu.resize(new_size.width, new_size.height);
                        match build_demo_text_renderer(
                            &gpu,
                            &mut text_system,
                            (new_size.width, new_size.height),
                        ) {
                            Ok(renderer) => text_renderer = renderer,
                            Err(e) => tracing::warn!(error = ?e, "failed to rebuild text renderer on resize"),
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        match gpu.render(Some(&text_renderer)) {
                            Ok(()) => {}
                            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                                let (w, h) = gpu.size();
                                gpu.resize(w, h);
                            }
                            Err(wgpu::SurfaceError::OutOfMemory) => {
                                tracing::error!("GPU out of memory, exiting");
                                elwt.exit();
                            }
                            Err(e) => tracing::warn!(error = ?e, "surface render error"),
                        }
                    }
                    _ => {}
                }
            }
            Event::AboutToWait => {
                window.request_redraw();
            }
            _ => {}
        }
    })?;

    Ok(())
}
