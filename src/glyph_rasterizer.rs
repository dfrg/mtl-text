use super::glyph::Glyph;
use metal::*;

pub trait GlyphRasterizer {
    type AtlasBuilder: AtlasBuilder;

    fn new(device: &DeviceRef) -> Self;
    fn new_atlas(&mut self) -> Self::AtlasBuilder;
}

pub trait AtlasBuilder {
    fn add_glyph(&mut self, glyph: &Glyph);
    fn record(&mut self, cmdbuf: &CommandBufferRef, target: &TextureRef) -> u32;
    fn release(&mut self, id: u32);
}
