use std::sync::Arc;
use std::time::Instant;

use ide_buffer::TextBuffer;
use ide_gpu::{CursorRenderer, GpuContext, PositionedGlyph, TextRenderer};
use ide_text::{RasterizedGlyph, TextSystem};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

const FONT_SIZE: f32 = 20.0;
const LINE_HEIGHT: f32 = 28.0;
const ORIGIN: (f32, f32) = (24.0, 24.0);
const CURSOR_WIDTH: f32 = 2.0;
const BLINK_PERIOD_MS: u128 = 500;

fn build_editor_text_renderer(
    gpu: &GpuContext,
    text_system: &mut TextSystem,
    buffer: &TextBuffer,
    window_size: (u32, u32),
) -> anyhow::Result<(TextRenderer, (f32, f32, f32, f32))> {
    let mut lines_glyphs: Vec<Vec<RasterizedGlyph>> = Vec::with_capacity(buffer.line_count());

    for i in 0..buffer.line_count() {
        let raw_line = buffer.line(i).unwrap_or_default();
        let trimmed = raw_line.trim_end_matches(['\n', '\r']);

        if trimmed.is_empty() {
            lines_glyphs.push(Vec::new());
            continue;
        }

        match text_system.shape_line(trimmed, FONT_SIZE, LINE_HEIGHT) {
            Ok(glyphs) => lines_glyphs.push(glyphs),
            Err(_) => lines_glyphs.push(Vec::new()),
        }
    }

    let mut positioned: Vec<PositionedGlyph> = Vec::new();
    for (i, glyphs) in lines_glyphs.iter().enumerate() {
        let line_origin_y = ORIGIN.1 + i as f32 * LINE_HEIGHT;
        for g in glyphs {
            positioned.push(PositionedGlyph {
                glyph: g,
                screen_x: ORIGIN.0 + g.x as f32,
                screen_y: line_origin_y + g.y as f32,
            });
        }
    }

    let text_renderer = TextRenderer::new(
        gpu.device(),
        gpu.queue(),
        gpu.surface_format(),
        &positioned,
        window_size,
    );

    let line0_end_x = lines_glyphs
        .first()
        .and_then(|glyphs| glyphs.last())
        .map(|g| ORIGIN.0 + g.x as f32 + g.width as f32)
        .unwrap_or(ORIGIN.0);
    let cursor_rect = (
        line0_end_x,
        ORIGIN.1,
        line0_end_x + CURSOR_WIDTH,
        ORIGIN.1 + LINE_HEIGHT,
    );

    Ok((text_renderer, cursor_rect))
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
    let buffer = TextBuffer::from_str(
        "Quantum Studio\n\nA GPU-accelerated, Rust-native IDE.\nPhase 4 starts soon.\n",
    );

    let (mut text_renderer, mut cursor_rect) =
        build_editor_text_renderer(&gpu, &mut text_system, &buffer, (size.width, size.height))?;
    let cursor_renderer = CursorRenderer::new(gpu.device(), gpu.surface_format());

    let start_time = Instant::now();

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

                        match build_editor_text_renderer(
                            &gpu,
                            &mut text_system,
                            &buffer,
                            (new_size.width, new_size.height),
                        ) {
                            Ok((renderer, rect)) => {
                                text_renderer = renderer;
                                cursor_rect = rect;
                            }
                            Err(e) => {
                                tracing::warn!(error = ?e, "failed to rebuild text renderer on resize")
                            }
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        let blink_on =
                            (start_time.elapsed().as_millis() / BLINK_PERIOD_MS) % 2 == 0;

                        if blink_on {
                            cursor_renderer.update(gpu.queue(), cursor_rect, gpu.size());
                        }

                        let cursor_to_draw = blink_on.then_some(&cursor_renderer);

                        match gpu.render(Some(&text_renderer), cursor_to_draw) {
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
