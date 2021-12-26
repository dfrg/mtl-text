use super::glyph::Glyph;
use metal::*;
use swash::scale::{image::Image, ScaleContext};

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Format {
    A8,
    Rgba8,
}

pub trait GlyphRasterizer {
    fn new(device: &DeviceRef, queue: &CommandQueueRef) -> Self;
    fn begin(&mut self, format: Format, width: u32, height: u32);
    // Unsafe because pointer in glyph must reference valid slice.
    unsafe fn add_glyph(&mut self, glyph: &Glyph);
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
    fn new(_device: &DeviceRef, _queue: &CommandQueueRef) -> Self {
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

    unsafe fn add_glyph(&mut self, glyph: &Glyph) {
        println!("adding glyph {:?}", glyph);
        use swash::scale::{Render, Source};
        use swash::zeno::{Transform, Vector};
        let mut scaler = self
            .scx
            .builder(glyph.font_ref())
            .size(glyph.font_size)
            .variations(glyph.variations().iter().map(|var| (var.tag, var.value)))
            .build();
        // Which components should be present here? piet-gpu and swash have
        // different expectations
        let _transform = Transform {
            xx: glyph.transform[0],
            xy: glyph.transform[1],
            yx: glyph.transform[2],
            yy: glyph.transform[3],
            x: glyph.transform[4],
            y: glyph.transform[5],
        };
        self.image.clear();
        Render::new(&[Source::ColorOutline(0), Source::Outline])
            .offset(Vector::new(glyph.subpx, 0.))
            // See above
            // .transform(Some(transform))
            .render_into(&mut scaler, glyph.glyph_id, &mut self.image);
        let placement = self.image.placement;
        let channels = match self.format {
            Format::A8 => 1,
            Format::Rgba8 => 4,
        };
        if placement.width == 0 || placement.height == 0 {
            return;
        }
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

mod tga {
    use std::fs::File;
    use std::io::{self, Write};
    use std::slice;

    fn write<W: Write, T>(w: &mut W, v: T) -> io::Result<()> {
        let size = std::mem::size_of::<T>();
        let bytes = unsafe { slice::from_raw_parts(&v as *const T as *const u8, size) };
        w.write_all(bytes)
    }

    pub fn write_image(
        buf: &[u8],
        width: u32,
        height: u32,
        channels: u32,
        path: &str,
    ) -> io::Result<()> {
        let f = File::create(path)?;
        let mut f = std::io::BufWriter::new(f);
        let w = width as usize;
        let h = height as usize;
        let fp = &mut f;
        write(fp, 0u16)?;
        write(fp, 2u8)?;
        write(fp, 0u32)?;
        write(fp, 0u8)?;
        write(fp, 0u32)?;
        write(fp, w as u16)?;
        write(fp, h as u16)?;
        write(fp, 32u8)?;
        write(fp, 0u8)?;
        let channels = channels as usize;
        for y in 0..h {
            let line = h - y - 1;
            let offset = line * w * channels;
            let mut x = 0;
            while x < w * channels {
                let rgba = if channels == 1 {
                    let v = buf[offset + x];
                    [v, v, v, 255]
                } else {
                    [
                        buf[offset + x + 2],
                        buf[offset + x + 1],
                        buf[offset + x],
                        buf[offset + x + 3],
                    ]
                };
                write(fp, rgba)?;
                x += channels;
            }
        }
        f.flush()?;
        io::Result::Ok(())
    }
}
