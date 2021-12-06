use metal::Texture;
use std::collections::HashMap;
use swash::CacheKey;

pub struct Atlas {
    pub texture: Texture,
    pub allocator: etagere::AtlasAllocator,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub font_id: CacheKey,
    pub font_size: u32,
    pub subpx: SubpixelOffset,
    pub glyph_id: u16,
}

#[derive(Copy, Clone)]
pub struct GlyphEntry {
    pub atlas: u16,
    pub is_color: bool,
    pub uv: [f32; 4],
}

#[derive(Default)]
pub struct GlyphCache {
    /// Alpha mask textures
    pub atlases: Vec<Atlas>,
    /// RGBA textures
    pub color_atlases: Vec<Atlas>,
    pub map: HashMap<GlyphKey, GlyphEntry>,
    // TODO: variations
}

#[derive(Hash, Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum SubpixelOffset {
    Zero = 0,
    Quarter = 1,
    Half = 2,
    ThreeQuarters = 3,
}

impl SubpixelOffset {
    pub fn quantize(pos: f32) -> Self {
        let apos = ((pos - pos.floor()) * 8.0) as i32;
        match apos {
            1..=2 => SubpixelOffset::Quarter,
            3..=4 => SubpixelOffset::Half,
            5..=6 => SubpixelOffset::ThreeQuarters,
            _ => SubpixelOffset::Zero,
        }
    }

    pub fn to_f32(self) -> f32 {
        match self {
            SubpixelOffset::Zero => 0.0,
            SubpixelOffset::Quarter => 0.25,
            SubpixelOffset::Half => 0.5,
            SubpixelOffset::ThreeQuarters => 0.75,
        }
    }
}
