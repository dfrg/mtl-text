use super::font::Font;

pub struct TextBuilder {
    max_width: f32,
    x: f32,
    y: f32,
    lines: Vec<Line>,
}

pub struct Text {
    pub lines: Vec<Line>,
}

pub struct Run {
    pub font: Font,
    pub font_size: f32,
    pub color: [f32; 4],
    pub ids: Vec<u16>,
    pub advances: Vec<f32>,
}

#[derive(Default)]
pub struct Line {
    pub y: f32,
    pub ascent: f32,
    pub descent: f32,
    pub runs: Vec<Run>,
}

impl TextBuilder {
    pub fn new(max_width: Option<f32>) -> Self {
        Self {
            max_width: max_width.unwrap_or(f32::MAX),
            x: 0.0,
            y: 0.0,
            lines: vec![Line::default()],
        }
    }

    pub fn add_text(mut self, font: &Font, font_size: f32, color: [f32; 4], text: &str) -> Self {
        let charmap = font.as_ref().charmap();
        let glyph_metrics = font.as_ref().glyph_metrics(&[]).scale(font_size);
        let metrics = font.as_ref().metrics(&[]).scale(font_size);
        let ascent = metrics.ascent;
        let descent = metrics.descent;
        let mut ids = vec![];
        let mut advances = vec![];
        for ch in text.chars() {
            let id = charmap.map(ch);
            let advance = glyph_metrics.advance_width(id);
            let mut end = self.x + advance;
            let mut line = self.lines.last_mut().unwrap();
            if ch == '\n' || end > self.max_width {
                line.runs.push(Run {
                    font: font.clone(),
                    font_size,
                    color,
                    ids: ids.clone(),
                    advances: advances.clone(),
                });
                ids.clear();
                advances.clear();
                self.x = 0.0;
                self.y = (line.y + line.ascent + line.descent).round();
                self.lines.push(Line {
                    y: self.y,
                    ..Default::default()
                });
                line = self.lines.last_mut().unwrap();
                end = advance;
            }
            line.ascent = line.ascent.max(ascent);
            line.descent = line.descent.max(descent);
            self.x = end;
            ids.push(id);
            advances.push(advance)
        }
        if !ids.is_empty() {
            let line = self.lines.last_mut().unwrap();
            line.ascent = line.ascent.max(ascent);
            line.descent = line.descent.max(descent);
            line.runs.push(Run {
                font: font.clone(),
                font_size,
                color,
                ids,
                advances,
            });
        }
        self
    }

    pub fn build(self) -> Text {
        Text { lines: self.lines }
    }
}
