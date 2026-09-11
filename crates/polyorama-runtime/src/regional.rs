//! Bounded regional scheduling for external native or browser workers.
//!
//! One runtime is shared by all images, detail panes and gallery consumers. The
//! application submits the complete desired set once per generation. Source adapters
//! own transport, compressed caches, decoding and enforcement of request reservations.

use std::collections::BTreeMap;

use polyorama_core::{RegionDemand, RegionKey, RegionalFrame, RegionalPixels};
use serde::{Deserialize, Serialize};

use crate::RequestToken;

#[derive(Clone, Copy, Debug)]
pub struct RegionalRuntimeLimits {
    pub max_demands: usize,
    pub max_in_flight: usize,
    /// Shared by worker output reservations, decoded data and pending GPU uploads.
    pub decoded_bytes: usize,
}

impl Default for RegionalRuntimeLimits {
    fn default() -> Self {
        Self {
            max_demands: 4096,
            max_in_flight: 8,
            decoded_bytes: 64 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionalRequest {
    pub key: RegionKey,
    pub token: RequestToken,
    pub max_decoded_bytes: usize,
}

#[derive(Debug)]
pub struct RegionalUpload {
    pub key: RegionKey,
    pub token: RequestToken,
    pub pixels: RegionalPixels,
}

/// Opt-in upload carrying separately accounted binary validity.
#[derive(Debug)]
pub struct RegionalFrameUpload {
    pub key: RegionKey,
    pub token: RequestToken,
    pub pixels: RegionalFrame,
}
impl From<RegionalUpload> for RegionalFrameUpload {
    fn from(upload: RegionalUpload) -> Self {
        Self {
            key: upload.key,
            token: upload.token,
            pixels: upload.pixels.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionalReconcileError {
    StaleGeneration,
    TooManyDemands,
    InvalidDemand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionalCompletion {
    Accepted,
    Stale,
    InvalidPayload,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegionalRuntimeMetrics {
    pub desired: usize,
    pub deduplicated: usize,
    /// Includes cancelled requests until their worker actually stops or completes.
    pub in_flight: usize,
    pub worker_reserved_bytes: usize,
    pub decoded_bytes: usize,
    pub upload_bytes: usize,
    pub resident_regions: usize,
    pub completed: u64,
    pub cancelled: u64,
    pub stale: u64,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct RegionalAllocationDiagnostics {
    pub entries: usize,
    pub desired_entries: usize,
    pub outstanding_reservations: usize,
    pub upload_entries: usize,
    pub worker_reserved_bytes: usize,
    pub upload_bytes: usize,
    pub decoded_payloads: usize,
    pub sample_capacity_bytes: usize,
    pub validity_capacity_bytes: usize,
    pub resident_entries: usize,
    pub failed_entries: usize,
}

impl RegionalRuntimeMetrics {
    pub fn accounted_decoded_bytes(self) -> usize {
        self.worker_reserved_bytes + self.decoded_bytes + self.upload_bytes
    }
}

enum RegionState {
    InFlight,
    Decoded(RegionalFrame),
    Uploading,
    Resident,
    Failed,
}

struct RegionEntry {
    token: RequestToken,
    touched: u64,
    state: RegionState,
}

pub struct RegionalRuntime {
    limits: RegionalRuntimeLimits,
    generation: Option<u64>,
    sequence: u64,
    desired: BTreeMap<RegionKey, RegionDemand>,
    entries: BTreeMap<RegionKey, RegionEntry>,
    flights: BTreeMap<RequestToken, RegionalRequest>,
    uploads: BTreeMap<RequestToken, (RegionKey, usize)>,
    deduplicated: usize,
    completed: u64,
    cancelled: u64,
    stale: u64,
}

impl RegionalRuntime {
    pub fn new(limits: RegionalRuntimeLimits) -> Self {
        Self {
            limits,
            generation: None,
            sequence: 0,
            desired: BTreeMap::new(),
            entries: BTreeMap::new(),
            flights: BTreeMap::new(),
            uploads: BTreeMap::new(),
            deduplicated: 0,
            completed: 0,
            cancelled: 0,
            stale: 0,
        }
    }

    /// Atomically replace all consumers' desired state. Equal generations are idempotent
    /// updates; an older generation is rejected without changing state. Cancellation
    /// tickets must reach workers. Keep reservations until completion or a confirmed stop.
    /// Exact keys share a request; intersecting but unequal rectangles are never unioned.
    pub fn reconcile(
        &mut self,
        generation: u64,
        demands: impl IntoIterator<Item = RegionDemand>,
    ) -> Result<Vec<RegionalRequest>, RegionalReconcileError> {
        if self.generation.is_some_and(|current| generation < current) {
            return Err(RegionalReconcileError::StaleGeneration);
        }
        let mut desired = BTreeMap::<RegionKey, RegionDemand>::new();
        let mut duplicates = 0;
        for (index, demand) in demands.into_iter().enumerate() {
            if index >= self.limits.max_demands {
                return Err(RegionalReconcileError::TooManyDemands);
            }
            if !demand.key.is_valid()
                || demand.max_decoded_bytes == 0
                || demand.max_decoded_bytes > self.limits.decoded_bytes
            {
                return Err(RegionalReconcileError::InvalidDemand);
            }
            desired
                .entry(demand.key.clone())
                .and_modify(|current| {
                    duplicates += 1;
                    current.priority = current.priority.max(demand.priority);
                    current.max_decoded_bytes =
                        current.max_decoded_bytes.max(demand.max_decoded_bytes);
                })
                .or_insert(demand);
        }
        let mut cancellations = Vec::new();
        self.entries.retain(|key, entry| {
            if desired.contains_key(key) {
                entry.touched = generation;
                return true;
            }
            match entry.state {
                RegionState::InFlight => {
                    if let Some(request) = self.flights.get(&entry.token) {
                        cancellations.push(request.clone());
                    }
                    false
                }
                // Upload bytes stay charged until the renderer returns its acknowledgement.
                RegionState::Uploading | RegionState::Failed => false,
                RegionState::Decoded(_) | RegionState::Resident => true,
            }
        });
        self.cancelled += cancellations.len() as u64;
        self.desired = desired;
        self.generation = Some(generation);
        self.deduplicated = duplicates;
        Ok(cancellations)
    }

    /// Reserve output bytes before handing work to a bounded external worker pool.
    /// A cancelled job still occupies both a worker slot and its byte reservation.
    pub fn dispatch(&mut self) -> Vec<RegionalRequest> {
        let mut pending: Vec<_> = self
            .desired
            .values()
            .filter(|demand| !self.entries.contains_key(&demand.key))
            .cloned()
            .collect();
        pending.sort_by_key(|demand| std::cmp::Reverse((demand.priority, demand.key.reduction)));
        let mut requests = Vec::new();
        for demand in pending {
            if self.flights.len() >= self.limits.max_in_flight {
                break;
            }
            self.evict_unused_decoded(demand.max_decoded_bytes);
            let used = self.metrics().accounted_decoded_bytes();
            if demand.max_decoded_bytes > self.limits.decoded_bytes.saturating_sub(used) {
                continue;
            }
            self.sequence = self
                .sequence
                .checked_add(1)
                .expect("regional request token exhausted");
            let token = RequestToken {
                source_generation: 0,
                demand_epoch: self.generation.unwrap_or(0),
                sequence: self.sequence,
            };
            let request = RegionalRequest {
                key: demand.key,
                token,
                max_decoded_bytes: demand.max_decoded_bytes,
            };
            self.entries.insert(
                request.key.clone(),
                RegionEntry {
                    token,
                    touched: token.demand_epoch,
                    state: RegionState::InFlight,
                },
            );
            self.flights.insert(token, request.clone());
            requests.push(request);
        }
        requests
    }

    fn evict_unused_decoded(&mut self, required: usize) {
        while required
            > self
                .limits
                .decoded_bytes
                .saturating_sub(self.metrics().accounted_decoded_bytes())
        {
            let victim = self
                .entries
                .iter()
                .filter(|(key, entry)| {
                    !self.desired.contains_key(*key)
                        && matches!(entry.state, RegionState::Decoded(_))
                })
                .min_by_key(|(_, entry)| entry.touched)
                .map(|(key, _)| key.clone());
            let Some(key) = victim else { break };
            self.entries.remove(&key);
        }
    }

    pub fn complete(
        &mut self,
        request: &RegionalRequest,
        pixels: RegionalPixels,
    ) -> RegionalCompletion {
        self.complete_frame(request, pixels.into())
    }

    pub fn complete_frame(
        &mut self,
        request: &RegionalRequest,
        pixels: RegionalFrame,
    ) -> RegionalCompletion {
        if self.flights.get(&request.token) != Some(request) {
            self.stale += 1;
            return RegionalCompletion::Stale;
        }
        self.flights.remove(&request.token);
        if !self.is_current_flight(request) {
            self.stale += 1;
            return RegionalCompletion::Stale;
        }
        let valid = pixels.allocation_bytes() <= request.max_decoded_bytes
            && pixels.layout.channels() == request.key.components.len()
            && pixels.is_valid();
        let entry = self
            .entries
            .get_mut(&request.key)
            .expect("current regional flight");
        if !valid {
            entry.state = RegionState::Failed;
            return RegionalCompletion::InvalidPayload;
        }
        entry.state = RegionState::Decoded(pixels);
        self.completed += 1;
        RegionalCompletion::Accepted
    }

    fn is_current_flight(&self, request: &RegionalRequest) -> bool {
        self.desired.contains_key(&request.key)
            && self.entries.get(&request.key).is_some_and(|entry| {
                entry.token == request.token && matches!(entry.state, RegionState::InFlight)
            })
    }

    /// Report failure after worker-owned output and scratch have been released.
    pub fn fail(&mut self, request: &RegionalRequest) -> bool {
        if self.flights.get(&request.token) != Some(request) {
            return false;
        }
        self.flights.remove(&request.token);
        if self.is_current_flight(request) {
            self.entries
                .get_mut(&request.key)
                .expect("current regional flight")
                .state = RegionState::Failed;
        }
        true
    }

    /// Call only after a cancelled worker has stopped and released its output allocation.
    pub fn acknowledge_cancelled(&mut self, request: &RegionalRequest) -> bool {
        if self.flights.get(&request.token) == Some(request) && !self.is_current_flight(request) {
            self.flights.remove(&request.token);
            true
        } else {
            false
        }
    }

    pub fn retry(&mut self, key: &RegionKey) {
        if self
            .entries
            .get(key)
            .is_some_and(|entry| matches!(entry.state, RegionState::Failed))
        {
            self.entries.remove(key);
        }
    }

    /// Transfer one visible decoded result to the renderer. Its bytes remain charged
    /// until `finish_upload`, including time spent waiting in a renderer queue.
    pub fn take_decoded(&mut self) -> Option<RegionalUpload> {
        let upload = self.take_frame(false)?;
        Some(RegionalUpload {
            key: upload.key,
            token: upload.token,
            pixels: upload.pixels.pixels,
        })
    }

    /// Includes binary validity. Legacy take_decoded leaves masked frames queued
    /// rather than silently stripping their required display contract.
    pub fn take_decoded_frame(&mut self) -> Option<RegionalFrameUpload> {
        self.take_frame(true)
    }

    fn take_frame(&mut self, include_validity: bool) -> Option<RegionalFrameUpload> {
        let key = self
            .desired
            .values()
            .filter(|demand| {
                self.entries
                    .get(&demand.key)
                    .is_some_and(|entry| matches!(&entry.state, RegionState::Decoded(p) if include_validity || p.validity.is_none()))
            })
            .max_by_key(|demand| (demand.priority, demand.key.reduction))?
            .key
            .clone();
        let entry = self.entries.get_mut(&key)?;
        let RegionState::Decoded(pixels) =
            std::mem::replace(&mut entry.state, RegionState::Uploading)
        else {
            unreachable!()
        };
        self.uploads
            .insert(entry.token, (key.clone(), pixels.allocation_bytes()));
        Some(RegionalFrameUpload {
            key,
            token: entry.token,
            pixels,
        })
    }

    /// Check immediately before GPU admission; cancelled or replaced uploads are stale.
    pub fn is_upload_current(&self, key: &RegionKey, token: RequestToken) -> bool {
        self.desired.contains_key(key)
            && self.entries.get(key).is_some_and(|entry| {
                entry.token == token && matches!(entry.state, RegionState::Uploading)
            })
    }

    /// Acknowledge synchronous upload or rejection after dropping the CPU payload.
    /// A false result means the renderer must discard any stale texture it just created.
    pub fn finish_upload(&mut self, key: &RegionKey, token: RequestToken, resident: bool) -> bool {
        if !self
            .uploads
            .get(&token)
            .is_some_and(|(upload_key, _)| upload_key == key)
        {
            return false;
        }
        self.uploads.remove(&token);
        let current = self.is_upload_current(key, token);
        if current && resident {
            self.entries
                .get_mut(key)
                .expect("current regional upload")
                .state = RegionState::Resident;
        } else if self
            .entries
            .get(key)
            .is_some_and(|entry| entry.token == token)
        {
            self.entries.remove(key);
        }
        current
    }

    /// The renderer owns GPU eviction and must report the exact evicted token.
    pub fn evict_resident(&mut self, key: &RegionKey, token: RequestToken) -> bool {
        if self.entries.get(key).is_some_and(|entry| {
            entry.token == token && matches!(entry.state, RegionState::Resident)
        }) {
            self.entries.remove(key);
            true
        } else {
            false
        }
    }

    pub fn is_resident(&self, key: &RegionKey) -> bool {
        self.entries
            .get(key)
            .is_some_and(|entry| matches!(entry.state, RegionState::Resident))
    }

    /// Current owned payload capacity, separately from reservations and transferred uploads.
    /// Map node allocator overhead is unavailable; these values are not RSS.
    pub fn allocation_diagnostics(&self) -> RegionalAllocationDiagnostics {
        let m = self.metrics();
        let mut d = RegionalAllocationDiagnostics {
            entries: self.entries.len(),
            desired_entries: self.desired.len(),
            outstanding_reservations: self.flights.len(),
            upload_entries: self.uploads.len(),
            worker_reserved_bytes: m.worker_reserved_bytes,
            upload_bytes: m.upload_bytes,
            ..Default::default()
        };
        for entry in self.entries.values() {
            match &entry.state {
                RegionState::Decoded(frame) => {
                    d.decoded_payloads += 1;
                    d.sample_capacity_bytes +=
                        frame.samples.capacity() * std::mem::size_of::<u16>();
                    d.validity_capacity_bytes += frame.validity.as_ref().map_or(0, Vec::capacity);
                }
                RegionState::Resident => d.resident_entries += 1,
                RegionState::Failed => d.failed_entries += 1,
                _ => {}
            }
        }
        d
    }

    pub fn metrics(&self) -> RegionalRuntimeMetrics {
        RegionalRuntimeMetrics {
            desired: self.desired.len(),
            deduplicated: self.deduplicated,
            in_flight: self.flights.len(),
            worker_reserved_bytes: self
                .flights
                .values()
                .map(|request| request.max_decoded_bytes)
                .sum(),
            decoded_bytes: self
                .entries
                .values()
                .map(|entry| match &entry.state {
                    RegionState::Decoded(pixels) => pixels.allocation_bytes(),
                    _ => 0,
                })
                .sum(),
            upload_bytes: self.uploads.values().map(|(_, bytes)| bytes).sum(),
            resident_regions: self
                .entries
                .values()
                .filter(|entry| matches!(entry.state, RegionState::Resident))
                .count(),
            completed: self.completed,
            cancelled: self.cancelled,
            stale: self.stale,
        }
    }
}

/// Global byte/item accounting for a renderer-owned regional GPU cache.
/// Values own the actual resources; dropping an eviction releases them. The caller
/// reports returned keys/tokens to `RegionalRuntime::evict_resident`.
pub struct RegionalCache<T> {
    max_bytes: usize,
    max_items: usize,
    bytes: usize,
    clock: u64,
    entries: BTreeMap<RegionKey, CacheEntry<T>>,
}

struct CacheEntry<T> {
    token: RequestToken,
    bytes: usize,
    touched: u64,
    value: T,
}

pub struct RegionalEviction<T> {
    pub key: RegionKey,
    pub token: RequestToken,
    pub value: T,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionalCacheCapacity;

impl<T> RegionalCache<T> {
    pub fn new(max_bytes: usize, max_items: usize) -> Self {
        Self {
            max_bytes,
            max_items,
            bytes: 0,
            clock: 0,
            entries: BTreeMap::new(),
        }
    }

    /// Strict admission: even a single oversized resource is rejected intact.
    pub fn insert(
        &mut self,
        key: RegionKey,
        token: RequestToken,
        bytes: usize,
        value: T,
    ) -> Result<Vec<RegionalEviction<T>>, T> {
        let Ok(evicted) = self.make_room(&key, bytes) else {
            return Err(value);
        };
        self.clock = self.clock.saturating_add(1);
        self.bytes += bytes;
        self.entries.insert(
            key,
            CacheEntry {
                token,
                bytes,
                touched: self.clock,
                value,
            },
        );
        Ok(evicted)
    }

    /// Evict and drop resources before allocating their replacement. This avoids a
    /// transient old-plus-new GPU allocation above the configured logical byte cap.
    pub fn make_room(
        &mut self,
        key: &RegionKey,
        bytes: usize,
    ) -> Result<Vec<RegionalEviction<T>>, RegionalCacheCapacity> {
        if bytes == 0 || bytes > self.max_bytes || self.max_items == 0 {
            return Err(RegionalCacheCapacity);
        }
        let mut evicted = Vec::new();
        if let Some(previous) = self.remove(key) {
            evicted.push(previous);
        }
        while self.entries.len() >= self.max_items || bytes > self.max_bytes - self.bytes {
            let victim = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.touched)
                .map(|(key, _)| key.clone())
                .expect("non-empty cache over capacity");
            evicted.push(self.remove(&victim).expect("selected cache entry"));
        }
        Ok(evicted)
    }

    pub fn get(&mut self, key: &RegionKey) -> Option<&T> {
        self.clock = self.clock.saturating_add(1);
        let entry = self.entries.get_mut(key)?;
        entry.touched = self.clock;
        Some(&entry.value)
    }

    pub fn peek(&self, key: &RegionKey) -> Option<&T> {
        self.entries.get(key).map(|entry| &entry.value)
    }

    pub fn remove(&mut self, key: &RegionKey) -> Option<RegionalEviction<T>> {
        let entry = self.entries.remove(key)?;
        self.bytes -= entry.bytes;
        Some(RegionalEviction {
            key: key.clone(),
            token: entry.token,
            value: entry.value,
        })
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polyorama_core::{
        DemandPriority, ImageRegion, RegionConsumerId, RepresentationId, SampleLayout, SourceStage,
    };

    fn demand(x: u32, consumer: u64) -> RegionDemand {
        RegionDemand {
            consumer: RegionConsumerId(consumer),
            key: RegionKey {
                representation: RepresentationId([1; 32]),
                region: ImageRegion {
                    x,
                    y: 0,
                    width: 2,
                    height: 2,
                },
                reduction: 0,
                components: vec![0],
                stage: SourceStage(0),
            },
            priority: DemandPriority::Visible,
            max_decoded_bytes: 8,
        }
    }

    fn pixels() -> RegionalPixels {
        RegionalPixels {
            width: 2,
            height: 2,
            layout: SampleLayout::Scalar,
            precision: 11,
            samples: vec![1024; 4],
        }
    }

    fn runtime(bytes: usize, flights: usize) -> RegionalRuntime {
        RegionalRuntime::new(RegionalRuntimeLimits {
            decoded_bytes: bytes,
            max_in_flight: flights,
            max_demands: 100,
        })
    }

    #[test]
    fn scattered_demands_stay_independent_and_exact_overlaps_share_work() {
        let mut runtime = runtime(32, 4);
        let mut prefetch = demand(10_000, 2);
        prefetch.priority = DemandPriority::Prefetch;
        runtime
            .reconcile(1, [demand(0, 1), demand(0, 2), prefetch, demand(1, 3)])
            .unwrap();
        let requests = runtime.dispatch();
        assert_eq!(requests.len(), 3);
        assert_eq!(runtime.metrics().deduplicated, 1);
        assert_eq!(requests[2].key.region.x, 10_000);
        assert!(requests.iter().all(|request| request.key.region.width == 2));
    }

    #[test]
    fn cancellation_keeps_worker_and_byte_reservations_until_actual_stop() {
        let mut runtime = runtime(8, 1);
        runtime.reconcile(1, [demand(0, 1)]).unwrap();
        let old = runtime.dispatch().remove(0);
        assert_eq!(
            runtime.reconcile(2, [demand(50_000, 2)]).unwrap(),
            vec![old.clone()]
        );
        assert!(runtime.dispatch().is_empty());
        assert_eq!(runtime.metrics().accounted_decoded_bytes(), 8);
        assert!(runtime.acknowledge_cancelled(&old));
        let new = runtime.dispatch().remove(0);
        assert_eq!(runtime.complete(&old, pixels()), RegionalCompletion::Stale);
        assert_eq!(runtime.metrics().in_flight, 1);
        assert_eq!(
            runtime.complete(&new, pixels()),
            RegionalCompletion::Accepted
        );
    }

    #[test]
    fn identical_key_cancel_and_return_rejects_old_token() {
        let mut runtime = runtime(16, 2);
        runtime.reconcile(1, [demand(0, 1)]).unwrap();
        let old = runtime.dispatch().remove(0);
        runtime.reconcile(2, []).unwrap();
        runtime.reconcile(3, [demand(0, 2)]).unwrap();
        let new = runtime.dispatch().remove(0);
        assert_ne!(old.token, new.token);
        assert_eq!(runtime.complete(&old, pixels()), RegionalCompletion::Stale);
        assert_eq!(
            runtime.complete(&new, pixels()),
            RegionalCompletion::Accepted
        );
    }

    #[test]
    fn global_budget_covers_workers_decoded_and_renderer_transfer() {
        let mut runtime = runtime(16, 8);
        let mut other_image = demand(0, 3);
        other_image.key.representation = RepresentationId([2; 32]);
        runtime
            .reconcile(1, [demand(0, 1), demand(1, 2), other_image])
            .unwrap();
        let requests = runtime.dispatch();
        assert_eq!(requests.len(), 2);
        runtime.complete(&requests[0], pixels());
        let upload = runtime.take_decoded().unwrap();
        assert_eq!(runtime.metrics().accounted_decoded_bytes(), 16);
        assert!(runtime.dispatch().is_empty());
        let key = upload.key.clone();
        let token = upload.token;
        drop(upload);
        assert!(runtime.finish_upload(&key, token, true));
        assert_eq!(runtime.dispatch().len(), 1);
        assert_eq!(runtime.metrics().accounted_decoded_bytes(), 16);
    }

    #[test]
    fn resident_identity_reuses_across_consumers_but_never_representations_or_stages() {
        let mut runtime = runtime(32, 4);
        let original = demand(0, 1);
        runtime.reconcile(1, [original.clone()]).unwrap();
        let request = runtime.dispatch().remove(0);
        runtime.complete(&request, pixels());
        let upload = runtime.take_decoded().unwrap();
        drop(upload);
        runtime.finish_upload(&request.key, request.token, true);
        runtime.reconcile(2, [demand(0, 2)]).unwrap();
        assert!(runtime.dispatch().is_empty());
        let mut changed = original.clone();
        changed.key.representation = RepresentationId([2; 32]);
        let mut quality = original;
        quality.key.stage = SourceStage(1);
        runtime.reconcile(3, [changed, quality]).unwrap();
        assert_eq!(runtime.dispatch().len(), 2);
        assert!(!runtime.evict_resident(
            &request.key,
            RequestToken {
                sequence: 99,
                ..request.token
            }
        ));
        assert!(runtime.evict_resident(&request.key, request.token));
    }

    #[test]
    fn stale_upload_is_charged_until_rejection_and_cannot_become_resident() {
        let mut runtime = runtime(8, 1);
        runtime.reconcile(1, [demand(0, 1)]).unwrap();
        let request = runtime.dispatch().remove(0);
        runtime.complete(&request, pixels());
        let upload = runtime.take_decoded().unwrap();
        runtime.reconcile(2, [demand(0, 2)]).unwrap();
        assert!(runtime.is_upload_current(&upload.key, upload.token));
        runtime.reconcile(3, [demand(10, 2)]).unwrap();
        assert!(!runtime.is_upload_current(&upload.key, upload.token));
        assert!(runtime.dispatch().is_empty());
        drop(upload);
        assert!(!runtime.finish_upload(&request.key, request.token, true));
        assert_eq!(runtime.dispatch().len(), 1);
    }

    #[test]
    fn invalid_snapshot_is_atomic_and_payload_cannot_exceed_reservation() {
        let mut runtime = runtime(16, 2);
        runtime.reconcile(3, [demand(0, 1)]).unwrap();
        assert_eq!(
            runtime.reconcile(2, []),
            Err(RegionalReconcileError::StaleGeneration)
        );
        let mut invalid = demand(1, 1);
        invalid.key.region.width = 0;
        assert_eq!(
            runtime.reconcile(4, [invalid]),
            Err(RegionalReconcileError::InvalidDemand)
        );
        assert_eq!(runtime.metrics().desired, 1);
        let request = runtime.dispatch().remove(0);
        let mut oversized = pixels();
        oversized.height = 4;
        oversized.samples = vec![0; 8];
        assert_eq!(
            runtime.complete(&request, oversized),
            RegionalCompletion::InvalidPayload
        );
        assert_eq!(runtime.metrics().accounted_decoded_bytes(), 0);
        assert!(runtime.dispatch().is_empty());
        runtime.retry(&request.key);
        let retry = runtime.dispatch().remove(0);
        let mut spare_allocation = pixels();
        spare_allocation.samples.reserve_exact(32);
        assert_eq!(spare_allocation.byte_len(), 8);
        assert_eq!(
            runtime.complete(&retry, spare_allocation),
            RegionalCompletion::InvalidPayload
        );
    }

    #[test]
    fn gpu_cache_obeys_global_strict_bytes_and_lru_across_images() {
        let mut cache = RegionalCache::new(16, 2);
        let a = demand(0, 1).key;
        let mut b = a.clone();
        b.representation = RepresentationId([2; 32]);
        let c = demand(100, 2).key;
        let token = RequestToken {
            source_generation: 0,
            demand_epoch: 1,
            sequence: 1,
        };
        assert!(cache.insert(a.clone(), token, 8, "a").unwrap().is_empty());
        assert!(cache.insert(b.clone(), token, 8, "b").unwrap().is_empty());
        assert_eq!(cache.get(&a), Some(&"a"));
        let evicted = cache.insert(c, token, 8, "c").unwrap();
        assert_eq!(evicted[0].key, b);
        assert_eq!(cache.bytes(), 16);
        assert_eq!(
            cache.insert(a.clone(), token, 17, "too large").err(),
            Some("too large")
        );
        assert_eq!(cache.get(&a), Some(&"a"));
    }
}
