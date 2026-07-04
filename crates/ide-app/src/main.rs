use std::sync::Arc;

use ide_gpu::GpuContext;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

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
                    }
                    WindowEvent::RedrawRequested => {
                        match gpu.render() {
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