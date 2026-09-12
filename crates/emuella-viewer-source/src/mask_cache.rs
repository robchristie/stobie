//! Directly accounted slots for the authenticated level/tile mask catalogue.
use anyhow::{Result, ensure};
use std::mem::size_of;

#[derive(Clone)]
enum MaskSlot {
    Empty,
    AllInvalid,
    AllValid,
    Bitmap(Box<[u8]>),
}
impl MaskSlot {
    fn bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Empty => None,
            Self::AllInvalid => Some(&[0]),
            Self::AllValid => Some(&[1]),
            Self::Bitmap(bytes) => Some(bytes),
        }
    }
}

pub(super) struct MaskCache {
    tiles: usize,
    slots: Box<[MaskSlot]>,
    occupied: usize,
}
impl MaskCache {
    pub(super) const SLOT_BYTES: usize = size_of::<MaskSlot>();
    pub(super) const CONTAINER_BYTES: usize = size_of::<Self>();

    pub(super) fn planned_metadata(slots: usize) -> Result<usize> {
        slots
            .checked_mul(Self::SLOT_BYTES)
            .and_then(|bytes| bytes.checked_add(Self::CONTAINER_BYTES))
            .ok_or_else(|| anyhow::anyhow!("mask catalogue metadata overflow"))
    }
    /// The caller admits `planned_metadata` against the descriptor budget first.
    pub(super) fn new(tiles: usize, slots: usize) -> Self {
        Self {
            tiles,
            slots: vec![MaskSlot::Empty; slots].into_boxed_slice(),
            occupied: 0,
        }
    }
    pub(super) fn metadata_bytes(&self) -> usize {
        Self::CONTAINER_BYTES + self.slots.len() * Self::SLOT_BYTES
    }
    pub(super) fn slots(&self) -> usize {
        self.slots.len()
    }
    pub(super) fn len(&self) -> usize {
        self.occupied
    }
    pub(super) fn is_empty(&self) -> bool {
        self.occupied == 0
    }
    fn index(&self, &(discard, tile): &(u8, u16)) -> Option<usize> {
        if usize::from(tile) >= self.tiles {
            return None;
        }
        usize::from(discard)
            .checked_mul(self.tiles)?
            .checked_add(usize::from(tile))
            .filter(|&i| i < self.slots.len())
    }
    pub(super) fn get(&self, key: &(u8, u16)) -> Option<&[u8]> {
        self.slots.get(self.index(key)?)?.bytes()
    }
    pub(super) fn contains_key(&self, key: &(u8, u16)) -> bool {
        self.get(key).is_some()
    }
    pub(super) fn insert(&mut self, key: (u8, u16), bytes: &[u8], compact: bool) -> Result<()> {
        let index = self
            .index(&key)
            .ok_or_else(|| anyhow::anyhow!("mask slot absent"))?;
        ensure!(
            self.slots[index].bytes().is_none(),
            "mask slot already occupied"
        );
        // Constants have no per-payload allocation. Bitmap boxes have no spare capacity.
        self.slots[index] = match (compact, bytes) {
            (true, [0]) => MaskSlot::AllInvalid,
            (true, [1]) => MaskSlot::AllValid,
            _ => MaskSlot::Bitmap(bytes.into()),
        };
        self.occupied += 1;
        Ok(())
    }
    pub(super) fn remove(&mut self, key: &(u8, u16)) {
        if let Some(index) = self.index(key)
            && self.slots[index].bytes().is_some()
        {
            self.slots[index] = MaskSlot::Empty;
            self.occupied -= 1;
        }
    }
    pub(super) fn values(&self) -> impl Iterator<Item = &[u8]> {
        self.slots.iter().filter_map(MaskSlot::bytes)
    }
    pub(super) fn keys(&self) -> impl Iterator<Item = (u8, u16)> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.bytes().is_some())
            .map(|(i, _)| ((i / self.tiles) as u8, (i % self.tiles) as u16))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thousands_of_constants_do_not_grow_slot_metadata_or_allocate_bitmap_boxes() {
        let mut cache = MaskCache::new(2000, 2000);
        let metadata = cache.metadata_bytes();
        for tile in 0..2000 {
            cache.insert((0, tile), &[1], true).unwrap();
        }
        assert_eq!(cache.len(), 2000);
        assert_eq!(cache.metadata_bytes(), metadata);
        assert!(
            cache
                .slots
                .iter()
                .all(|slot| matches!(slot, MaskSlot::AllValid))
        );
        for tile in (0..2000).step_by(2) {
            cache.remove(&(0, tile));
        }
        assert_eq!(cache.len(), 1000);
        assert_eq!(cache.metadata_bytes(), metadata);
        for tile in (0..2000).step_by(2) {
            cache.insert((0, tile), &[0], true).unwrap();
        }
        assert_eq!(cache.len(), 2000);
        assert_eq!(cache.metadata_bytes(), metadata);
    }
    #[test]
    fn exact_slots_inline_constants_and_boxed_legacy_release_without_hidden_capacity() {
        let mut cache = MaskCache::new(2, 6);
        let metadata = MaskCache::planned_metadata(6).unwrap();
        assert_eq!(cache.metadata_bytes(), metadata);
        for (key, bytes, compact) in [
            ((0, 0), vec![0], true),
            ((0, 1), vec![1], true),
            ((1, 0), vec![2, 5, 7], true),
            ((2, 1), vec![1], false),
        ] {
            cache.insert(key, &bytes, compact).unwrap();
            assert_eq!(cache.get(&key), Some(bytes.as_slice()));
            assert_eq!(cache.metadata_bytes(), metadata);
        }
        assert!(matches!(cache.slots[0], MaskSlot::AllInvalid));
        assert!(matches!(cache.slots[1], MaskSlot::AllValid));
        assert!(matches!(cache.slots[5], MaskSlot::Bitmap(_)));
        assert_eq!(
            cache.keys().collect::<Vec<_>>(),
            vec![(0, 0), (0, 1), (1, 0), (2, 1)]
        );
        assert!(cache.get(&(0, 2)).is_none());
        assert!(cache.insert((3, 0), &[1], true).is_err());
        cache.remove(&(0, 1));
        assert_eq!(cache.len(), 3);
        assert_eq!(cache.metadata_bytes(), metadata);
        assert!(cache.get(&(0, 1)).is_none());
        cache.insert((0, 1), &[1], true).unwrap();
        assert_eq!(cache.len(), 4);
        assert!(MaskCache::planned_metadata(usize::MAX).is_err());
    }
}
