use super::glyph_cache::GlyphCache;
use super::glyph_rasterizer::GlyphRasterizer;
use metal::*;

pub struct Renderer<G> {
    pub device: Device,
    pub layer: MetalLayer,
    pub queue: CommandQueue,
    pub width: u32,
    pub height: u32,
    pub glyph_cache: GlyphCache,
    pub glyph_rasterizer: G,
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
        Self {
            device,
            layer,
            queue,
            width: 0,
            height: 0,
            glyph_cache: GlyphCache::default(),
            glyph_rasterizer,
        }
    }

    pub fn set_target_size(&mut self, width: u32, height: u32) {
        self.layer
            .set_drawable_size(CGSize::new(width as f64, height as f64));
        self.width = width;
        self.height = height;
    }

    pub fn new_frame(&mut self, clear_color: [f32; 4]) -> FrameRenderer<G> {
        FrameRenderer {
            r: self,
            clear_color,
        }
    }
}

pub struct FrameRenderer<'a, G> {
    r: &'a mut Renderer<G>,
    clear_color: [f32; 4],
}

impl<'a, G: GlyphRasterizer> FrameRenderer<'a, G> {
    pub fn render(self) {
        let drawable = match self.r.layer.next_drawable() {
            Some(drawable) => drawable,
            None => return,
        };
        let pass = RenderPassDescriptor::new();
        let color_attachment = pass.color_attachments().object_at(0).unwrap();
        color_attachment.set_texture(Some(&drawable.texture()));
        color_attachment.set_load_action(MTLLoadAction::Clear);
        let [r, g, b, a] = self.clear_color;
        color_attachment.set_clear_color(MTLClearColor::new(r as _, g as _, b as _, a as _));
        color_attachment.set_store_action(MTLStoreAction::Store);
        let cmdbuf = self.r.queue.new_command_buffer();
        let encoder = cmdbuf.new_render_command_encoder(&pass);
        encoder.end_encoding();
        cmdbuf.present_drawable(&drawable);
        cmdbuf.commit();
    }
}
