use super::font::Font;
use super::glyph_cache::{GlyphCache, GlyphKey, SubpixelOffset};
use super::glyph_rasterizer::GlyphRasterizer;
use super::text::Text;
use metal::*;
use std::ops::Range;
use swash::scale::ScaleContext;

pub struct Renderer<G> {
    pub device: Device,
    pub layer: MetalLayer,
    pub queue: CommandQueue,
    pub width: u32,
    pub height: u32,
    pub glyph_cache: GlyphCache,
    pub glyph_rasterizer: G,
    scale_ctx: ScaleContext,
    glyphs: Vec<RunGlyph>,
    runs: Vec<RunRange>,
}

impl<G: GlyphRasterizer> Renderer<G> {
    pub fn new() -> Self {
        let device = Device::system_default().expect("no metal device available");
        let layer = MetalLayer::new();
        layer.set_device(&device);
        layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        layer.set_presents_with_transaction(false);
        let queue = device.new_command_queue();
        let glyph_rasterizer = G::new(&device);
        let glyph_cache = GlyphCache::new(device.clone());
        Self {
            device,
            layer,
            queue,
            width: 0,
            height: 0,
            glyph_cache,
            glyph_rasterizer,
            scale_ctx: ScaleContext::new(),
            glyphs: vec![],
            runs: vec![],
        }
    }

    pub fn set_target_size(&mut self, width: u32, height: u32) {
        self.layer
            .set_drawable_size(CGSize::new(width as f64, height as f64));
        self.width = width;
        self.height = height;
    }

    pub fn new_frame(&mut self, bg_color: [f32; 4]) -> FrameRenderer<G> {
        self.glyphs.clear();
        self.runs.clear();
        FrameRenderer {
            r: self,
            bg_color,
            flush_cache: false,
        }
    }
}

pub struct FrameRenderer<'a, G> {
    r: &'a mut Renderer<G>,
    bg_color: [f32; 4],
    flush_cache: bool,
}

impl<'a, G: GlyphRasterizer> FrameRenderer<'a, G> {
    pub fn draw_text(&mut self, x: f32, y: f32, text: &Text) {
        for line in &text.lines {
            let baseline = y + line.y + line.ascent;
            let mut pen_x = x;
            for run in &line.runs {
                let start = self.r.glyphs.len();
                for (id, advance) in run.ids.iter().zip(&run.advances) {
                    let subpx = SubpixelOffset::quantize(pen_x);
                    self.r.glyphs.push(RunGlyph {
                        id: *id,
                        x: pen_x,
                        y: baseline,
                    });
                    if !self.flush_cache && self.r.glyph_cache.map.get(&GlyphKey {
                        font_id: run.font.key,
                        font_size: run.font_size.to_bits(),
                        subpx,
                        id: *id,
                    }).is_none() {
                        self.flush_cache = true;
                    }
                    pen_x += *advance;
                }
                let end = self.r.glyphs.len();
                self.r.runs.push(RunRange {
                    font: run.font.clone(),
                    font_size: run.font_size,
                    color: run.color,
                    range: start..end,
                });
            }
        }
    }

    pub fn render(mut self) {
        if self.flush_cache {
            self.render_uncached();
        } else {
            self.render_cached();
        }
        let drawable = match self.r.layer.next_drawable() {
            Some(drawable) => drawable,
            None => return,
        };
        let pass = RenderPassDescriptor::new();
        let color_attachment = pass.color_attachments().object_at(0).unwrap();
        color_attachment.set_texture(Some(&drawable.texture()));
        color_attachment.set_load_action(MTLLoadAction::Clear);
        let [r, g, b, a] = self.bg_color;
        color_attachment.set_clear_color(MTLClearColor::new(r as _, g as _, b as _, a as _));
        color_attachment.set_store_action(MTLStoreAction::Store);
        let cmdbuf = self.r.queue.new_command_buffer();
        let encoder = cmdbuf.new_render_command_encoder(&pass);
        encoder.end_encoding();
        cmdbuf.present_drawable(&drawable);
        cmdbuf.commit();
    }

    fn render_uncached(&mut self) {
        self.r.glyph_cache.clear();
    }

    fn render_cached(&mut self) {

    }
}

struct RunRange {
    font: Font,
    font_size: f32,
    color: [f32; 4],
    range: Range<usize>,
}

struct RunGlyph {
    id: u16,
    x: f32,
    y: f32,
}
