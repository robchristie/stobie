use std::collections::BTreeMap;

use eframe::egui;
use polyorama_core::TileKey;
use polyorama_runtime::RequestToken;

pub const THUMBNAIL_CACHE_BUDGET: usize = 4 * 1024 * 1024;
const THUMBNAIL_SIDE: usize = 64;

struct ThumbnailEntry {
    texture: egui::TextureHandle,
    token: RequestToken,
    bytes: usize,
    touched: u64,
}

/// A bounded presentation cache for worker-decoded thumbnail pixels.
pub struct ThumbnailCache {
    budget: usize,
    used: usize,
    clock: u64,
    entries: BTreeMap<TileKey, ThumbnailEntry>,
}

impl Default for ThumbnailCache {
    fn default() -> Self {
        Self::new(THUMBNAIL_CACHE_BUDGET)
    }
}

impl ThumbnailCache {
    pub fn new(budget: usize) -> Self {
        Self {
            budget,
            used: 0,
            clock: 0,
            entries: BTreeMap::new(),
        }
    }

    pub fn insert(
        &mut self,
        context: &egui::Context,
        key: TileKey,
        token: RequestToken,
        scalar_u16_le: &[u8],
    ) -> Result<Vec<(TileKey, RequestToken)>, String> {
        let expected = THUMBNAIL_SIDE * THUMBNAIL_SIDE * 2;
        if scalar_u16_le.len() != expected {
            return Err(format!(
                "thumbnail {key:?} decoded {} bytes; expected {expected}",
                scalar_u16_le.len()
            ));
        }
        let pixels = scalar_u16_le
            .chunks_exact(2)
            .map(|bytes| {
                let value = u16::from_le_bytes([bytes[0], bytes[1]]) as f32 / u16::MAX as f32;
                let blue = (42.0 + value * 186.0) as u8;
                let green = (20.0 + value.sqrt() * 205.0) as u8;
                let red = (12.0 + value.powi(2) * 235.0) as u8;
                egui::Color32::from_rgb(red, green, blue)
            })
            .collect();
        let image = egui::ColorImage::new([THUMBNAIL_SIDE, THUMBNAIL_SIDE], pixels);
        let texture = context.load_texture(
            format!("thumbnail-{}-{}", key.x, token.sequence),
            image,
            egui::TextureOptions::LINEAR,
        );
        self.clock += 1;
        let bytes = THUMBNAIL_SIDE * THUMBNAIL_SIDE * 4;
        if let Some(previous) = self.entries.insert(
            key,
            ThumbnailEntry {
                texture,
                token,
                bytes,
                touched: self.clock,
            },
        ) {
            self.used -= previous.bytes;
        }
        self.used += bytes;

        let mut evicted = Vec::new();
        // One oversized item is retained so a valid completion always makes progress.
        while self.used > self.budget && self.entries.len() > 1 {
            let victim = self
                .entries
                .iter()
                .filter(|(candidate, _)| **candidate != key)
                .min_by_key(|(_, entry)| entry.touched)
                .map(|(candidate, _)| *candidate)
                .expect("more than one entry has an eviction candidate");
            let entry = self.entries.remove(&victim).expect("candidate exists");
            self.used -= entry.bytes;
            evicted.push((victim, entry.token));
        }
        Ok(evicted)
    }

    pub fn texture(&mut self, key: TileKey) -> Option<egui::TextureId> {
        self.clock += 1;
        let entry = self.entries.get_mut(&key)?;
        entry.touched = self.clock;
        Some(entry.texture.id())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn used(&self) -> usize {
        self.used
    }

    pub fn keys(&self) -> Vec<TileKey> {
        self.entries.keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polyorama_core::SourceId;

    fn key(x: u32) -> TileKey {
        TileKey {
            source: SourceId(2),
            level: 0,
            x,
            y: 0,
        }
    }

    fn token(sequence: u64) -> RequestToken {
        RequestToken {
            source_generation: 1,
            demand_epoch: 1,
            sequence,
        }
    }

    #[test]
    fn decoded_thumbnails_are_bounded_and_report_exact_eviction_tokens() {
        let context = egui::Context::default();
        let mut cache = ThumbnailCache::new(THUMBNAIL_SIDE * THUMBNAIL_SIDE * 4);
        let bytes = vec![0_u8; THUMBNAIL_SIDE * THUMBNAIL_SIDE * 2];
        assert!(
            cache
                .insert(&context, key(1), token(1), &bytes)
                .unwrap()
                .is_empty()
        );
        let evicted = cache.insert(&context, key(2), token(2), &bytes).unwrap();
        assert_eq!(evicted, vec![(key(1), token(1))]);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn invalid_worker_payload_is_rejected() {
        let context = egui::Context::default();
        let error = ThumbnailCache::default()
            .insert(&context, key(1), token(1), &[0, 1])
            .unwrap_err();
        assert!(error.contains("expected 8192"));
    }
}
