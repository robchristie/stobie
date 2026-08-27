//! Desired-set reconciliation and bounded CPU-only worker scheduling.

use polyorama_core::{
    DemandPriority, ResourceState, RuntimeMetrics, TILE_SIZE, TileDemand, TileKey, WorkerHealth,
    reconcile_demands,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;
use tracing::{debug, info_span};
use web_time::Instant;

pub const DEFAULT_CACHE_BUDGET: usize = 64 * 1024 * 1024;
pub const DEFAULT_UPLOAD_BUDGET: usize = 4 * 1024 * 1024;
pub const DEFAULT_SCHEDULER_CAPACITY: usize = 64;
pub const DEFAULT_EXTERNAL_CAPACITY: usize = 16;
pub const DEFAULT_NATIVE_QUEUE_CAPACITY: usize = 8;
pub const DEFAULT_NATIVE_EVENT_CAPACITY: usize = 8;
pub const DEFAULT_DECODED_CAPACITY_BYTES: usize = 8 * 1024 * 1024;
pub const DEFAULT_BROWSER_CREDITS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RequestToken {
    pub source_generation: u64,
    pub demand_epoch: u64,
    pub sequence: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecodeRequest {
    pub key: TileKey,
    pub token: RequestToken,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DecodeEvent {
    Completed {
        key: TileKey,
        token: RequestToken,
        scalar_u16_le: Vec<u8>,
        preparation_ms: f64,
        decode_ms: f64,
    },
    Failed {
        key: TileKey,
        token: RequestToken,
        preparation_ms: f64,
        decode_ms: f64,
        message: String,
    },
}
impl DecodeEvent {
    pub fn key(&self) -> TileKey {
        match self {
            Self::Completed { key, .. } | Self::Failed { key, .. } => *key,
        }
    }
    pub fn token(&self) -> RequestToken {
        match self {
            Self::Completed { token, .. } | Self::Failed { token, .. } => *token,
        }
    }
    pub fn bytes(&self) -> usize {
        match self {
            Self::Completed { scalar_u16_le, .. } => scalar_u16_le.len(),
            Self::Failed { .. } => 0,
        }
    }
}

/// Deterministic fixture source. It is deliberately CPU-only and invoked by workers only.
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
/// Portable encoding: never cast a `u16` allocation to bytes.
pub fn scalar_to_le_bytes(values: &[u16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 2);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}
/// Worker-side fixture preparation plus decode; reconciliation only admits compact requests.
pub fn prepare_and_decode(request: DecodeRequest) -> DecodeEvent {
    let _span = info_span!("worker_prepare_decode", ?request.key, ?request.token).entered();
    let prepared = Instant::now();
    let compressed =
        lz4_flex::compress_prepend_size(&scalar_to_le_bytes(&synthetic_scalar_tile(request.key)));
    let preparation_ms = prepared.elapsed().as_secs_f64() * 1000.0;
    let decode_started = Instant::now();
    match lz4_flex::decompress_size_prepended(&compressed) {
        Ok(scalar_u16_le) => DecodeEvent::Completed {
            key: request.key,
            token: request.token,
            scalar_u16_le,
            preparation_ms,
            decode_ms: decode_started.elapsed().as_secs_f64() * 1000.0,
        },
        Err(error) => DecodeEvent::Failed {
            key: request.key,
            token: request.token,
            preparation_ms,
            decode_ms: decode_started.elapsed().as_secs_f64() * 1000.0,
            message: error.to_string(),
        },
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeInitError {
    NativeWorkerStart(String),
}
#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub scheduler_capacity: usize,
    pub external_capacity: usize,
    pub native_queue_capacity: usize,
    pub native_event_capacity: usize,
    pub decoded_capacity_bytes: usize,
    pub browser_credits: usize,
}
impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            scheduler_capacity: DEFAULT_SCHEDULER_CAPACITY,
            external_capacity: DEFAULT_EXTERNAL_CAPACITY,
            native_queue_capacity: DEFAULT_NATIVE_QUEUE_CAPACITY,
            native_event_capacity: DEFAULT_NATIVE_EVENT_CAPACITY,
            decoded_capacity_bytes: DEFAULT_DECODED_CAPACITY_BYTES,
            browser_credits: DEFAULT_BROWSER_CREDITS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EntryState {
    Queued,
    Dispatched,
    Decoded,
    HandedToRenderer,
    Resident,
    Failed,
    Obsolete,
}
#[derive(Clone, Debug)]
struct ResourceEntry {
    token: RequestToken,
    priority: DemandPriority,
    state: EntryState,
    admitted: Instant,
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
    fn start(config: &RuntimeConfig) -> Result<Self, RuntimeInitError> {
        let (requests, receive_requests) = crossbeam_channel::bounded(config.native_queue_capacity);
        let (send_events, events) = crossbeam_channel::bounded(config.native_event_capacity);
        let waker = Arc::new(parking_lot::Mutex::new(None::<RepaintWaker>));
        let worker_waker = waker.clone();
        std::thread::Builder::new()
            .name("polyorama-tile-decoder".into())
            .spawn(move || {
                while let Ok(request) = receive_requests.recv() {
                    if send_events.send(prepare_and_decode(request)).is_err() {
                        break;
                    }
                    if let Some(wake) = worker_waker.lock().as_ref() {
                        wake();
                    }
                }
            })
            .map_err(|error| RuntimeInitError::NativeWorkerStart(error.to_string()))?;
        Ok(Self {
            requests,
            events,
            waker,
        })
    }
}

pub struct Runtime {
    config: RuntimeConfig,
    source_generation: u64,
    demand_epoch: u64,
    next_sequence: u64,
    desired: BTreeMap<TileKey, TileDemand>,
    resources: BTreeMap<TileKey, ResourceEntry>,
    decoded: VecDeque<DecodeEvent>,
    decoded_bytes: usize,
    external_requests: VecDeque<DecodeRequest>,
    browser_submitted: BTreeSet<RequestToken>,
    #[cfg(not(target_arch = "wasm32"))]
    native: Option<NativeWorker>,
    pub metrics: RuntimeMetrics,
}
impl Default for Runtime {
    fn default() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self::try_new(RuntimeConfig::default()).unwrap_or_else(|_| {
                Self::without_native(RuntimeConfig::default(), WorkerHealth::Unavailable)
            })
        }
        #[cfg(target_arch = "wasm32")]
        {
            Self::try_new(RuntimeConfig::default()).unwrap_or_else(|_| {
                Self::new_inner(RuntimeConfig::default(), WorkerHealth::Unavailable)
            })
        }
    }
}
impl Runtime {
    pub fn try_new(config: RuntimeConfig) -> Result<Self, RuntimeInitError> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let native = NativeWorker::start(&config)?;
            Ok(Self::new_inner(config, Some(native), WorkerHealth::Running))
        }
        #[cfg(target_arch = "wasm32")]
        {
            Ok(Self::new_inner(config, WorkerHealth::Starting))
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn without_native(config: RuntimeConfig, health: WorkerHealth) -> Self {
        Self::new_inner(config, None, health)
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn new_inner(
        config: RuntimeConfig,
        native: Option<NativeWorker>,
        health: WorkerHealth,
    ) -> Self {
        Self {
            config,
            source_generation: 1,
            demand_epoch: 0,
            next_sequence: 0,
            desired: BTreeMap::new(),
            resources: BTreeMap::new(),
            decoded: VecDeque::new(),
            decoded_bytes: 0,
            external_requests: VecDeque::new(),
            browser_submitted: BTreeSet::new(),
            native,
            metrics: RuntimeMetrics {
                worker_health: health,
                ..Default::default()
            },
        }
    }
    #[cfg(target_arch = "wasm32")]
    fn new_inner(config: RuntimeConfig, health: WorkerHealth) -> Self {
        Self {
            config,
            source_generation: 1,
            demand_epoch: 0,
            next_sequence: 0,
            desired: BTreeMap::new(),
            resources: BTreeMap::new(),
            decoded: VecDeque::new(),
            decoded_bytes: 0,
            external_requests: VecDeque::new(),
            browser_submitted: BTreeSet::new(),
            metrics: RuntimeMetrics {
                worker_health: health,
                ..Default::default()
            },
        }
    }
    pub fn generation(&self) -> u64 {
        self.source_generation
    }
    pub fn demand_epoch(&self) -> u64 {
        self.demand_epoch
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_repaint_waker(&mut self, waker: RepaintWaker) {
        if let Some(native) = &self.native {
            *native.waker.lock() = Some(waker);
        }
    }
    pub fn record_browser_worker_unavailable(&mut self, message: impl Into<String>) {
        self.metrics.worker_health = WorkerHealth::Unavailable;
        self.metrics.worker_failures += 1;
        self.metrics.last_worker_error = message.into();
        for entry in self.resources.values_mut() {
            if matches!(entry.state, EntryState::Queued | EntryState::Dispatched) {
                entry.state = EntryState::Failed;
                self.metrics.failed += 1;
            }
        }
        self.external_requests.clear();
        self.browser_submitted.clear();
        self.update_metrics();
    }
    pub fn record_browser_transport_failure(
        &mut self,
        request: DecodeRequest,
        message: impl Into<String>,
    ) {
        self.accept_event(DecodeEvent::Failed {
            key: request.key,
            token: request.token,
            preparation_ms: 0.0,
            decode_ms: 0.0,
            message: message.into(),
        });
    }
    pub fn invalidate(&mut self) {
        self.source_generation += 1;
        self.demand_epoch += 1;
        self.desired.clear();
        self.resources.clear();
        self.decoded.clear();
        self.decoded_bytes = 0;
        self.external_requests.clear();
        self.browser_submitted.clear();
        self.update_metrics();
    }
    pub fn state(&self, key: TileKey) -> ResourceState {
        match self.resources.get(&key).map(|entry| entry.state) {
            Some(EntryState::Queued) => ResourceState::Queued,
            Some(EntryState::Dispatched) => ResourceState::Decoding,
            Some(EntryState::Decoded | EntryState::HandedToRenderer) => ResourceState::Decoded,
            Some(EntryState::Resident) => ResourceState::Resident,
            Some(EntryState::Failed) => ResourceState::Failed,
            _ => ResourceState::Missing,
        }
    }
    pub fn token(&self, key: TileKey) -> Option<RequestToken> {
        self.resources.get(&key).map(|entry| entry.token)
    }
    /// Atomically replaces desired state. Existing tokens survive only for retained keys.
    pub fn reconcile(&mut self, demands: impl IntoIterator<Item = TileDemand>) {
        let _span = info_span!("demand_reconciliation").entered();
        self.demand_epoch += 1;
        let current_generation = self.source_generation;
        let mut stale = 0_u64;
        let (demands, duplicates) = reconcile_demands(demands.into_iter().filter(|demand| {
            let current = demand.generation == current_generation;
            stale += u64::from(!current);
            current
        }));
        self.metrics.stale_demands_rejected += stale;
        let new_desired: BTreeMap<_, _> = demands
            .into_iter()
            .map(|demand| (demand.key, demand))
            .collect();
        let previous_desired: BTreeSet<_> = self.desired.keys().copied().collect();
        let mut remove = Vec::new();
        for (key, entry) in &mut self.resources {
            if new_desired.contains_key(key) {
                continue;
            }
            match entry.state {
                EntryState::Queued | EntryState::Decoded | EntryState::Failed => remove.push(*key),
                EntryState::Dispatched | EntryState::HandedToRenderer => {
                    entry.state = EntryState::Obsolete
                }
                EntryState::Resident | EntryState::Obsolete => {}
            }
        }
        for key in remove {
            self.resources.remove(&key);
        }
        self.decoded
            .retain(|event| new_desired.contains_key(&event.key()));
        self.decoded_bytes = self.decoded.iter().map(DecodeEvent::bytes).sum();
        self.external_requests.retain(|request| {
            new_desired.contains_key(&request.key)
                && self.resources.get(&request.key).is_some_and(|entry| {
                    entry.token == request.token && entry.state == EntryState::Queued
                })
        });
        self.desired = new_desired;
        for demand in self.desired.values().copied() {
            if let Some(entry) = self.resources.get_mut(&demand.key) {
                entry.priority = demand.priority;
                if entry.state == EntryState::Resident && !previous_desired.contains(&demand.key) {
                    self.metrics.cache_hits += 1;
                }
            }
        }
        self.metrics.total_demands = self.desired.len() + duplicates;
        self.metrics.duplicate_demands_removed = duplicates;
        self.metrics.visible_demands = self
            .desired
            .values()
            .filter(|demand| demand.priority == DemandPriority::Visible)
            .count();
        self.metrics.prefetch_demands = self.desired.len() - self.metrics.visible_demands;
        self.admit_desired();
        self.dispatch_ready();
        self.update_metrics();
    }
    fn outstanding_count(&self) -> usize {
        self.resources
            .values()
            .filter(|entry| {
                matches!(
                    entry.state,
                    EntryState::Queued
                        | EntryState::Dispatched
                        | EntryState::Decoded
                        | EntryState::HandedToRenderer
                        | EntryState::Obsolete
                )
            })
            .count()
    }
    fn admit_desired(&mut self) {
        let available = self
            .config
            .scheduler_capacity
            .saturating_sub(self.outstanding_count());
        let mut candidates: Vec<_> = self
            .desired
            .values()
            .filter(|demand| !self.resources.contains_key(&demand.key))
            .copied()
            .collect();
        candidates.sort_by_key(|demand| {
            std::cmp::Reverse((
                demand.priority,
                demand.key.level,
                demand.key.x,
                demand.key.y,
            ))
        });
        for demand in candidates.into_iter().take(available) {
            self.next_sequence += 1;
            let token = RequestToken {
                source_generation: self.source_generation,
                demand_epoch: self.demand_epoch,
                sequence: self.next_sequence,
            };
            self.resources.insert(
                demand.key,
                ResourceEntry {
                    token,
                    priority: demand.priority,
                    state: EntryState::Queued,
                    admitted: Instant::now(),
                },
            );
            self.metrics.cache_misses += 1;
        }
    }
    fn queued_requests(&self) -> Vec<DecodeRequest> {
        let mut output: Vec<_> = self
            .resources
            .iter()
            .filter_map(|(key, entry)| {
                (entry.state == EntryState::Queued && self.desired.contains_key(key)).then_some(
                    DecodeRequest {
                        key: *key,
                        token: entry.token,
                    },
                )
            })
            .collect();
        output.sort_by_key(|request| {
            let entry = &self.resources[&request.key];
            std::cmp::Reverse((
                entry.priority,
                request.key.level,
                u64::MAX - request.token.sequence,
            ))
        });
        output
    }
    fn dispatch_ready(&mut self) {
        for request in self
            .queued_requests()
            .into_iter()
            .take(self.config.scheduler_capacity)
        {
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(native) = &self.native {
                match native.requests.try_send(request.clone()) {
                    Ok(()) => {
                        self.resources.get_mut(&request.key).unwrap().state =
                            EntryState::Dispatched;
                        continue;
                    }
                    Err(crossbeam_channel::TrySendError::Full(_)) => {
                        self.metrics.deferred_dispatches += 1;
                        break;
                    }
                    Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                        self.metrics.worker_health = WorkerHealth::Stopped;
                        self.metrics.worker_failures += 1;
                        self.resources.get_mut(&request.key).unwrap().state = EntryState::Failed;
                        continue;
                    }
                }
            }
            if self.external_requests.len() >= self.config.external_capacity
                || self.browser_submitted.len() + self.external_requests.len()
                    >= self.config.browser_credits
            {
                self.metrics.deferred_dispatches += 1;
                break;
            }
            if self
                .external_requests
                .iter()
                .any(|queued| queued.token == request.token)
            {
                continue;
            }
            self.external_requests.push_back(request);
        }
    }
    /// Browser-only pull. Taking a request consumes one fixed browser credit.
    pub fn take_external_request(&mut self) -> Option<DecodeRequest> {
        let request = self.external_requests.pop_front()?;
        if self.browser_submitted.len() >= self.config.browser_credits {
            self.external_requests.push_front(request);
            return None;
        }
        let entry = self.resources.get_mut(&request.key)?;
        if entry.token != request.token
            || entry.state != EntryState::Queued
            || !self.desired.contains_key(&request.key)
        {
            return None;
        }
        entry.state = EntryState::Dispatched;
        self.browser_submitted.insert(request.token);
        self.dispatch_ready();
        self.update_metrics();
        Some(request)
    }
    pub fn accept_event(&mut self, event: DecodeEvent) {
        if self.metrics.worker_health == WorkerHealth::Starting {
            self.metrics.worker_health = WorkerHealth::Running;
        }
        let key = event.key();
        let token = event.token();
        self.browser_submitted.remove(&token);
        let Some(entry) = self.resources.get_mut(&key) else {
            self.metrics.completion_unknown += 1;
            self.metrics.stale_discarded += 1;
            self.dispatch_ready();
            self.update_metrics();
            return;
        };
        if entry.token != token {
            self.metrics.completion_superseded += 1;
            self.metrics.stale_discarded += 1;
            self.dispatch_ready();
            self.update_metrics();
            return;
        }
        if !self.desired.contains_key(&key) || entry.state == EntryState::Obsolete {
            self.metrics.completion_obsolete += 1;
            self.metrics.stale_discarded += 1;
            self.resources.remove(&key);
            self.admit_desired();
            self.dispatch_ready();
            self.update_metrics();
            return;
        }
        if entry.state != EntryState::Dispatched {
            self.metrics.completion_duplicate += 1;
            self.metrics.stale_discarded += 1;
            self.dispatch_ready();
            self.update_metrics();
            return;
        }
        match &event {
            DecodeEvent::Completed {
                preparation_ms,
                decode_ms,
                ..
            } => {
                let end_to_end_ms = entry.admitted.elapsed().as_secs_f64() * 1000.0;
                let bytes = event.bytes();
                if self.decoded_bytes + bytes > self.config.decoded_capacity_bytes
                    && !self.decoded.is_empty()
                {
                    self.metrics.deferred_completions += 1;
                    self.metrics.failed += 1;
                    self.metrics.last_worker_error =
                        "decoded hand-off queue capacity exhausted".into();
                    entry.state = EntryState::Failed;
                    self.admit_desired();
                    self.dispatch_ready();
                    self.update_metrics();
                    return;
                }
                entry.state = EntryState::Decoded;
                self.decoded_bytes += bytes;
                self.metrics.completed += 1;
                self.record_latency(*preparation_ms, *decode_ms, end_to_end_ms);
                self.decoded.push_back(event);
            }
            DecodeEvent::Failed { message, .. } => {
                entry.state = EntryState::Failed;
                self.metrics.failed += 1;
                self.metrics.worker_failures += 1;
                self.metrics.last_worker_error = message.clone();
            }
        }
        self.admit_desired();
        self.dispatch_ready();
        self.update_metrics();
    }
    pub fn poll(&mut self) -> usize {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let events: Vec<_> = self
                .native
                .as_ref()
                .map(|native| native.events.try_iter().collect())
                .unwrap_or_default();
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
    pub fn take_decoded_for_renderer(&mut self) -> Option<DecodeEvent> {
        let event = self.decoded.pop_front()?;
        self.decoded_bytes -= event.bytes();
        if let Some(entry) = self.resources.get_mut(&event.key())
            && entry.token == event.token()
            && entry.state == EntryState::Decoded
        {
            entry.state = EntryState::HandedToRenderer;
        }
        self.update_metrics();
        Some(event)
    }
    pub fn mark_resident(&mut self, key: TileKey, token: RequestToken) {
        let obsolete = self
            .resources
            .get(&key)
            .is_some_and(|entry| entry.token == token && entry.state == EntryState::Obsolete);
        if obsolete {
            self.resources.remove(&key);
        } else if let Some(entry) = self.resources.get_mut(&key) {
            if entry.token == token && entry.state == EntryState::HandedToRenderer {
                entry.state = EntryState::Resident;
            } else {
                self.metrics.residency_rejected += 1;
            }
        } else {
            self.metrics.residency_rejected += 1;
        }
        self.admit_desired();
        self.dispatch_ready();
        self.update_metrics();
    }
    pub fn mark_handoff_failed(
        &mut self,
        key: TileKey,
        token: RequestToken,
        message: impl Into<String>,
    ) {
        let obsolete = self
            .resources
            .get(&key)
            .is_some_and(|entry| entry.token == token && entry.state == EntryState::Obsolete);
        if obsolete {
            self.resources.remove(&key);
        } else if let Some(entry) = self.resources.get_mut(&key) {
            if entry.token == token && entry.state == EntryState::HandedToRenderer {
                entry.state = EntryState::Failed;
                self.metrics.failed += 1;
                self.metrics.last_worker_error = message.into();
            } else {
                self.metrics.residency_rejected += 1;
            }
        } else {
            self.metrics.residency_rejected += 1;
        }
        self.admit_desired();
        self.dispatch_ready();
        self.update_metrics();
    }
    pub fn mark_evicted(&mut self, key: TileKey, token: RequestToken) {
        let mut remove = false;
        if let Some(entry) = self.resources.get_mut(&key) {
            if entry.token == token && entry.state == EntryState::Resident {
                remove = true;
                self.metrics.evictions += 1;
            } else {
                self.metrics.residency_rejected += 1;
            }
        } else {
            self.metrics.residency_rejected += 1;
        }
        if remove {
            self.resources.remove(&key);
        }
        self.admit_desired();
        self.dispatch_ready();
        self.update_metrics();
    }
    fn record_latency(&mut self, preparation: f64, decode: f64, end_to_end: f64) {
        self.metrics.preparation_latency_ms.record(preparation);
        self.metrics.decode_latency_ms.record(decode);
        self.metrics.end_to_end_latency_ms.record(end_to_end);
        self.metrics.decode_latency_ms_median = self.metrics.decode_latency_ms.p50;
    }
    fn update_metrics(&mut self) {
        self.metrics.desired = self.desired.len();
        self.metrics.queued = self
            .resources
            .values()
            .filter(|entry| entry.state == EntryState::Queued)
            .count();
        self.metrics.in_flight = self
            .resources
            .values()
            .filter(|entry| entry.state == EntryState::Dispatched)
            .count();
        self.metrics.decoded = self
            .resources
            .values()
            .filter(|entry| entry.state == EntryState::Decoded)
            .count();
        self.metrics.decoded_bytes = self.decoded_bytes;
        self.metrics.worker_queue_depth = self.external_requests.len();
        self.metrics.scheduler_capacity = self.config.scheduler_capacity;
        self.metrics.external_queue_capacity = self.config.external_capacity;
        self.metrics.browser_credit_capacity = self.config.browser_credits;
        self.metrics.browser_credits_in_use = self.browser_submitted.len();
        self.metrics.scheduler_high_water = self
            .metrics
            .scheduler_high_water
            .max(self.outstanding_count());
        self.metrics.external_queue_high_water = self
            .metrics
            .external_queue_high_water
            .max(self.external_requests.len());
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(native) = &self.native {
            self.metrics.native_queue_depth = native.requests.len();
            self.metrics.native_queue_high_water = self
                .metrics
                .native_queue_high_water
                .max(native.requests.len());
        }
        debug!(
            desired = self.metrics.desired,
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
    pub fn clear(&mut self) {
        self.used = 0;
        self.entries.clear();
    }
    /// A single oversized resident entry is retained; adding another evicts it as normal.
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
/// A first item may exceed the budget so a queue always makes forward progress.
pub fn take_upload_budget(
    queue: &mut VecDeque<DecodeEvent>,
    byte_budget: usize,
) -> Vec<DecodeEvent> {
    let mut used = 0;
    let mut output = Vec::new();
    while let Some(front) = queue.front() {
        let bytes = front.bytes();
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
    use polyorama_core::SourceId;
    fn key(x: u32) -> TileKey {
        TileKey {
            source: SourceId(1),
            level: 0,
            x,
            y: 0,
        }
    }
    fn demand(runtime: &Runtime, key: TileKey, priority: DemandPriority) -> TileDemand {
        TileDemand {
            key,
            priority,
            generation: runtime.generation(),
        }
    }
    fn event(key: TileKey, token: RequestToken) -> DecodeEvent {
        DecodeEvent::Completed {
            key,
            token,
            scalar_u16_le: vec![0; 2],
            preparation_ms: 2.0,
            decode_ms: 4.0,
        }
    }
    #[test]
    fn compact_request_prepares_off_scheduler_and_portably_encodes_le() {
        assert_eq!(
            scalar_to_le_bytes(&[0x1234, 0xabcd]),
            vec![0x34, 0x12, 0xcd, 0xab]
        );
        let request = DecodeRequest {
            key: key(0),
            token: RequestToken {
                source_generation: 1,
                demand_epoch: 1,
                sequence: 1,
            },
        };
        assert!(
            matches!(prepare_and_decode(request), DecodeEvent::Completed { scalar_u16_le, .. } if scalar_u16_le.len() == (TILE_SIZE*TILE_SIZE*2) as usize)
        );
    }
    #[test]
    fn missing_obsolete_superseded_and_duplicate_completions_are_rejected() {
        let mut runtime = Runtime::default();
        let unknown = RequestToken {
            source_generation: 1,
            demand_epoch: 1,
            sequence: 99,
        };
        runtime.accept_event(event(key(9), unknown));
        assert_eq!(runtime.metrics.completion_unknown, 1);
        runtime.reconcile([demand(&runtime, key(0), DemandPriority::Visible)]);
        let token = runtime.token(key(0)).unwrap();
        runtime.accept_event(event(
            key(0),
            RequestToken {
                sequence: token.sequence + 1,
                ..token
            },
        ));
        assert_eq!(runtime.metrics.completion_superseded, 1);
        runtime.reconcile([]);
        runtime.accept_event(event(key(0), token));
        assert_eq!(runtime.metrics.completion_obsolete, 1);
        runtime.reconcile([demand(&runtime, key(1), DemandPriority::Visible)]);
        let token = runtime.token(key(1)).unwrap();
        runtime.accept_event(event(key(1), token));
        runtime.accept_event(event(key(1), token));
        assert_eq!(runtime.metrics.completion_duplicate, 1);
    }
    #[test]
    fn retained_tokens_are_stable_and_priority_is_replaced() {
        let mut runtime = Runtime::default();
        #[cfg(not(target_arch = "wasm32"))]
        {
            runtime.native = None;
        }
        runtime.reconcile([demand(&runtime, key(0), DemandPriority::Prefetch)]);
        let token = runtime.token(key(0)).unwrap();
        runtime.reconcile([demand(&runtime, key(0), DemandPriority::Visible)]);
        assert_eq!(runtime.token(key(0)), Some(token));
        assert_eq!(runtime.resources[&key(0)].priority, DemandPriority::Visible);
        assert_eq!(runtime.metrics.visible_demands, 1);
    }
    #[test]
    fn browser_credits_and_external_queue_are_bounded() {
        let mut runtime = Runtime::try_new(RuntimeConfig {
            browser_credits: 1,
            external_capacity: 1,
            native_queue_capacity: 0,
            ..RuntimeConfig::default()
        })
        .unwrap();
        #[cfg(not(target_arch = "wasm32"))]
        {
            runtime.native = None;
        }
        runtime.reconcile([
            demand(&runtime, key(0), DemandPriority::Visible),
            demand(&runtime, key(1), DemandPriority::Visible),
        ]);
        let first = runtime.take_external_request().unwrap();
        assert!(runtime.take_external_request().is_none());
        runtime.accept_event(event(first.key, first.token));
        assert!(runtime.metrics.browser_credits_in_use <= 1);
    }
    #[test]
    fn scheduler_admission_is_bounded_and_backfills_after_residency() {
        let mut runtime = Runtime::try_new(RuntimeConfig {
            scheduler_capacity: 4,
            external_capacity: 4,
            browser_credits: 4,
            ..RuntimeConfig::default()
        })
        .unwrap();
        #[cfg(not(target_arch = "wasm32"))]
        {
            runtime.native = None;
        }
        let demands: Vec<_> = (0..100)
            .map(|index| demand(&runtime, key(index), DemandPriority::Visible))
            .collect();
        runtime.reconcile(demands);
        assert_eq!(runtime.metrics.desired, 100);
        assert_eq!(runtime.outstanding_count(), 4);
        assert!(runtime.metrics.scheduler_high_water <= 4);

        let request = runtime.take_external_request().unwrap();
        runtime.accept_event(event(request.key, request.token));
        let decoded = runtime.take_decoded_for_renderer().unwrap();
        runtime.mark_resident(decoded.key(), decoded.token());
        assert_eq!(runtime.outstanding_count(), 4);
        assert_eq!(runtime.metrics.cache_misses, 5);
    }

    #[test]
    fn stale_generation_demands_are_rejected_before_admission() {
        let mut runtime = Runtime::default();
        #[cfg(not(target_arch = "wasm32"))]
        {
            runtime.native = None;
        }
        let stale_generation = runtime.generation();
        runtime.invalidate();
        runtime.reconcile([TileDemand {
            key: key(0),
            priority: DemandPriority::Visible,
            generation: stale_generation,
        }]);
        assert_eq!(runtime.metrics.desired, 0);
        assert_eq!(runtime.metrics.stale_demands_rejected, 1);
        assert!(runtime.token(key(0)).is_none());
    }

    #[test]
    fn resident_re_demand_is_counted_as_a_cache_hit() {
        let mut runtime = Runtime::default();
        #[cfg(not(target_arch = "wasm32"))]
        {
            runtime.native = None;
        }
        runtime.reconcile([demand(&runtime, key(0), DemandPriority::Visible)]);
        let request = runtime.take_external_request().unwrap();
        runtime.accept_event(event(request.key, request.token));
        let decoded = runtime.take_decoded_for_renderer().unwrap();
        runtime.mark_resident(decoded.key(), decoded.token());
        runtime.reconcile([]);
        runtime.reconcile([demand(&runtime, key(0), DemandPriority::Visible)]);
        assert_eq!(runtime.metrics.cache_hits, 1);
        assert_eq!(runtime.state(key(0)), ResourceState::Resident);
    }
    #[test]
    fn actual_percentiles_use_bounded_samples() {
        let mut summary = polyorama_core::LatencySummary::default();
        for value in 1..=200 {
            summary.record(value as f64);
        }
        assert_eq!(summary.samples, 128);
        assert!(summary.p50 >= 136.0 && summary.p50 <= 137.0);
        assert_eq!(summary.p95, 194.0);
    }
    #[test]
    fn cache_and_upload_allow_one_oversized_item() {
        let mut cache = TileCache::new(10);
        assert!(cache.insert(key(0), 20).is_empty());
        assert_eq!(cache.used(), 20);
        let token = RequestToken {
            source_generation: 1,
            demand_epoch: 1,
            sequence: 1,
        };
        let mut queue = VecDeque::from([
            event(key(0), token),
            event(
                key(1),
                RequestToken {
                    sequence: 2,
                    ..token
                },
            ),
        ]);
        assert_eq!(take_upload_budget(&mut queue, 1).len(), 1);
        assert_eq!(queue.len(), 1);
    }
}
