//! The transport-neutral worker boundary. Codec work is called only by executors.
use anyhow::{Result, ensure};
use emuella_viewer_source::{ClientLimits, Manifest, Region, SharedClient};
use polyorama_core::{RegionalPixels, RepresentationId, SampleLayout};
use polyorama_runtime::RegionalRequest;
use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
pub const HTTP_LIMIT: usize = 8 << 20;
#[cfg(not(target_arch = "wasm32"))]
pub const CATALOGUE_LIMIT: usize = 16 << 20;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    pub request: RegionalRequest,
    pub manifest: Manifest,
}
impl Job {
    pub fn region(&self) -> Region {
        let k = &self.request.key;
        Region {
            x: k.region.x,
            y: k.region.y,
            width: k.region.width,
            height: k.region.height,
            discard: k.reduction,
            components: k.components.clone(),
        }
    }
}
/// Native uses one process-wide monotonic origin; browser realms use
/// performance.timeOrigin + performance.now(). These are diagnostic clocks.
pub fn pacing_now_ms() -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        static ORIGIN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        ORIGIN
            .get_or_init(std::time::Instant::now)
            .elapsed()
            .as_secs_f64()
            * 1000.
    }
    #[cfg(target_arch = "wasm32")]
    {
        let performance = web_sys::window()
            .expect("UI window")
            .performance()
            .expect("performance clock");
        performance.time_origin() + performance.now()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorkerTiming {
    pub started_ms: f64,
    pub finished_ms: f64,
    pub published_ms: f64,
    pub received_ms: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorkerMetrics {
    #[serde(default)]
    pub timing: Option<WorkerTiming>,
    pub decoded_evidence: Vec<DecodedEvidence>,
    pub decoded_evidence_dropped: u64,
    pub selected_block_coefficients: u64,
    pub peak_codec_workspace_bytes: u64,
    pub synthesis_coefficients_loaded: u64,
    pub synthesis_horizontal_values: u64,
    pub synthesis_vertical_values: u64,
    pub synthesis_lifting_updates: u64,
    pub synthesis_output_samples: u64,

    pub wasm_linear_bytes: Option<u64>,
    pub compressed_bytes: usize,
    pub peak_compressed_bytes: usize,
    pub peak_descriptor_bytes: usize,
    pub descriptor_bytes: usize,
    pub received_jpp_bytes: u64,
    pub received_descriptor_bytes: u64,
    pub decode_count: u64,
    pub decoded_pixels: u64,
    pub selected_code_blocks: u64,
    pub compressed_read_bytes: u64,
    pub cache_hits: u64,
    pub requests: u64,
    pub representation_evictions: u64,
    pub compressed_bin_evictions: u64,
    pub transferred_sample_bytes: u64,
    pub aborted: u64,
    pub retries: u64,
    pub elapsed_ms: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecodedEvidence {
    pub tid: String,
    pub region: Region,
    pub width: u32,
    pub height: u32,
    pub precision: u8,
    pub fnv1a64_u16le: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Event {
    Catalogue(Vec<Manifest>),
    Completed {
        request: RegionalRequest,
        pixels: RegionalPixels,
        metrics: WorkerMetrics,
    },
    Cancelled {
        request: RegionalRequest,
        metrics: WorkerMetrics,
    },
    Failed {
        request: Option<RegionalRequest>,
        error: String,
        metrics: WorkerMetrics,
    },
}
impl Event {
    pub fn metrics_mut(&mut self) -> Option<&mut WorkerMetrics> {
        match self {
            Self::Catalogue(_) => None,
            Self::Completed { metrics, .. }
            | Self::Cancelled { metrics, .. }
            | Self::Failed { metrics, .. } => Some(metrics),
        }
    }
}
pub fn representation(manifest: &Manifest) -> RepresentationId {
    let mut bytes = [0; 32];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&manifest.tid[i * 2..i * 2 + 2], 16).unwrap_or(0);
    }
    RepresentationId(bytes)
}
pub struct Engine {
    pub client: SharedClient,
    pub metrics: WorkerMetrics,
}
impl Engine {
    pub fn new(compressed_bytes: usize) -> Self {
        Self {
            client: SharedClient::new(ClientLimits {
                compressed_bytes,
                descriptor_bytes: 16 << 20,
                ..Default::default()
            }),
            metrics: WorkerMetrics::default(),
        }
    }
    pub fn snapshot(&self) -> WorkerMetrics {
        let mut m = self.metrics.clone();
        let (compressed, descriptors) = self.client.resident_bytes();
        m.compressed_bytes = compressed;
        m.descriptor_bytes = descriptors;
        let c = &self.client.metrics;
        m.received_jpp_bytes = c.received_jpp_bytes;
        m.representation_evictions = c.representation_evictions;
        m.compressed_bin_evictions = c.compressed_bin_evictions;
        m.peak_compressed_bytes = c.peak_compressed_bytes;
        m.peak_descriptor_bytes = c.peak_descriptor_bytes;
        m.received_descriptor_bytes = c.received_descriptor_bytes;
        m.decode_count = c.decode_count;
        m.decoded_pixels = c.decoded_pixels;
        m.selected_code_blocks = c.selected_code_blocks;
        m.compressed_read_bytes = c.compressed_read_bytes;
        m.selected_block_coefficients = c.selected_block_coefficients;
        m.peak_codec_workspace_bytes = c.peak_codec_workspace_bytes;
        m.synthesis_coefficients_loaded = c.synthesis_coefficients_loaded;
        m.synthesis_horizontal_values = c.synthesis_horizontal_values;
        m.synthesis_vertical_values = c.synthesis_vertical_values;
        m.synthesis_lifting_updates = c.synthesis_lifting_updates;
        m.synthesis_output_samples = c.synthesis_output_samples;

        m
    }
    pub fn decode(&mut self, job: &Job) -> Result<RegionalPixels> {
        let region = job.region();
        let scale = 1u64 << region.discard;
        let width = (u64::from(region.x) + u64::from(region.width)).div_ceil(scale)
            - u64::from(region.x) / scale;
        let height = (u64::from(region.y) + u64::from(region.height)).div_ceil(scale)
            - u64::from(region.y) / scale;
        // Reserve both the codec's planar output and the interleaving destination.
        let peak = width
            .checked_mul(height)
            .and_then(|v| v.checked_mul(region.components.len() as u64 * 4));
        ensure!(
            peak.is_some_and(|n| n <= job.request.max_decoded_bytes as u64),
            "decoded output reservation exceeded"
        );
        let result = self.client.decode(&job.manifest.tid, &region)?;
        let count = result.width as usize * result.height as usize;
        ensure!(
            result.planes.len() == region.components.len(),
            "wrong plane count"
        );
        ensure!(
            result
                .planes
                .iter()
                .all(|p| p.len() == count * if result.bits_per_sample == 8 { 1 } else { 2 }),
            "wrong sample plane length"
        );
        let mut samples = Vec::with_capacity(count * result.planes.len());
        for i in 0..count {
            for plane in &result.planes {
                samples.push(if result.bits_per_sample == 8 {
                    u16::from(plane[i])
                } else {
                    u16::from_le_bytes([plane[i * 2], plane[i * 2 + 1]])
                });
            }
        }
        let hash = samples
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .fold(0xcbf29ce484222325u64, |h, b| {
                (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
            });
        if self.metrics.decoded_evidence.len() == 64 {
            self.metrics.decoded_evidence.remove(0);
            self.metrics.decoded_evidence_dropped += 1;
        }
        self.metrics.decoded_evidence.push(DecodedEvidence {
            tid: job.manifest.tid.clone(),
            region: region.clone(),
            width: result.width,
            height: result.height,
            precision: result.bits_per_sample,
            fnv1a64_u16le: format!("{hash:016x}"),
        });
        Ok(RegionalPixels {
            width: result.width,
            height: result.height,
            layout: if result.planes.len() == 1 {
                SampleLayout::Scalar
            } else {
                SampleLayout::Rgb
            },
            precision: result.bits_per_sample,
            samples,
        })
    }
}
