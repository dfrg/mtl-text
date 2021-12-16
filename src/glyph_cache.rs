use metal::{Device, Texture};
use std::collections::HashMap;
use swash::CacheKey;

pub const ATLAS_SIZE: u32 = 1024;

pub struct Atlas {
    pub texture: Texture,
    pub allocator: etagere::AtlasAllocator,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub font_id: CacheKey,
    pub font_size: u32,
    pub subpx: SubpixelOffset,
    pub id: u16,
}

#[derive(Copy, Clone)]
pub struct GlyphEntry {
    pub is_color: bool,
    pub uv: [f32; 4],
    pub left: i16,
    pub top: i16,
    pub width: u16,
    pub height: u16,
}

pub struct GlyphCache {
    device: Device,
    pub alpha: Option<Atlas>,
    pub color: Option<Atlas>,
    pub map: HashMap<GlyphKey, GlyphEntry>,
    // TODO: variations
}

impl GlyphCache {
    pub fn new(device: Device) -> Self {
        Self {
            device,
            alpha: None,
            color: None,
            map: Default::default(),
        }
    }

    pub fn get(&self, key: &GlyphKey) -> Option<&GlyphEntry> {
        self.map.get(key)
    }

    pub fn insert(
        &mut self,
        key: GlyphKey,
        is_color: bool,
        width: u16,
        height: u16,
    ) -> Option<&mut GlyphEntry> {
        let (atlas, format) = if is_color {
            (&mut self.color, metal::MTLPixelFormat::RGBA8Unorm)
        } else {
            (&mut self.alpha, metal::MTLPixelFormat::A8Unorm)
        };
        if atlas.is_none() {
            let desc = metal::TextureDescriptor::new();
            desc.set_width(ATLAS_SIZE as _);
            desc.set_height(ATLAS_SIZE as _);
            desc.set_pixel_format(format);
            desc.set_usage(
                metal::MTLTextureUsage::ShaderRead | metal::MTLTextureUsage::ShaderWrite,
            );
            let texture = self.device.new_texture(&desc);
            *atlas = Some(Atlas {
                texture,
                allocator: etagere::AtlasAllocator::new(etagere::size2(
                    ATLAS_SIZE as i32,
                    ATLAS_SIZE as i32,
                )),
            });
        }
        let entry = if width == 0 || height == 0 {
            GlyphEntry {
                is_color,
                uv: [0.0; 4],
                left: 0,
                top: 0,
                width,
                height,
            }
        } else {
            let atlas = atlas.as_mut()?;
            let allocation = atlas
                .allocator
                .allocate(etagere::size2(width as i32, height as i32))?;
            let rect = allocation.rectangle;
            let x0 = rect.min.x as f32 / ATLAS_SIZE as f32;
            let y0 = rect.min.y as f32 / ATLAS_SIZE as f32;
            let x1 = (rect.min.x as f32 + width as f32) / ATLAS_SIZE as f32;
            let y1 = (rect.min.y as f32 + height as f32) / ATLAS_SIZE as f32;
            GlyphEntry {
                is_color,
                uv: [x0, y0, x1, y1],
                left: 0,
                top: 0,
                width,
                height,
            }
        };
        self.map.insert(key, entry);
        self.map.get_mut(&key)
    }

    pub fn clear(&mut self) {
        for atlas in self.alpha.iter_mut().chain(self.color.iter_mut()) {
            atlas.allocator.clear();
        }
        self.map.clear();
    }
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
