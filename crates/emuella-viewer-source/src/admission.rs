//! Opt-in exclusive working-set admission; ordinary multi-request clients are unchanged.
use super::*;

pub(super) const BIN_LIMIT: usize = 100_000;
pub(super) const MAX_BIN_LENGTH: u64 = 32 << 20;
const TILE_LIMIT: usize = 64;

/// One explicit request lifetime. Call `end_request` on every terminal path.
/// A stale token cannot release a newer request. Dropping the client releases all state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestScope(u64);

pub(super) struct ActiveRequest {
    pub(super) scope: RequestScope,
    pub(super) tid: String,
    pub(super) region: Region,
    pub(super) masks: Box<[(u8, u16)]>,
    pub(super) bins: Box<[(jpip::BinKey, u64)]>,
    pub(super) bytes: usize,
}
impl ActiveRequest {
    pub(super) fn bin_length(&self, key: jpip::BinKey) -> Option<u64> {
        self.bins
            .binary_search_by_key(&key, |b| b.0)
            .ok()
            .map(|i| self.bins[i].1)
    }
    fn metadata_bytes(&self) -> usize {
        // Boxed slices have exact lengths. Allocator overhead is not payload residency.
        std::mem::size_of::<Self>()
            + self.tid.capacity()
            + self.region.components.capacity() * std::mem::size_of::<u16>()
            + std::mem::size_of_val(&*self.masks)
            + std::mem::size_of_val(&*self.bins)
    }
}
impl SharedClient {
    /// Admit exactly one region after all its descriptors are present. The full
    /// demanded bin lengths come from authenticated descriptors, not response sizes.
    /// Reserves within the existing compressed+mask cap, including absent payloads.
    /// No pin is installed on failure. Interleaved regions require the legacy API
    /// or separate clients; this opt-in scope never silently replaces another scope.
    pub fn begin_request(&mut self, tid: &str, region: &Region) -> Result<RequestScope> {
        ensure!(
            self.active_request.is_none(),
            "request scope already active"
        );
        ensure!(
            self.missing_tiles(tid, region)?.is_empty(),
            "required descriptors incomplete"
        );
        let r = self.touch(tid)?;
        let tiles = region.tiles(&r.manifest.identity.profile)?;
        ensure!(
            tiles.len() <= TILE_LIMIT,
            "working-set admission: regional tile limit"
        );
        let ranges = bin_ranges(&r.index)?;
        let wanted = demands(&r.index, region)?;
        ensure!(
            wanted.len() <= BIN_LIMIT,
            "working-set admission: bin metadata limit"
        );
        let mut bins = Vec::with_capacity(wanted.len());
        let mut bytes = 0usize;
        for demand in wanted {
            let range = ranges
                .get(&demand.key)
                .ok_or_else(|| anyhow!("required bin range absent"))?;
            let length = range.end - range.start;
            ensure!(
                length <= MAX_BIN_LENGTH,
                "working-set admission: bin length limit"
            );
            bytes = bytes
                .checked_add(usize::try_from(length)?)
                .ok_or_else(|| anyhow!("working-set admission: size overflow"))?;
            bins.push((demand.key, length));
        }
        bins.sort_unstable_by_key(|b| b.0);
        let mut masks = Vec::new();
        let image_bytes = bytes;
        if let Some(validity) = &r.manifest.identity.validity {
            for tile in tiles {
                let length =
                    validity.byte_len(&r.manifest.identity.profile, tile, region.discard)?;
                bytes = bytes
                    .checked_add(length)
                    .ok_or_else(|| anyhow!("working-set admission: size overflow"))?;
                masks.push((region.discard, tile));
            }
        }
        ensure!(
            bytes <= self.limits.compressed_bytes,
            "working-set admission: required compressed bins and masks need {bytes} bytes, limit {} (image bins {image_bytes}, masks {})",
            self.limits.compressed_bytes,
            bytes - image_bytes
        );
        let scope = RequestScope(
            self.next_request
                .checked_add(1)
                .ok_or_else(|| anyhow!("request scope exhausted"))?,
        );
        let active = ActiveRequest {
            scope,
            tid: tid.into(),
            region: region.clone(),
            masks: masks.into_boxed_slice(),
            bins: bins.into_boxed_slice(),
            bytes,
        };
        // Previous partial bins can contain holes invisible to the public cache model.
        // Start this exclusive lifetime with complete dependencies only; continuations
        // inside it retain all fragments. This does not affect unscoped callers.
        let r = self.touch(tid)?;
        let mut retained = 0usize;
        let mut evictions = 0;
        for &(key, length) in &active.bins {
            if r.cache.is_complete(key) {
                let mut byte = [0];
                ensure!(
                    (length == 0 || r.cache.read(key, length - 1, &mut byte).is_ok())
                        && r.cache.read(key, length, &mut byte).is_err(),
                    "working-set admission: cached bin length mismatch"
                );
                retained += length as usize;
            }
        }
        for &(key, _) in &active.bins {
            if !r.cache.is_complete(key) && r.cache.evict(key) {
                evictions += 1;
            }
        }
        self.metrics.compressed_bin_evictions += evictions;
        let r = self.touch(tid)?;
        retained += active
            .masks
            .iter()
            .filter_map(|k| r.masks.get(k))
            .map(<[u8]>::len)
            .sum::<usize>();
        let missing = bytes - retained;
        self.next_request = scope.0;
        self.active_request = Some(active);
        let result: Result<()> = (|| {
            while self.resident_bytes().0.saturating_add(missing) > self.limits.compressed_bytes {
                self.evict_unprotected()?;
            }
            // Prevent the underlying cache's independent bin-count LRU from ever
            // evicting a dependency, including zero-length bins.
            loop {
                let r = self.entries.get(tid).unwrap();
                let active = self.active_request.as_ref().unwrap();
                let absent = active
                    .bins
                    .iter()
                    .filter(|(k, _)| !r.cache.is_complete(*k))
                    .count();
                if r.cache.bin_count().saturating_add(absent) <= BIN_LIMIT {
                    break;
                }
                self.evict_unprotected()?;
            }
            Ok(())
        })();
        if result.is_err() {
            self.active_request = None;
        }
        result?;
        Ok(scope)
    }
    pub fn end_request(&mut self, scope: RequestScope) {
        if self
            .active_request
            .as_ref()
            .is_some_and(|a| a.scope == scope)
        {
            let active = self.active_request.take().unwrap();
            let r = self.entries.get_mut(&active.tid).expect("active resident");
            // Cancelled/error responses can leave holes not advertised by model().
            // Reclaim these known partial keys at the lifetime boundary so they
            // cannot become invisible unrelated pressure on the next request.
            for &(key, _) in &active.bins {
                if !r.cache.is_complete(key) && r.cache.evict(key) {
                    self.metrics.compressed_bin_evictions += 1;
                }
            }
        }
    }
    /// Reserved payload and separately bounded retained pin metadata. The latter
    /// is honestly exposed, not charged a second time to compressed payload bytes.
    pub fn request_reservation(&self) -> (usize, usize) {
        self.active_request
            .as_ref()
            .map_or((0, 0), |a| (a.bytes, a.metadata_bytes()))
    }
    pub(super) fn check_request(&self, tid: &str, region: &Region) -> Result<()> {
        if let Some(a) = &self.active_request {
            ensure!(
                a.tid == tid && a.region == *region,
                "operation outside active request"
            );
        }
        Ok(())
    }
    fn evict_unprotected(&mut self) -> Result<()> {
        let active = self.active_request.as_ref().expect("scoped eviction");
        let mask = self
            .entries
            .iter()
            .flat_map(|(tid, r)| {
                r.masks
                    .keys()
                    .filter(move |k| tid != &active.tid || !active.masks.contains(k))
                    .map(move |k| (r.used, tid.clone(), k))
            })
            .min();
        if let Some((_, tid, mask)) = mask {
            self.entries.get_mut(&tid).unwrap().masks.remove(&mask);
            self.metrics.mask_evictions += 1;
            return Ok(());
        }
        if self.entries.len() > 1 {
            let tid = active.tid.clone();
            return self.evict_other(&tid);
        }
        let r = self.entries.get_mut(&active.tid).unwrap();
        if let Some(key) = r
            .cache
            .model()
            .keys()
            .find(|&&key| active.bin_length(key).is_none())
            .copied()
        {
            r.cache.evict(key);
            self.metrics.compressed_bin_evictions += 1;
            return Ok(());
        }
        Err(anyhow!(
            "working-set admission: unrelated sparse cache occupancy cannot be reclaimed within bounds"
        ))
    }
}
