use super::font::Font;
use super::glyph_rasterizer::GlyphRasterizer;
use super::render::Renderer;

pub struct TextLayoutBuilder<'a, G: GlyphRasterizer> {
    renderer: &'a mut Renderer<G>,
    max_width: f32,
    runs: Vec<BuilderRun>,
}

struct BuilderRun {
    font: Font,
    size: f32,
    glyphs: Vec<u16>,
}

impl<'a, G: GlyphRasterizer> TextLayoutBuilder<'a, G> {
    pub fn new(renderer: &'a mut Renderer<G>, max_width: f32) -> Self {
        Self {
            renderer,
            max_width,
            runs: vec![],
        }
    }

    pub fn add_text(&mut self, font: Font, size: f32, text: &str) {
        let charmap = font.as_ref().charmap();
        let glyphs = text.chars().map(|ch| charmap.map(ch)).collect();
        self.runs.push(BuilderRun { font, size, glyphs })
    }
}
