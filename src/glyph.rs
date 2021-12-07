use swash::{FontDataRef, FontRef};

#[derive(Copy, Clone)]
#[repr(C)]
pub struct FontVariation {
    pub tag: u32,
    pub value: f32,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Glyph {
    pub unique_id: u64,
    pub font_data: *const u8,
    pub font_data_len: u32,
    pub font_size: f32,
    pub glyph_id: u16,
    pub transform: [f32; 6],
    pub rect: [u16; 4],
    pub subpx: f32,
    pub variations: *const FontVariation,
    pub num_variations: u32,
}

impl Glyph {
    pub fn font_ref(&self) -> FontRef {
        unsafe {
            let data = std::slice::from_raw_parts(self.font_data, self.font_data_len as usize);
            let font_data = FontDataRef::new(data).expect("invalid font");
            let mut font_ref = font_data.get(0).expect("invalid font index");
            font_ref.key = std::mem::transmute(self.unique_id);
            font_ref
        }
    }

    pub fn variations(&self) -> &[FontVariation] {
        unsafe { std::slice::from_raw_parts(self.variations, self.num_variations as usize) }
    }
}
