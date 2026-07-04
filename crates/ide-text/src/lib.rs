use cosmic_text::{Attrs, Buffer, Color as CosmicColor, FontSystem, Metrics, Shaping, SwashCache};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TextError {
    #[error("no glyphs produced for input text")]
    NoGlyphs,
}

#[derive(Debug, Clone)]
pub struct RasterizedGlyph {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

pub struct TextSystem {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

impl TextSystem {
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
        }
    }

    pub fn shape_line(
        &mut self,
        text: &str,
        font_size: f32,
        line_height: f32,
    ) -> Result<Vec<RasterizedGlyph>, TextError> {
        let metrics = Metrics::new(font_size, line_height);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);        // whole line shapes as one run.
        buffer.set_size(&mut self.font_system, 4096.0, line_height * 2.0);
        buffer.set_text(
            &mut self.font_system,
            text,
            Attrs::new(),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut glyphs = Vec::new();

        for run in buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical = glyph.physical((0.0, 0.0), 1.0);

                let Some(image) = self
                    .swash_cache
                    .get_image(&mut self.font_system, physical.cache_key)
                    .as_ref()
                else {
                    continue;
                };

                if image.placement.width == 0 || image.placement.height == 0 {
                    continue;
                }
                let mut pixels =
                    Vec::with_capacity(image.data.len() * 4);
                for &coverage in &image.data {
                    pixels.extend_from_slice(&[255, 255, 255, coverage]);
                }

                glyphs.push(RasterizedGlyph {
                    x: physical.x + image.placement.left,
                    y: run.line_y as i32 + physical.y - image.placement.top,
                    width: image.placement.width,
                    height: image.placement.height,
                    pixels,
                });
            }
        }

        if glyphs.is_empty() {
            return Err(TextError::NoGlyphs);
        }
        let _ = CosmicColor::rgb(255, 255, 255);

        Ok(glyphs)
    }
}

impl Default for TextSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shaping_quantum_studio_produces_glyphs() {
        let mut text_system = TextSystem::new();
        let glyphs = text_system
            .shape_line("Quantum Studio", 32.0, 40.0)
            .expect("shaping should produce glyphs for plain ASCII text");
        assert_eq!(glyphs.len(), 13, "expected one glyph per visible character");

        for glyph in &glyphs {
            assert!(glyph.width > 0, "glyph bitmap width should be non-zero");
            assert!(glyph.height > 0, "glyph bitmap height should be non-zero");
            assert_eq!(
                glyph.pixels.len(),
                (glyph.width * glyph.height * 4) as usize,
                "pixel buffer size should match width * height * 4 (RGBA8)"
            );
        }
    }

    #[test]
    fn empty_string_returns_no_glyphs_error() {
        let mut text_system = TextSystem::new();
        let result = text_system.shape_line("", 32.0, 40.0);
        assert!(matches!(result, Err(TextError::NoGlyphs)));
    }
}