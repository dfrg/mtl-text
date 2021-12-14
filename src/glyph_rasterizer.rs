use super::glyph::Glyph;
use metal::*;
use swash::scale::{image::Image, ScaleContext};

pub enum Format {
    A8,
    Rgba8,
}

pub trait GlyphRasterizer {
    fn new(device: &DeviceRef) -> Self;
    fn begin(&mut self, format: Format, width: u32, height: u32);
    fn add_glyph(&mut self, glyph: &Glyph);
    fn record(&mut self, cmdbuf: &CommandBufferRef, target: &TextureRef) -> u32;
    fn release(&mut self, id: u32);
}

pub struct SoftwareGlyphRasterizer {
    scx: ScaleContext,
    image: Image,
    pixbuf: Vec<u8>,
    format: Format,
    width: usize,
    height: usize,
    stride: usize,
}

impl GlyphRasterizer for SoftwareGlyphRasterizer {
    fn new(_device: &DeviceRef) -> Self {
        Self {
            scx: ScaleContext::new(),
            image: Image::new(),
            pixbuf: vec![],
            format: Format::A8,
            width: 0,
            height: 0,
            stride: 0,
        }
    }

    fn begin(&mut self, format: Format, width: u32, height: u32) {
        let pixel_size = match format {
            Format::A8 => 1,
            Format::Rgba8 => 4,
        };
        self.format = format;
        self.width = width as usize;
        self.height = height as usize;
        self.stride = self.width * pixel_size;
        let size = self.stride * self.height;
        self.pixbuf.clear();
        self.pixbuf.resize(size, 0);
    }

    fn add_glyph(&mut self, glyph: &Glyph) {
        use swash::scale::{Render, Source};
        use swash::zeno::{Transform, Vector};
        let mut scaler = self
            .scx
            .builder(glyph.font_ref())
            .size(glyph.font_size)
            .variations(glyph.variations().iter().map(|var| (var.tag, var.value)))
            .build();
        let transform = Transform {
            xx: glyph.transform[0],
            xy: glyph.transform[1],
            yx: glyph.transform[2],
            yy: glyph.transform[3],
            x: glyph.transform[4],
            y: glyph.transform[5],
        };
        Render::new(&[Source::ColorOutline(0), Source::Outline])
            .offset(Vector::new(glyph.subpx, 0.))
            .transform(Some(transform))
            .render_into(&mut scaler, glyph.glyph_id, &mut self.image);
        let placement = self.image.placement;
        let channels = match self.format {
            Format::A8 => 1,
            Format::Rgba8 => 4,
        };
        copy_rect(
            glyph.rect[0],
            glyph.rect[1],
            placement.width as usize,
            &self.image.data,
            self.width,
            &mut self.pixbuf,
            channels,
        );
    }

    fn record(&mut self, _cmdbuf: &CommandBufferRef, target: &TextureRef) -> u32 {
        target.replace_region(
            MTLRegion::new_2d(0, 0, self.width as _, self.height as _),
            0,
            self.pixbuf.as_ptr() as _,
            self.stride as _,
        );
        !0
    }

    fn release(&mut self, _id: u32) {}
}

fn copy_rect(
    x: u16,
    y: u16,
    width: usize,
    image: &[u8],
    target_width: usize,
    target: &mut [u8],
    channels: usize,
) -> Option<()> {
    let image_pitch = width * channels;
    let buffer_pitch = target_width * channels;
    let mut offset = y as usize * buffer_pitch + x as usize * channels;
    for row in image.chunks(image_pitch) {
        let dest = target.get_mut(offset..offset + image_pitch)?;
        dest.copy_from_slice(row);
        offset += buffer_pitch;
    }
    Some(())
}
