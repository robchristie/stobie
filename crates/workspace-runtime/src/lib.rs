//! Demand reconciliation and CPU-only worker protocol.

use std::collections::{BTreeMap, VecDeque};

#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;
use web_time::Instant;

use serde::{Deserialize, Serialize};
use tracing::{debug, info_span};
use workspace_core::{
    DemandPriority, ResourceState, RuntimeMetrics, TILE_SIZE, TileDemand, TileKey,
    reconcile_demands,
};

pub const DEFAULT_CACHE_BUDGET: usize = 64 * 1024 * 1024;
pub const DEFAULT_UPLOAD_BUDGET: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecodeRequest {
    pub key: TileKey,
    pub generation: u64,
    pub compressed_lz4: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DecodeEvent {
    Completed {
        key: TileKey,
        generation: u64,
        scalar_u16_le: Vec<u8>,
        decode_ms: f64,
    },
    Failed {
        key: TileKey,
        generation: u64,
        message: String,
    },
}

pub fn synthetic_scalar_tile(key: TileKey) -> Vec<u16> {
    let dimension = if key.source.0 == 2 { 64 } else { TILE_SIZE };
    let mut values = Vec::with_capacity((dimension * dimension) as usize);
    let level_scale = 1_u32 << key.level;
    for local_y in 0..dimension {
        for local_x in 0..dimension {
            let x = (key.x * dimension + local_x) * level_scale;
            let y = (key.y * dimension + local_y) * level_scale;
            let gradient = ((x ^ y) & 0xffff) as u16;
            let rings = (((x as f64).hypot(y as f64) / 96.0).sin() * 8192.0 + 8192.0) as u16;
            let target = if (x / 2048 + y / 2048).is_multiple_of(17)
                && (x % 2048).abs_diff(1024) < 64
                && (y % 2048).abs_diff(1024) < 64
            {
                28_000
            } else {
                0
            };
            let noise = ((x.wrapping_mul(1664525) ^ y.wrapping_mul(1013904223) ^ key.level as u32)
                & 0xff) as u16;
            values.push(
                ((gradient as u32 / 2 + rings as u32 + target as u32 + noise as u32 * 8)
                    .min(u16::MAX as u32)) as u16,
            );
        }
    }
    values
}

pub fn compressed_tile_request(key: TileKey, generation: u64) -> DecodeRequest {
    let scalar = synthetic_scalar_tile(key);
    let bytes: &[u8] =
        unsafe { std::slice::from_raw_parts(scalar.as_ptr().cast(), scalar.len() * 2) };
    DecodeRequest {
        key,
        generation,
        compressed_lz4: lz4_flex::compress_prepend_size(bytes),
    }
}

pub fn decode(request: DecodeRequest) -> DecodeEvent {
    let _span = info_span!("worker_decode", ?request.key, request.generation).entered();
    let started = Instant::now();
    match lz4_flex::decompress_size_prepended(&request.compressed_lz4) {
        Ok(scalar_u16_le) => DecodeEvent::Completed {
            key: request.key,
            generation: request.generation,
            scalar_u16_le,
            decode_ms: started.elapsed().as_secs_f64() * 1000.0,
        },
        Err(error) => DecodeEvent::Failed {
            key: request.key,
            generation: request.generation,
            message: error.to_string(),
        },
    }
}

#[derive(Clone, Debug)]
struct ResourceEntry {
    state: ResourceState,
    generation: u64,
}

#[cfg(not(target_arch = "wasm32"))]
type RepaintWaker = Arc<dyn Fn() + Send + Sync>;

#[cfg(not(target_arch = "wasm32"))]
struct NativeWorker {
    requests: crossbeam_channel::Sender<DecodeRequest>,
    events: crossbeam_channel::Receiver<DecodeEvent>,
    waker: Arc<parking_lot::Mutex<Option<RepaintWaker>>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeWorker {
    fn start() -> Self {
        let (request_tx, request_rx) = crossbeam_channel::unbounded::<DecodeRequest>();
        let (event_tx, event_rx) = crossbeam_channel::unbounded();
        let waker = Arc::new(parking_lot::Mutex::new(None::<RepaintWaker>));
        let worker_waker = waker.clone();
        std::thread::Builder::new()
            .name("polyorama-tile-decoder".into())
            .spawn(move || {
                while let Ok(request) = request_rx.recv() {
                    if event_tx.send(decode(request)).is_err() {
                        break;
                    }
                    if let Some(wake) = worker_waker.lock().as_ref() {
                        wake();
                    }
                }
            })
            .expect("tile worker thread must start");
        Self {
            requests: request_tx,
            events: event_rx,
            waker,
        }
    }
}

pub struct Runtime {
    generation: u64,
    resources: BTreeMap<TileKey, ResourceEntry>,
    external_requests: VecDeque<DecodeRequest>,
    decoded: VecDeque<DecodeEvent>,
    failures: BTreeMap<TileKey, String>,
    #[cfg(not(target_arch = "wasm32"))]
    native: NativeWorker,
    pub metrics: RuntimeMetrics,
}

impl Default for Runtime {
    fn default() -> Self {
        Self {
            generation: 1,
            resources: BTreeMap::new(),
            external_requests: VecDeque::new(),
            decoded: VecDeque::new(),
            failures: BTreeMap::new(),
            #[cfg(not(target_arch = "wasm32"))]
            native: NativeWorker::start(),
            metrics: RuntimeMetrics::default(),
        }
    }
}

impl Runtime {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_repaint_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        *self.native.waker.lock() = Some(waker);
    }

    pub fn invalidate(&mut self) {
        self.generation += 1;
        self.resources.clear();
        self.external_requests.clear();
        self.decoded.clear();
        self.failures.clear();
        self.update_depths();
    }

    pub fn state(&self, key: TileKey) -> ResourceState {
        self.resources
            .get(&key)
            .map_or(ResourceState::Missing, |entry| entry.state)
    }

    pub fn reconcile(&mut self, demands: impl IntoIterator<Item = TileDemand>) {
        let _span = info_span!("demand_reconciliation").entered();
        let (demands, duplicates) = reconcile_demands(demands);
        self.metrics.total_demands = demands.len() + duplicates;
        self.metrics.duplicate_demands_removed = duplicates;
        self.metrics.visible_demands = demands
            .iter()
            .filter(|item| item.priority == DemandPriority::Visible)
            .count();
        self.metrics.prefetch_demands = demands.len() - self.metrics.visible_demands;
        for demand in demands {
            match self.resources.get(&demand.key).map(|entry| entry.state) {
                Some(
                    ResourceState::Queued
                    | ResourceState::Decoding
                    | ResourceState::Decoded
                    | ResourceState::Resident,
                ) => {
                    self.metrics.cache_hits += 1;
                }
                Some(ResourceState::Failed) => {}
                _ => {
                    self.metrics.cache_misses += 1;
                    self.resources.insert(
                        demand.key,
                        ResourceEntry {
                            state: ResourceState::Queued,
                            generation: demand.generation,
                        },
                    );
                    let request = compressed_tile_request(demand.key, demand.generation);
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        self.resources.get_mut(&demand.key).unwrap().state =
                            ResourceState::Decoding;
                        if self.native.requests.send(request).is_err() {
                            self.mark_failed(demand.key, "native worker stopped".into());
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    self.external_requests.push_back(request);
                }
            }
        }
        self.update_depths();
    }

    pub fn take_external_request(&mut self) -> Option<DecodeRequest> {
        let request = self.external_requests.pop_front()?;
        if let Some(entry) = self.resources.get_mut(&request.key) {
            entry.state = ResourceState::Decoding;
        }
        self.update_depths();
        Some(request)
    }

    pub fn accept_event(&mut self, event: DecodeEvent) {
        let (key, generation) = match &event {
            DecodeEvent::Completed {
                key, generation, ..
            }
            | DecodeEvent::Failed {
                key, generation, ..
            } => (*key, *generation),
        };
        if generation != self.generation
            || self
                .resources
                .get(&key)
                .is_some_and(|entry| entry.generation != generation)
        {
            self.metrics.stale_discarded += 1;
            return;
        }
        match &event {
            DecodeEvent::Completed { decode_ms, .. } => {
                if let Some(entry) = self.resources.get_mut(&key) {
                    entry.state = ResourceState::Decoded;
                }
                self.metrics.completed += 1;
                self.metrics.decode_latency_ms_median =
                    if self.metrics.decode_latency_ms_median == 0.0 {
                        *decode_ms
                    } else {
                        (self.metrics.decode_latency_ms_median + decode_ms) * 0.5
                    };
                self.decoded.push_back(event);
            }
            DecodeEvent::Failed { message, .. } => self.mark_failed(key, message.clone()),
        }
        self.update_depths();
    }

    pub fn poll(&mut self) -> usize {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let events: Vec<_> = self.native.events.try_iter().collect();
            let count = events.len();
            for event in events {
                self.accept_event(event);
            }
            count
        }
        #[cfg(target_arch = "wasm32")]
        {
            0
        }
    }

    pub fn pop_decoded(&mut self) -> Option<DecodeEvent> {
        self.decoded.pop_front()
    }

    pub fn mark_resident(&mut self, key: TileKey) {
        if let Some(entry) = self.resources.get_mut(&key) {
            entry.state = ResourceState::Resident;
        }
    }

    pub fn mark_evicted(&mut self, key: TileKey) {
        if let Some(entry) = self.resources.get_mut(&key) {
            entry.state = ResourceState::Missing;
        }
        self.metrics.evictions += 1;
        self.update_depths();
    }

    pub fn pending_decoded_bytes(&self) -> usize {
        self.decoded
            .iter()
            .map(|event| match event {
                DecodeEvent::Completed { scalar_u16_le, .. } => scalar_u16_le.len(),
                DecodeEvent::Failed { .. } => 0,
            })
            .sum()
    }

    fn mark_failed(&mut self, key: TileKey, message: String) {
        self.failures.insert(key, message);
        if let Some(entry) = self.resources.get_mut(&key) {
            entry.state = ResourceState::Failed;
        }
        self.metrics.failed += 1;
    }

    fn update_depths(&mut self) {
        self.metrics.queued = self
            .resources
            .values()
            .filter(|entry| entry.state == ResourceState::Queued)
            .count();
        self.metrics.in_flight = self
            .resources
            .values()
            .filter(|entry| entry.state == ResourceState::Decoding)
            .count();
        self.metrics.worker_queue_depth = self.metrics.queued + self.metrics.in_flight;
        debug!(
            queued = self.metrics.queued,
            in_flight = self.metrics.in_flight,
            "runtime depth"
        );
    }
}

#[derive(Clone, Debug)]
struct CacheEntry {
    bytes: usize,
    touched: u64,
}

#[derive(Clone, Debug)]
pub struct TileCache {
    budget: usize,
    used: usize,
    clock: u64,
    entries: BTreeMap<TileKey, CacheEntry>,
    pub evictions: u64,
}

impl TileCache {
    pub fn new(budget: usize) -> Self {
        Self {
            budget,
            used: 0,
            clock: 0,
            entries: BTreeMap::new(),
            evictions: 0,
        }
    }
    pub fn used(&self) -> usize {
        self.used
    }
    pub fn budget(&self) -> usize {
        self.budget
    }
    pub fn contains(&mut self, key: TileKey) -> bool {
        self.clock += 1;
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.touched = self.clock;
            true
        } else {
            false
        }
    }

    pub fn insert(&mut self, key: TileKey, bytes: usize) -> Vec<TileKey> {
        self.clock += 1;
        if let Some(old) = self.entries.insert(
            key,
            CacheEntry {
                bytes,
                touched: self.clock,
            },
        ) {
            self.used -= old.bytes;
        }
        self.used += bytes;
        let mut evicted = Vec::new();
        while self.used > self.budget && self.entries.len() > 1 {
            let victim = self
                .entries
                .iter()
                .filter(|(candidate, _)| **candidate != key)
                .min_by_key(|(_, entry)| entry.touched)
                .map(|(candidate, _)| *candidate);
            let Some(victim) = victim else { break };
            self.used -= self.entries.remove(&victim).unwrap().bytes;
            self.evictions += 1;
            evicted.push(victim);
        }
        evicted
    }
}

pub fn take_upload_budget(
    queue: &mut VecDeque<DecodeEvent>,
    byte_budget: usize,
) -> Vec<DecodeEvent> {
    let mut used = 0;
    let mut output = Vec::new();
    while let Some(front) = queue.front() {
        let bytes = match front {
            DecodeEvent::Completed { scalar_u16_le, .. } => scalar_u16_le.len(),
            DecodeEvent::Failed { .. } => 0,
        };
        if !output.is_empty() && used + bytes > byte_budget {
            break;
        }
        used += bytes;
        output.push(queue.pop_front().unwrap());
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use workspace_core::SourceId;

    fn key(x: u32) -> TileKey {
        TileKey {
            source: SourceId(1),
            level: 0,
            x,
            y: 0,
        }
    }

    #[test]
    fn compressed_payload_decodes_to_scalar_tile() {
        let event = decode(compressed_tile_request(key(0), 1));
        match event {
            DecodeEvent::Completed { scalar_u16_le, .. } => {
                assert_eq!(scalar_u16_le.len(), (TILE_SIZE * TILE_SIZE * 2) as usize)
            }
            _ => panic!("decode failed"),
        }
    }

    #[test]
    fn stale_generation_is_rejected() {
        let mut runtime = Runtime::default();
        runtime.resources.insert(
            key(0),
            ResourceEntry {
                state: ResourceState::Decoding,
                generation: 2,
            },
        );
        runtime.generation = 2;
        runtime.accept_event(DecodeEvent::Completed {
            key: key(0),
            generation: 1,
            scalar_u16_le: vec![0; 2],
            decode_ms: 1.0,
        });
        assert_eq!(runtime.metrics.stale_discarded, 1);
    }

    #[test]
    fn invalidation_resets_resources_for_new_generation_demand() {
        let mut runtime = Runtime::default();
        runtime.resources.insert(
            key(0),
            ResourceEntry {
                state: ResourceState::Resident,
                generation: runtime.generation(),
            },
        );
        runtime.failures.insert(key(1), "old failure".into());
        let old_generation = runtime.generation();

        runtime.invalidate();

        assert_eq!(runtime.generation(), old_generation + 1);
        assert_eq!(runtime.state(key(0)), ResourceState::Missing);
        assert!(runtime.failures.is_empty());

        runtime.reconcile([TileDemand {
            key: key(0),
            priority: DemandPriority::Visible,
            generation: runtime.generation(),
        }]);
        assert!(matches!(
            runtime.state(key(0)),
            ResourceState::Queued | ResourceState::Decoding
        ));
    }

    #[test]
    fn cache_evicts_deterministically_within_budget() {
        let mut cache = TileCache::new(20);
        cache.insert(key(0), 10);
        cache.insert(key(1), 10);
        let evicted = cache.insert(key(2), 10);
        assert_eq!(evicted, vec![key(0)]);
        assert!(cache.used() <= cache.budget());
    }

    #[test]
    fn cache_hits_refresh_recency_before_eviction() {
        let mut cache = TileCache::new(20);
        cache.insert(key(0), 10);
        cache.insert(key(1), 10);
        assert!(cache.contains(key(0)));
        assert_eq!(cache.insert(key(2), 10), vec![key(1)]);
        assert!(cache.contains(key(0)));
        assert!(cache.contains(key(2)));
    }

    #[test]
    fn one_tile_key_has_one_shared_residency_entry() {
        let mut cache = TileCache::new(20);
        assert!(cache.insert(key(0), 10).is_empty());
        assert!(cache.insert(key(0), 10).is_empty());
        assert_eq!(cache.used(), 10);
        assert!(cache.contains(key(0)));
        assert_eq!(cache.evictions, 0);
    }

    #[test]
    fn per_frame_upload_budget_is_respected() {
        let mut queue = VecDeque::from(
            (0..3)
                .map(|x| DecodeEvent::Completed {
                    key: key(x),
                    generation: 1,
                    scalar_u16_le: vec![0; 10],
                    decode_ms: 1.0,
                })
                .collect::<Vec<_>>(),
        );
        let selected = take_upload_budget(&mut queue, 20);
        assert_eq!(selected.len(), 2);
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn failed_resource_is_terminal_until_explicit_invalidation_policy_changes() {
        let mut runtime = Runtime::default();
        let demand = TileDemand {
            key: key(0),
            priority: DemandPriority::Visible,
            generation: runtime.generation(),
        };
        runtime.resources.insert(
            demand.key,
            ResourceEntry {
                state: ResourceState::Decoding,
                generation: demand.generation,
            },
        );
        runtime.accept_event(DecodeEvent::Failed {
            key: demand.key,
            generation: demand.generation,
            message: "fixture decode failure".into(),
        });
        let misses_before = runtime.metrics.cache_misses;

        runtime.reconcile([demand]);

        assert_eq!(runtime.state(demand.key), ResourceState::Failed);
        assert_eq!(runtime.metrics.cache_misses, misses_before);
        assert_eq!(runtime.metrics.failed, 1);
    }

    #[test]
    fn evicted_resident_tile_can_be_demanded_again() {
        let mut runtime = Runtime::default();
        let demand = TileDemand {
            key: key(0),
            priority: DemandPriority::Visible,
            generation: runtime.generation(),
        };
        runtime.resources.insert(
            demand.key,
            ResourceEntry {
                state: ResourceState::Resident,
                generation: demand.generation,
            },
        );

        runtime.mark_evicted(demand.key);
        assert_eq!(runtime.state(demand.key), ResourceState::Missing);
        runtime.reconcile([demand]);

        assert!(matches!(
            runtime.state(demand.key),
            ResourceState::Queued | ResourceState::Decoding
        ));
        assert_eq!(runtime.metrics.cache_misses, 1);
        assert_eq!(runtime.metrics.evictions, 1);
    }
}
