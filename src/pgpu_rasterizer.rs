use piet_gpu::{glyph_render::GlyphRenderer, PixelFormat, RenderConfig};
use piet_gpu_hal::{QueryPool, Session};

use super::glyph_rasterizer::GlyphRasterizer;

pub struct PgpuRasterizer {
    session: Session,
    glyph_renderer: GlyphRenderer,
    pgpu_renderer: Option<piet_gpu::Renderer>,
    query_pool: QueryPool,
    width: u32,
    height: u32,
    format: crate::glyph_rasterizer::Format,
}

impl GlyphRasterizer for PgpuRasterizer {
    fn new(device: &metal::DeviceRef, queue: &metal::CommandQueueRef) -> Self {
        let piet_device = piet_gpu_hal::Device::new_from_raw_mtl(device, &queue);
        let session = Session::new(piet_device);
        let glyph_renderer = GlyphRenderer::new();
        let query_pool = session.create_query_pool(8).unwrap();
        PgpuRasterizer {
            session,
            glyph_renderer,
            pgpu_renderer: None,
            query_pool,
            width: 0,
            height: 0,
            format: crate::glyph_rasterizer::Format::A8,
        }
    }

    fn begin(&mut self, format: crate::glyph_rasterizer::Format, width: u32, height: u32) {
        if self.pgpu_renderer.is_none()
            || self.width != width
            || self.height != height
            || self.format != format
        {
            self.width = width;
            self.height = height;
            self.format = format;
            let format = match format {
                crate::glyph_rasterizer::Format::A8 => PixelFormat::A8,
                crate::glyph_rasterizer::Format::Rgba8 => PixelFormat::Rgba8,
            };
            let config = RenderConfig::new(width as usize, height as usize).pixel_format(format);
            unsafe {
                self.pgpu_renderer =
                    piet_gpu::Renderer::new_from_config(&self.session, config, 1).ok();
            }
        }
    }

    unsafe fn add_glyph(&mut self, glyph: &crate::glyph::Glyph) {
        let font_data = std::slice::from_raw_parts(glyph.font_data, glyph.font_data_len as usize);
        // TODO: fine-tune transform (including subpix)
        let transform = [
            glyph.font_size,
            0.0,
            0.0,
            -glyph.font_size,
            glyph.rect[0] as f32,
            glyph.rect[3] as f32,
        ];
        self.glyph_renderer
            .add_glyph(font_data, glyph.unique_id, glyph.glyph_id, transform);
    }

    fn record(&mut self, cmdbuf: &metal::CommandBufferRef, target: &metal::TextureRef) -> u32 {
        unsafe {
            let mut cmd_buf = self.session.cmd_buf_from_raw_mtl(cmdbuf);
            let dst_image = self
                .session
                .image_from_raw_mtl(target, self.width, self.height);
            if let Some(renderer) = &mut self.pgpu_renderer {
                renderer
                    .upload_render_ctx(&mut self.glyph_renderer.render_ctx, 0)
                    .unwrap();
                renderer.record(&mut cmd_buf, &self.query_pool, 0);
                // TODO later: we can bind the destination image and avoid the copy.
                cmd_buf.blit_image(&renderer.image_dev, &dst_image);
            }
        }
        // Return a token which will be used by release. Probably the right thing to do is use
        // it to select a buffer index from the pool.
        0
    }

    fn release(&mut self, _id: u32) {
        // TODO: worry about this later.
    }
}
