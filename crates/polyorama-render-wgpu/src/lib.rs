//! Typed renderer requests and renderer-owned wgpu resources.

use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};

use bytemuck::{Pod, Zeroable};
use parking_lot::Mutex;
use polyorama_core::{Camera, PaneId, PhysicalPoint, RenderMetrics, SourceId, TileKey};
use polyorama_runtime::{
    DEFAULT_CACHE_BUDGET, DEFAULT_UPLOAD_BUDGET, DecodeEvent, RequestToken, TileCache,
};
use tracing::info_span;
use web_time::Instant;
use wgpu::util::DeviceExt;

/// Maximum number of decoded tiles waiting for renderer-owned GPU upload.
pub const DEFAULT_RENDER_UPLOAD_QUEUE_ITEMS: usize = 16;
/// The bridge holds at most two normal upload batches. One oversized tile is permitted alone.
pub const DEFAULT_RENDER_UPLOAD_QUEUE_BYTES: usize = DEFAULT_UPLOAD_BUDGET * 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayMap {
    Viridis,
    Greyscale,
    Threshold,
}

#[derive(Clone, Copy, Debug)]
pub struct DisplaySettings {
    pub window_low: f32,
    pub window_high: f32,
    pub map: DisplayMap,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            window_low: 0.05,
            window_high: 0.9,
            map: DisplayMap::Viridis,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PhysicalViewport {
    pub origin: PhysicalPoint,
    pub size: PhysicalPoint,
    pub scale_factor: f32,
}

#[derive(Clone, Debug)]
pub struct ImageRenderRequest {
    pub pane: PaneId,
    pub source: SourceId,
    pub source_generation: u64,
    pub viewport: PhysicalViewport,
    pub camera: Camera,
    pub display: DisplaySettings,
    pub desired_tiles: Vec<TileKey>,
}

#[derive(Default)]
pub struct RenderPlan {
    pub images: Vec<ImageRenderRequest>,
}

impl RenderPlan {
    pub fn submit(&mut self, request: ImageRenderRequest) {
        self.images.push(request);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UploadQueueLimits {
    pub max_items: usize,
    pub max_bytes: usize,
}

impl Default for UploadQueueLimits {
    fn default() -> Self {
        Self {
            max_items: DEFAULT_RENDER_UPLOAD_QUEUE_ITEMS,
            max_bytes: DEFAULT_RENDER_UPLOAD_QUEUE_BYTES,
        }
    }
}

/// A renderer-owned residency transition. Consumers must validate the token with the runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileResidencyAck {
    pub key: TileKey,
    pub token: RequestToken,
}

/// Why a decoded tile was not admitted to the bounded renderer bridge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UploadRejection {
    ItemCapacity,
    ByteCapacity,
}

/// Admission preserves ownership of a rejected event, so callers can retry it without loss.
#[must_use]
#[derive(Debug)]
pub enum UploadAdmission {
    Accepted,
    Rejected {
        event: DecodeEvent,
        reason: UploadRejection,
    },
}

impl UploadAdmission {
    pub fn accepted(&self) -> bool {
        matches!(self, Self::Accepted)
    }
}

#[derive(Clone)]
pub struct RenderBridge(Arc<Mutex<RenderBridgeState>>);

struct RenderBridgeState {
    uploads: VecDeque<DecodeEvent>,
    upload_bytes: usize,
    limits: UploadQueueLimits,
    became_resident: Vec<TileResidencyAck>,
    evicted: Vec<TileResidencyAck>,
    metrics: RenderMetrics,
}

impl RenderBridge {
    pub fn with_upload_limits(limits: UploadQueueLimits) -> Self {
        Self(Arc::new(Mutex::new(RenderBridgeState {
            uploads: VecDeque::new(),
            upload_bytes: 0,
            limits,
            became_resident: Vec::new(),
            evicted: Vec::new(),
            metrics: RenderMetrics::default(),
        })))
    }

    /// Admit a decoded tile without losing it when the bridge is full.
    ///
    /// A normal event must fit both caps. An event larger than `max_bytes` is admitted only when
    /// the queue is empty, where it is retained as the sole queued event and uploaded in the next
    /// batch. This bounded exception guarantees forward progress for a single oversized tile.
    pub fn push(&self, event: DecodeEvent) -> UploadAdmission {
        let mut state = self.0.lock();
        let bytes = event.bytes();
        let reason = if state.uploads.len() >= state.limits.max_items {
            Some(UploadRejection::ItemCapacity)
        } else if bytes > state.limits.max_bytes {
            (!state.uploads.is_empty()).then_some(UploadRejection::ByteCapacity)
        } else if state.upload_bytes.saturating_add(bytes) > state.limits.max_bytes {
            Some(UploadRejection::ByteCapacity)
        } else {
            None
        };
        if let Some(reason) = reason {
            return UploadAdmission::Rejected { event, reason };
        }

        state.upload_bytes += bytes;
        state.uploads.push_back(event);
        state.metrics.pending_upload_bytes = state.upload_bytes;
        UploadAdmission::Accepted
    }

    pub fn take_resident(&self) -> Vec<TileResidencyAck> {
        std::mem::take(&mut self.0.lock().became_resident)
    }
    pub fn take_evicted(&self) -> Vec<TileResidencyAck> {
        std::mem::take(&mut self.0.lock().evicted)
    }
    pub fn snapshot(&self) -> RenderMetrics {
        self.0.lock().metrics.clone()
    }

    fn retain_generation(&self, generation: u64) {
        let mut state = self.0.lock();
        state
            .uploads
            .retain(|event| event.token().source_generation == generation);
        state.upload_bytes = state.uploads.iter().map(DecodeEvent::bytes).sum();
        state.became_resident.clear();
        state.evicted.clear();
        state.metrics.pending_upload_bytes = state.upload_bytes;
        state.metrics.resident_texture_bytes = 0;
    }
}

impl Default for RenderBridge {
    fn default() -> Self {
        Self::with_upload_limits(UploadQueueLimits::default())
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DisplayUniform {
    low: f32,
    high: f32,
    map: u32,
    _padding: u32,
    rect_ndc: [f32; 4],
}

struct ScalarTexture {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    token: RequestToken,
}
struct TileDraw {
    _uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}
struct PaneDraw {
    tiles: Vec<TileDraw>,
}

pub struct ScalarRenderer {
    pipeline: wgpu::RenderPipeline,
    tile_vertices: wgpu::Buffer,
    bind_layout: wgpu::BindGroupLayout,
    textures: BTreeMap<TileKey, ScalarTexture>,
    panes: BTreeMap<PaneId, PaneDraw>,
    cache: TileCache,
    bridge: RenderBridge,
    last_upload_frame: u64,
    source_generation: Option<u64>,
}

impl ScalarRenderer {
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        bridge: RenderBridge,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("polyorama scalar shader"),
            source: wgpu::ShaderSource::Wgsl(SCALAR_SHADER.into()),
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("polyorama scalar bindings"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("polyorama scalar pipeline layout"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });
        let tile_vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("polyorama bounded tile quad"),
            contents: bytemuck::cast_slice(&TILE_QUAD),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("polyorama scalar pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<[f32; 2]>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    }],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        {
            let mut state = bridge.0.lock();
            state.metrics.capability_profile = format!(
                "WebGPU/WGPU; scalar=R16Uint; target={target_format:?}; timestamps=unavailable"
            );
        }
        Self {
            pipeline,
            tile_vertices,
            bind_layout,
            textures: BTreeMap::new(),
            panes: BTreeMap::new(),
            cache: TileCache::new(DEFAULT_CACHE_BUDGET),
            bridge,
            last_upload_frame: u64::MAX,
            source_generation: None,
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame_number: u64,
        request: &ImageRenderRequest,
    ) {
        let started = Instant::now();
        let _span = info_span!("render_preparation", pane = request.pane.0).entered();
        self.ensure_generation(request.source_generation);
        if self.last_upload_frame != frame_number {
            self.last_upload_frame = frame_number;
            self.begin_frame_metrics();
            self.upload_pending(device, queue);
        }
        let map = match request.display.map {
            DisplayMap::Viridis => 0,
            DisplayMap::Greyscale => 1,
            DisplayMap::Threshold => 2,
        };
        let pane = self
            .panes
            .entry(request.pane)
            .or_insert_with(|| PaneDraw { tiles: Vec::new() });
        pane.tiles.clear();
        let mut desired_tiles = request.desired_tiles.clone();
        desired_tiles.sort_by_key(|key| std::cmp::Reverse((key.level, key.x, key.y)));
        desired_tiles.dedup();
        for key in desired_tiles {
            if let Some(texture) = self.textures.get(&key)
                && texture.token.source_generation == request.source_generation
                && self.cache.contains(key)
            {
                let uniform_data = DisplayUniform {
                    low: request.display.window_low,
                    high: request.display.window_high,
                    map,
                    _padding: 0,
                    rect_ndc: tile_ndc_rect(request, key),
                };
                let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("polyorama tile display uniform"),
                    contents: bytemuck::bytes_of(&uniform_data),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("polyorama scalar tile bind group"),
                    layout: &self.bind_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&texture.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: uniform.as_entire_binding(),
                        },
                    ],
                });
                pane.tiles.push(TileDraw {
                    _uniform: uniform,
                    bind_group,
                });
            }
        }
        let mut state = self.bridge.0.lock();
        // Each callback preparation represents one renderer viewport/job. The renderer returns
        // no command buffers: egui owns the enclosing encoder and submission.
        state.metrics.gpu_viewports += 1;
        state.metrics.render_jobs += 1;
        state.metrics.resident_texture_bytes = self.cache.used();
        state.metrics.cache_evictions = self.cache.evictions;
        state.metrics.prepare_ms += started.elapsed().as_secs_f64() * 1000.0;
    }

    fn ensure_generation(&mut self, generation: u64) {
        if self.source_generation == Some(generation) {
            return;
        }
        self.source_generation = Some(generation);
        self.textures.clear();
        self.panes.clear();
        self.cache.clear();
        self.bridge.retain_generation(generation);
    }

    fn begin_frame_metrics(&self) {
        let mut state = self.bridge.0.lock();
        state.metrics.gpu_viewports = 0;
        state.metrics.render_jobs = 0;
        state.metrics.render_passes = 0;
        state.metrics.draw_calls = 0;
        state.metrics.command_buffers = 0;
        state.metrics.uploaded_bytes = 0;
        state.metrics.prepare_ms = 0.0;
    }

    fn upload_pending(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let _span = info_span!("tile_upload").entered();
        let mut state = self.bridge.0.lock();
        let mut used = 0;
        while let Some(event) = state.uploads.front() {
            let bytes = event.bytes();
            if used > 0 && used + bytes > DEFAULT_UPLOAD_BUDGET {
                break;
            }
            let event = state.uploads.pop_front().unwrap();
            state.upload_bytes -= bytes;
            if event.token().source_generation != self.source_generation.unwrap_or_default() {
                continue;
            }
            if let DecodeEvent::Completed {
                key,
                token,
                scalar_u16_le,
                ..
            } = event
            {
                // The cache deliberately permits one tile larger than its budget. In that case
                // it is the sole resident texture; admitting a later tile evicts it normally.
                if let Some(previous) = self.textures.remove(&key) {
                    state.evicted.push(TileResidencyAck {
                        key,
                        token: previous.token,
                    });
                }
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("polyorama R16Uint scalar tile"),
                    size: wgpu::Extent3d {
                        width: polyorama_core::TILE_SIZE,
                        height: polyorama_core::TILE_SIZE,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R16Uint,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &scalar_u16_le,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(polyorama_core::TILE_SIZE * 2),
                        rows_per_image: Some(polyorama_core::TILE_SIZE),
                    },
                    wgpu::Extent3d {
                        width: polyorama_core::TILE_SIZE,
                        height: polyorama_core::TILE_SIZE,
                        depth_or_array_layers: 1,
                    },
                );
                let evicted = self.cache.insert(key, bytes);
                let eviction_span = info_span!("cache_eviction", count = evicted.len());
                let _eviction_guard = (!evicted.is_empty()).then(|| eviction_span.enter());
                for victim in evicted {
                    if let Some(texture) = self.textures.remove(&victim) {
                        state.evicted.push(TileResidencyAck {
                            key: victim,
                            token: texture.token,
                        });
                    }
                }
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                self.textures.insert(
                    key,
                    ScalarTexture {
                        _texture: texture,
                        view,
                        token,
                    },
                );
                state.became_resident.push(TileResidencyAck { key, token });
                used += bytes;
            }
        }
        state.metrics.uploaded_bytes = used;
        state.metrics.pending_upload_bytes = state.upload_bytes;
        state.metrics.resident_texture_bytes = self.cache.used();
        state.metrics.cache_evictions = self.cache.evictions;
    }

    pub fn paint(
        &self,
        pane: PaneId,
        viewport: PixelRect,
        clip: PixelRect,
        render_pass: &mut wgpu::RenderPass<'static>,
    ) {
        let _span = info_span!("viewport_rendering", pane = pane.0).entered();
        // We do not own egui's render pass, but this callback has been invoked with one.
        self.bridge.0.lock().metrics.render_passes += 1;
        let Some(draw) = self.panes.get(&pane) else {
            return;
        };
        if draw.tiles.is_empty() {
            return;
        }
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_scissor_rect(clip.x, clip.y, clip.width.max(1), clip.height.max(1));
        render_pass.set_viewport(
            viewport.x as f32,
            viewport.y as f32,
            viewport.width as f32,
            viewport.height as f32,
            0.0,
            1.0,
        );
        render_pass.set_vertex_buffer(0, self.tile_vertices.slice(..));
        self.bridge.0.lock().metrics.draw_calls += draw.tiles.len();
        for tile in &draw.tiles {
            render_pass.set_bind_group(0, &tile.bind_group, &[]);
            render_pass.draw(0..TILE_QUAD.len() as u32, 0..1);
        }
    }
}

const TILE_QUAD: [[f32; 2]; 6] = [
    [0.0, 0.0],
    [1.0, 0.0],
    [0.0, 1.0],
    [0.0, 1.0],
    [1.0, 0.0],
    [1.0, 1.0],
];

fn tile_ndc_rect(request: &ImageRenderRequest, key: TileKey) -> [f32; 4] {
    let scale = request.viewport.scale_factor.max(f32::EPSILON) as f64;
    let viewport_width = request.viewport.size.x / scale;
    let viewport_height = request.viewport.size.y / scale;
    let tile_extent = polyorama_core::TILE_SIZE as f64 * 2_f64.powi(key.level as i32);
    let left = viewport_width * 0.5
        + (key.x as f64 * tile_extent - request.camera.centre.x)
            / request.camera.pixels_per_screen_point;
    let top = viewport_height * 0.5
        + (key.y as f64 * tile_extent - request.camera.centre.y)
            / request.camera.pixels_per_screen_point;
    let right = left + tile_extent / request.camera.pixels_per_screen_point;
    let bottom = top + tile_extent / request.camera.pixels_per_screen_point;
    [
        (left / viewport_width * 2.0 - 1.0) as f32,
        (1.0 - bottom / viewport_height * 2.0) as f32,
        (right / viewport_width * 2.0 - 1.0) as f32,
        (1.0 - top / viewport_height * 2.0) as f32,
    ]
}

#[derive(Clone, Copy)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

const SCALAR_SHADER: &str = r#"
struct Display { low: f32, high: f32, map: u32, padding: u32, rect_ndc: vec4<f32> };
@group(0) @binding(0) var scalar_tile: texture_2d<u32>;
@group(0) @binding(1) var<uniform> display: Display;

struct VertexOut { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs_main(@location(0) local: vec2<f32>) -> VertexOut {
    var out: VertexOut;
    out.position = vec4(mix(display.rect_ndc.xy, display.rect_ndc.zw, local), 0.0, 1.0);
    out.uv = vec2(local.x, 1.0 - local.y);
    return out;
}
fn viridis(t: f32) -> vec3<f32> {
    return vec3(0.267 + 0.633*t, 0.005 + 0.86*t - 0.42*t*t, 0.329 + 0.55*(1.0-abs(2.0*t-1.0)));
}
@fragment fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let size = textureDimensions(scalar_tile);
    let pixel = vec2<i32>(clamp(in.uv * vec2<f32>(size), vec2(0.0), vec2<f32>(size - vec2<u32>(1u))));
    let scalar = f32(textureLoad(scalar_tile, pixel, 0).r) / 65535.0;
    let t = clamp((scalar - display.low) / max(0.0001, display.high - display.low), 0.0, 1.0);
    var colour = viridis(t);
    if display.map == 1u { colour = vec3(t); }
    if display.map == 2u { colour = select(vec3(0.025, 0.035, 0.045), vec3(1.0, 0.45, 0.08), t > 0.56); }
    return vec4(colour, 1.0);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use polyorama_core::{ImagePoint, SourceId};

    fn key(x: u32) -> TileKey {
        TileKey {
            source: SourceId(1),
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

    fn event(key: TileKey, token: RequestToken, bytes: usize) -> DecodeEvent {
        DecodeEvent::Completed {
            key,
            token,
            scalar_u16_le: vec![0; bytes],
            preparation_ms: 0.0,
            decode_ms: 0.0,
        }
    }

    fn request(camera: Camera) -> ImageRenderRequest {
        ImageRenderRequest {
            pane: PaneId(1),
            source: SourceId(1),
            source_generation: 1,
            viewport: PhysicalViewport {
                origin: PhysicalPoint::new(0.0, 0.0),
                size: PhysicalPoint::new(1_000.0, 800.0),
                scale_factor: 1.0,
            },
            camera,
            display: DisplaySettings::default(),
            desired_tiles: Vec::new(),
        }
    }

    #[test]
    fn camera_pan_translates_tile_geometry() {
        let camera = Camera {
            centre: ImagePoint::new(65_536.0, 65_536.0),
            pixels_per_screen_point: 128.0,
        };
        let key = TileKey {
            source: SourceId(1),
            level: 8,
            x: 1,
            y: 1,
        };
        let before = tile_ndc_rect(&request(camera), key);
        let mut panned = camera;
        panned.centre.x += 12_800.0;
        let after = tile_ndc_rect(&request(panned), key);
        assert!(after[0] < before[0]);
        assert!((after[1] - before[1]).abs() < f32::EPSILON);
    }

    #[test]
    fn camera_zoom_changes_tile_extent_around_the_viewport() {
        let camera = Camera {
            centre: ImagePoint::new(65_536.0, 65_536.0),
            pixels_per_screen_point: 128.0,
        };
        let key = TileKey {
            source: SourceId(1),
            level: 8,
            x: 1,
            y: 1,
        };
        let before = tile_ndc_rect(&request(camera), key);
        let mut zoomed = camera;
        zoomed.pixels_per_screen_point *= 0.5;
        let after = tile_ndc_rect(&request(zoomed), key);
        assert!(after[2] - after[0] > before[2] - before[0]);
        assert!(after[3] - after[1] > before[3] - before[1]);
    }

    #[test]
    fn tile_quad_cannot_rasterise_outside_projected_bounds() {
        assert_eq!(TILE_QUAD.len(), 6);
        assert!(
            TILE_QUAD
                .iter()
                .flatten()
                .all(|value| (0.0..=1.0).contains(value))
        );
        for corner in [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]] {
            assert!(TILE_QUAD.contains(&corner));
        }
    }

    #[test]
    fn bridge_rejects_at_byte_capacity_without_losing_the_event() {
        let bridge = RenderBridge::with_upload_limits(UploadQueueLimits {
            max_items: 2,
            max_bytes: 10,
        });
        assert!(bridge.push(event(key(0), token(1), 6)).accepted());
        let rejected = bridge.push(event(key(1), token(2), 5));
        assert!(matches!(
            rejected,
            UploadAdmission::Rejected {
                event: DecodeEvent::Completed { key: rejected_key, token: rejected_token, .. },
                reason: UploadRejection::ByteCapacity,
            } if rejected_key == key(1) && rejected_token == token(2)
        ));
        let state = bridge.0.lock();
        assert_eq!(state.uploads.len(), 1);
        assert_eq!(state.upload_bytes, 6);
        assert_eq!(state.metrics.pending_upload_bytes, 6);
    }

    #[test]
    fn bridge_rejects_at_item_capacity_without_losing_the_event() {
        let bridge = RenderBridge::with_upload_limits(UploadQueueLimits {
            max_items: 1,
            max_bytes: 10,
        });
        assert!(bridge.push(event(key(0), token(1), 1)).accepted());
        assert!(matches!(
            bridge.push(event(key(1), token(2), 1)),
            UploadAdmission::Rejected {
                event: DecodeEvent::Completed { key: rejected_key, token: rejected_token, .. },
                reason: UploadRejection::ItemCapacity,
            } if rejected_key == key(1) && rejected_token == token(2)
        ));
    }

    #[test]
    fn bridge_admits_one_oversized_event_only_when_empty() {
        let bridge = RenderBridge::with_upload_limits(UploadQueueLimits {
            max_items: 2,
            max_bytes: 10,
        });
        assert!(bridge.push(event(key(0), token(1), 11)).accepted());
        assert!(matches!(
            bridge.push(event(key(1), token(2), 1)),
            UploadAdmission::Rejected {
                reason: UploadRejection::ByteCapacity,
                ..
            }
        ));
        let state = bridge.0.lock();
        assert_eq!(state.uploads.len(), 1);
        assert_eq!(state.upload_bytes, 11);
        assert_eq!(state.metrics.pending_upload_bytes, 11);
    }

    #[test]
    fn acknowledgements_keep_the_token_that_owned_gpu_residency() {
        let bridge = RenderBridge::default();
        let acknowledgement = TileResidencyAck {
            key: key(3),
            token: token(7),
        };
        bridge.0.lock().became_resident.push(acknowledgement);
        bridge.0.lock().evicted.push(acknowledgement);
        assert_eq!(bridge.take_resident(), vec![acknowledgement]);
        assert_eq!(bridge.take_evicted(), vec![acknowledgement]);
    }

    #[test]
    fn generation_change_discards_old_uploads_and_residency_acknowledgements() {
        let bridge = RenderBridge::default();
        let old = token(1);
        let current = RequestToken {
            source_generation: 2,
            demand_epoch: 2,
            sequence: 2,
        };
        assert!(bridge.push(event(key(0), old, 4)).accepted());
        assert!(bridge.push(event(key(1), current, 6)).accepted());
        bridge.0.lock().became_resident.push(TileResidencyAck {
            key: key(0),
            token: old,
        });
        bridge.retain_generation(2);
        let state = bridge.0.lock();
        assert_eq!(state.uploads.len(), 1);
        assert_eq!(state.uploads.front().unwrap().token(), current);
        assert_eq!(state.upload_bytes, 6);
        assert!(state.became_resident.is_empty());
    }

    #[test]
    fn gpu_cache_retains_a_single_oversized_texture_then_evicts_it_for_the_next_texture() {
        let mut cache = TileCache::new(10);
        assert!(cache.insert(key(0), 20).is_empty());
        assert_eq!(cache.used(), 20);
        assert_eq!(cache.insert(key(1), 5), vec![key(0)]);
        assert_eq!(cache.used(), 5);
    }
}
