use std::sync::Arc;
use swash::{CacheKey, FontDataRef, FontRef};

#[derive(Clone)]
pub struct Font {
    pub data: Arc<Vec<u8>>,
    pub offset: u32,
    pub key: CacheKey,
}

impl Font {
    pub fn new(data: Vec<u8>) -> Option<Self> {
        let font_data = FontDataRef::new(&data)?;
        let font_ref = font_data.get(0)?;
        let offset = font_ref.offset;
        let key = font_ref.key;
        Some(Self {
            data: Arc::new(data),
            offset,
            key,
        })
    }

    pub fn from_file(path: &str) -> Option<Self> {
        Self::new(std::fs::read(path).ok()?)
    }

    pub fn as_ref(&self) -> FontRef {
        FontRef {
            data: &*self.data,
            offset: self.offset,
            key: self.key,
        }
    }
}
