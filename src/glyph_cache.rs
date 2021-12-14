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
    pub glyph_id: u16,
}

#[derive(Copy, Clone)]
pub struct GlyphEntry {
    pub atlas: u16,
    pub is_color: bool,
    pub uv: [f32; 4],
    pub left: i32,
    pub top: i32,
}

pub struct GlyphCache {
    device: Device,
    /// Alpha mask textures
    pub atlases: Vec<Atlas>,
    /// RGBA textures
    pub color_atlases: Vec<Atlas>,
    pub map: HashMap<GlyphKey, GlyphEntry>,
    // TODO: variations
}

impl GlyphCache {
    pub fn new(device: Device) -> Self {
        Self {
            device,
            atlases: vec![],
            color_atlases: vec![],
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
        width: u32,
        height: u32,
    ) -> Option<&mut GlyphEntry> {
        let atlases = if is_color {
            &mut self.color_atlases
        } else {
            &mut self.atlases
        };
        let mut atlas_index = None;
        let mut allocation = None;
        for (i, atlas) in atlases.iter_mut().enumerate() {
            if let Some(alloc) = atlas
                .allocator
                .allocate(etagere::size2(width as i32, height as i32))
            {
                atlas_index = Some(i);
                allocation = Some(alloc);
                break;
            }
        }
        if atlas_index.is_none() {
            atlas_index = Some(atlases.len());
            let format = if is_color {
                metal::MTLPixelFormat::RGBA8Unorm
            } else {
                metal::MTLPixelFormat::A8Unorm
            };
            let desc = metal::TextureDescriptor::new();
            desc.set_width(ATLAS_SIZE as _);
            desc.set_height(ATLAS_SIZE as _);
            desc.set_pixel_format(format);
            let texture = self.device.new_texture(&desc);
            let mut atlas = Atlas {
                texture,
                allocator: etagere::AtlasAllocator::new(etagere::size2(
                    ATLAS_SIZE as i32,
                    ATLAS_SIZE as i32,
                )),
            };
            allocation = atlas
                .allocator
                .allocate(etagere::size2(width as i32, height as i32));
            atlases.push(atlas);
        }
        let (atlas_index, allocation) = (atlas_index?, allocation?);
        let rect = allocation.rectangle;
        let x0 = rect.min.x as f32 / ATLAS_SIZE as f32;
        let y0 = rect.min.y as f32 / ATLAS_SIZE as f32;
        let x1 = (rect.min.x as f32 + width as f32) / ATLAS_SIZE as f32;
        let y1 = (rect.min.y as f32 + height as f32) / ATLAS_SIZE as f32;
        let entry = GlyphEntry {
            atlas: atlas_index as u16,
            is_color,
            uv: [x0, y0, x1, y1],
            left: 0,
            top: 0,
        };
        self.map.insert(key, entry);
        self.map.get_mut(&key)
    }

    pub fn clear(&mut self) {
        while self.atlases.len() > 1 {
            self.atlases.pop();
        }
        while self.color_atlases.len() > 1 {
            self.color_atlases.pop();
        }
        for atlas in self.atlases.iter_mut().chain(self.color_atlases.iter_mut()) {
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
