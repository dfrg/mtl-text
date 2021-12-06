use super::font::Font;
use super::render::Renderer;

pub struct TextLayoutBuilder<'a> {
    renderer: &'a mut Renderer,
    runs: Vec<BuilderRun>,
}

struct BuilderRun {
    font: Font,
    size: f32,
    glyphs: Vec<u16>,
}

impl<'a> TextLayoutBuilder<'a> {
    pub fn new(renderer: &'a mut Renderer) -> Self {
        Self {
            renderer,
            runs: vec![],
        }
    }

    pub fn add_text(&mut self, font: Font, size: f32, text: &str) {
        let charmap = font.as_ref().charmap();
        let glyphs = text.chars().map(|ch| charmap.map(ch)).collect();
        self.runs.push(BuilderRun { font, size, glyphs })
    }

    pub fn build(mut self) -> TextLayout {
        TextLayout {}
    }
}

pub struct TextLayout {}
