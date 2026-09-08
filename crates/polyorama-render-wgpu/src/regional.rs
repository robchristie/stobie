//! Integer regional textures shared by every image and gallery consumer.

use std::collections::BTreeMap;

use bytemuck::{Pod, Zeroable};
use polyorama_core::{PaneId, RegionKey, RegionalPixels, SampleLayout};
use polyorama_runtime::{RegionalCache, RegionalUpload, RequestToken};
use wgpu::util::DeviceExt;

use crate::PixelRect;

/// Per-channel stretch in native sample units. This never changes source/cache identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionalDisplaySettings {
    pub low: [f32; 3],
    pub high: [f32; 3],
    pub gamma: f32,
}

impl Default for RegionalDisplaySettings {
    fn default() -> Self {
        Self {
            low: [0.0; 3],
            high: [65535.0; 3],
            gamma: 1.0,
        }
    }
}

impl RegionalDisplaySettings {
    pub fn is_valid(self) -> bool {
        self.gamma.is_finite()
            && self.gamma > 0.0
            && self
                .low
                .iter()
                .zip(self.high)
                .all(|(low, high)| low.is_finite() && high.is_finite() && high > *low)
    }
}

#[derive(Clone, Debug)]
pub struct RegionalDraw {
    pub key: RegionKey,
    /// Left, top, right, bottom in viewport-local normalised device coordinates.
    pub rect_ndc: [f32; 4],
    /// Left, top, right, bottom in texture coordinates; permits parent-region crops.
    pub uv: [f32; 4],
    pub display: RegionalDisplaySettings,
}

#[derive(Clone, Copy, Debug)]
pub struct RegionalGpuLimits {
    /// Logical texture bytes; driver allocation overhead is outside this measurement.
    pub texture_bytes: usize,
    pub texture_items: usize,
    /// Total uploaded bytes per frame, bounding staging and temporary packed sample data.
    /// The application submits the shared GPU queue once each frame.
    pub upload_scratch_bytes: usize,
    /// Total draws across all panes in one frame.
    pub draws: usize,
}

impl Default for RegionalGpuLimits {
    fn default() -> Self {
        Self {
            texture_bytes: 128 * 1024 * 1024,
            texture_items: 4096,
            upload_scratch_bytes: 4 * 1024 * 1024,
            draws: 4096,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegionalGpuMetrics {
    pub texture_bytes: usize,
    pub texture_items: usize,
    pub upload_scratch_peak_bytes: usize,
    pub upload_bytes_this_frame: usize,
    pub uploads: u64,
    pub evictions: u64,
    pub prepared_draws: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionalGpuError {
    InvalidPayload,
    Capacity,
    UploadScratchCapacity,
    TextureDimension,
    FrameAlreadyPrepared,
    InvalidDraw,
    DrawCapacity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionalResidency {
    pub key: RegionKey,
    pub token: RequestToken,
}

pub struct RegionalGpuAdmission {
    pub resident: RegionalResidency,
    pub evicted: Vec<RegionalResidency>,
}

struct RegionalTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    scalar: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RegionalUniform {
    rect: [f32; 4],
    uv: [f32; 4],
    low_gamma: [f32; 4],
    high_scalar: [f32; 4],
}

struct PreparedRegion {
    bind_group: wgpu::BindGroup,
    _uniform: wgpu::Buffer,
}

/// Own one per shared WGPU device, not one per viewport or source.
///
/// Each frame: `begin_frame`, upload results, then `prepare` panes and `paint`.
/// Uploads are refused after preparation starts, so cached bind groups cannot retain
/// evicted textures beyond their byte accounting. Pending driver work may retain old
/// allocations; these metrics measure owned logical residency, not physical VRAM.
pub struct RegionalRenderer {
    pipeline: wgpu::RenderPipeline,
    bindings: wgpu::BindGroupLayout,
    cache: RegionalCache<RegionalTexture>,
    panes: BTreeMap<PaneId, Vec<PreparedRegion>>,
    limits: RegionalGpuLimits,
    preparing: bool,
    metrics: RegionalGpuMetrics,
}

impl RegionalRenderer {
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        limits: RegionalGpuLimits,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("polyorama native regional samples"),
            source: wgpu::ShaderSource::Wgsl(REGIONAL_SHADER.into()),
        });
        let bindings = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("polyorama regional bindings"),
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
            label: Some("polyorama regional pipeline layout"),
            bind_group_layouts: &[Some(&bindings)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("polyorama regional pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
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
        Self {
            pipeline,
            bindings,
            cache: RegionalCache::new(limits.texture_bytes, limits.texture_items),
            panes: BTreeMap::new(),
            limits,
            preparing: false,
            metrics: RegionalGpuMetrics::default(),
        }
    }

    pub fn begin_frame(&mut self) {
        self.panes.clear();
        self.preparing = false;
        self.metrics.prepared_draws = 0;
        self.metrics.upload_bytes_this_frame = 0;
    }

    /// Call `RegionalRuntime::is_upload_current` before admission, then report all
    /// evictions and `finish_upload` after this consumes/drops the decoded allocation.
    /// Rejection preserves ownership for explicit retry or release and acknowledgement.
    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        upload: RegionalUpload,
    ) -> Result<RegionalGpuAdmission, (Box<RegionalUpload>, RegionalGpuError)> {
        let validation = if self.preparing {
            Err(RegionalGpuError::FrameAlreadyPrepared)
        } else if !upload.key.is_valid()
            || !upload.pixels.is_valid()
            || upload.key.components.len() != upload.pixels.layout.channels()
        {
            Err(RegionalGpuError::InvalidPayload)
        } else if upload.pixels.width > device.limits().max_texture_dimension_2d
            || upload.pixels.height > device.limits().max_texture_dimension_2d
        {
            Err(RegionalGpuError::TextureDimension)
        } else {
            regional_texture_bytes(&upload.pixels).ok_or(RegionalGpuError::Capacity)
        };
        let bytes = match validation {
            Ok(bytes) => bytes,
            Err(error) => return Err((Box::new(upload), error)),
        };
        if bytes
            > self
                .limits
                .upload_scratch_bytes
                .saturating_sub(self.metrics.upload_bytes_this_frame)
        {
            return Err((Box::new(upload), RegionalGpuError::UploadScratchCapacity));
        }
        let evictions = match self.cache.make_room(&upload.key, bytes) {
            Ok(evictions) => evictions,
            Err(_) => return Err((Box::new(upload), RegionalGpuError::Capacity)),
        };
        let evicted: Vec<_> = evictions
            .into_iter()
            .map(|entry| {
                entry.value.texture.destroy();
                RegionalResidency {
                    key: entry.key,
                    token: entry.token,
                }
            })
            .collect();
        let scalar = upload.pixels.layout == SampleLayout::Scalar;
        let format = if scalar {
            wgpu::TextureFormat::R16Uint
        } else {
            wgpu::TextureFormat::Rgba16Uint
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("polyorama regional integer samples"),
            size: wgpu::Extent3d {
                width: upload.pixels.width,
                height: upload.pixels.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let packed = pack_regional_samples(&upload.pixels);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &packed,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(upload.pixels.width * if scalar { 2 } else { 8 }),
                rows_per_image: Some(upload.pixels.height),
            },
            texture.size(),
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let resident = RegionalResidency {
            key: upload.key.clone(),
            token: upload.token,
        };
        // make_room already released old resources before this allocation.
        let inserted = self.cache.insert(
            upload.key,
            upload.token,
            bytes,
            RegionalTexture {
                texture,
                view,
                scalar,
            },
        );
        debug_assert!(matches!(inserted, Ok(ref removed) if removed.is_empty()));
        self.metrics.upload_scratch_peak_bytes =
            self.metrics.upload_scratch_peak_bytes.max(packed.len());
        self.metrics.uploads += 1;
        self.metrics.upload_bytes_this_frame += bytes;
        self.metrics.evictions += evicted.len() as u64;
        Ok(RegionalGpuAdmission { resident, evicted })
    }

    /// Prepare only materialised draws. Missing keys are skipped for coarse fallback
    /// composition by the caller; draws are painted in the supplied back-to-front order.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        pane: PaneId,
        draws: &[RegionalDraw],
    ) -> Result<usize, RegionalGpuError> {
        let previous = self.panes.get(&pane).map_or(0, Vec::len);
        if draws.len()
            > self
                .limits
                .draws
                .saturating_sub(self.metrics.prepared_draws - previous)
        {
            return Err(RegionalGpuError::DrawCapacity);
        }
        if draws.iter().any(|draw| {
            !draw.display.is_valid()
                || draw
                    .rect_ndc
                    .iter()
                    .chain(draw.uv.iter())
                    .any(|v| !v.is_finite())
                || draw.rect_ndc[0] >= draw.rect_ndc[2]
                || draw.rect_ndc[1] <= draw.rect_ndc[3]
                || draw.uv[0] < 0.0
                || draw.uv[1] < 0.0
                || draw.uv[2] > 1.0
                || draw.uv[3] > 1.0
                || draw.uv[0] >= draw.uv[2]
                || draw.uv[1] >= draw.uv[3]
        }) {
            return Err(RegionalGpuError::InvalidDraw);
        }
        self.preparing = true;
        let mut prepared = Vec::new();
        for draw in draws {
            let Some(texture) = self.cache.get(&draw.key) else {
                continue;
            };
            let [r_low, g_low, b_low] = draw.display.low;
            let [r_high, g_high, b_high] = draw.display.high;
            let settings = RegionalUniform {
                rect: draw.rect_ndc,
                uv: draw.uv,
                low_gamma: [r_low, g_low, b_low, draw.display.gamma],
                high_scalar: [
                    r_high,
                    g_high,
                    b_high,
                    if texture.scalar { 1.0 } else { 0.0 },
                ],
            };
            let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("polyorama regional display settings"),
                contents: bytemuck::bytes_of(&settings),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("polyorama regional draw"),
                layout: &self.bindings,
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
            prepared.push(PreparedRegion {
                bind_group,
                _uniform: uniform,
            });
        }
        let count = prepared.len();
        self.metrics.prepared_draws = self.metrics.prepared_draws - previous + count;
        self.panes.insert(pane, prepared);
        Ok(count)
    }

    pub fn paint(
        &self,
        pane: PaneId,
        viewport: PixelRect,
        clip: PixelRect,
        pass: &mut wgpu::RenderPass<'static>,
    ) {
        if viewport.width == 0 || viewport.height == 0 || clip.width == 0 || clip.height == 0 {
            return;
        }
        let Some(draws) = self.panes.get(&pane) else {
            return;
        };
        pass.set_pipeline(&self.pipeline);
        pass.set_viewport(
            viewport.x as f32,
            viewport.y as f32,
            viewport.width as f32,
            viewport.height as f32,
            0.0,
            1.0,
        );
        pass.set_scissor_rect(clip.x, clip.y, clip.width, clip.height);
        for draw in draws {
            pass.set_bind_group(0, &draw.bind_group, &[]);
            pass.draw(0..6, 0..1);
        }
    }

    pub fn contains(&self, key: &RegionKey) -> bool {
        self.cache.peek(key).is_some()
    }

    pub fn metrics(&self) -> RegionalGpuMetrics {
        RegionalGpuMetrics {
            texture_bytes: self.cache.bytes(),
            texture_items: self.cache.len(),
            ..self.metrics
        }
    }
}

fn regional_texture_bytes(pixels: &RegionalPixels) -> Option<usize> {
    (pixels.width as usize)
        .checked_mul(pixels.height as usize)?
        .checked_mul(if pixels.layout == SampleLayout::Scalar {
            2
        } else {
            8
        })
}

fn pack_regional_samples(pixels: &RegionalPixels) -> Vec<u8> {
    let mut output =
        Vec::with_capacity(regional_texture_bytes(pixels).expect("validated texture size"));
    for pixel in pixels.samples.chunks_exact(pixels.layout.channels()) {
        for sample in pixel {
            output.extend_from_slice(&sample.to_le_bytes());
        }
        if pixels.layout == SampleLayout::Rgb {
            output.extend_from_slice(&0_u16.to_le_bytes());
        }
    }
    output
}

const REGIONAL_SHADER: &str = r#"
struct Display { rect: vec4<f32>, uv: vec4<f32>, low_gamma: vec4<f32>, high_scalar: vec4<f32> };
@group(0) @binding(0) var samples: texture_2d<u32>;
@group(0) @binding(1) var<uniform> display: Display;
struct VertexOut { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs_main(@builtin(vertex_index) index: u32) -> VertexOut {
    let vertices = array<vec2<f32>, 6>(vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0), vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0));
    let local = vertices[index];
    var out: VertexOut;
    out.position = vec4(mix(display.rect.xy, display.rect.zw, local), 0.0, 1.0);
    out.uv = mix(display.uv.xy, display.uv.zw, local);
    return out;
}
@fragment fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let size = textureDimensions(samples);
    let pixel = vec2<i32>(clamp(in.uv * vec2<f32>(size), vec2(0.0), vec2<f32>(size - vec2<u32>(1u))));
    let raw = vec3<f32>(textureLoad(samples, pixel, 0).rgb);
    let colour = select(raw, vec3(raw.r), display.high_scalar.w > 0.5);
    let mapped = clamp((colour - display.low_gamma.rgb) / (display.high_scalar.rgb - display.low_gamma.rgb), vec3(0.0), vec3(1.0));
    return vec4(pow(mapped, vec3(1.0 / display.low_gamma.w)), 1.0);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_scalar_and_rgb_precision_survive_texture_packing() {
        let scalar = RegionalPixels {
            width: 2,
            height: 1,
            layout: SampleLayout::Scalar,
            precision: 11,
            samples: vec![1, 2047],
        };
        assert_eq!(pack_regional_samples(&scalar), vec![1, 0, 255, 7]);
        let rgb = RegionalPixels {
            width: 1,
            height: 1,
            layout: SampleLayout::Rgb,
            precision: 16,
            samples: vec![257, 32768, 65535],
        };
        assert_eq!(
            pack_regional_samples(&rgb),
            vec![1, 1, 0, 128, 255, 255, 0, 0]
        );
        assert_eq!(regional_texture_bytes(&rgb), Some(8));
    }

    #[test]
    fn display_mapping_rejects_non_finite_or_degenerate_windows() {
        assert!(RegionalDisplaySettings::default().is_valid());
        assert!(
            !RegionalDisplaySettings {
                gamma: f32::NAN,
                ..Default::default()
            }
            .is_valid()
        );
        assert!(
            !RegionalDisplaySettings {
                low: [2.0; 3],
                high: [1.0; 3],
                gamma: 1.0
            }
            .is_valid()
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        struct ThreadWake(std::thread::Thread);
        impl std::task::Wake for ThreadWake {
            fn wake(self: std::sync::Arc<Self>) {
                self.0.unpark();
            }
        }
        let waker = std::task::Waker::from(std::sync::Arc::new(ThreadWake(std::thread::current())));
        let mut context = std::task::Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        loop {
            match future.as_mut().poll(&mut context) {
                std::task::Poll::Ready(value) => return value,
                std::task::Poll::Pending => std::thread::park(),
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "requires a native GPU adapter; run explicitly during regional renderer qualification"]
    fn regional_gpu_readback_preserves_orientation_precision_and_residency_budget() {
        use polyorama_core::{ImageRegion, RepresentationId, SourceStage};
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("native GPU adapter");
        eprintln!("regional readback adapter: {:?}", adapter.get_info());
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("GPU device");
        let mut renderer = RegionalRenderer::new(
            &device,
            wgpu::TextureFormat::Rgba8Unorm,
            RegionalGpuLimits {
                texture_bytes: 64,
                texture_items: 2,
                upload_scratch_bytes: 64,
                draws: 2,
            },
        );
        let cases = [
            RegionalPixels {
                width: 4,
                height: 2,
                layout: SampleLayout::Scalar,
                precision: 11,
                samples: vec![0, 1024, 2047, 500, 100, 200, 300, 400],
            },
            RegionalPixels {
                width: 4,
                height: 2,
                layout: SampleLayout::Rgb,
                precision: 16,
                samples: vec![
                    0, 0, 0, 32768, 0, 0, 0, 65535, 0, 0, 0, 65535, 65535, 65535, 65535, 16384,
                    32768, 49152, 1000, 2000, 3000, 60000, 50000, 40000,
                ],
            },
        ];
        for (case, pixels) in cases.into_iter().enumerate() {
            let maximum = ((1_u32 << pixels.precision) - 1) as f32;
            let expected: Vec<[u8; 4]> = pixels
                .samples
                .chunks_exact(pixels.layout.channels())
                .map(|sample| {
                    let channel =
                        |index: usize| ((sample[index] as f32 / maximum) * 255.0).round() as u8;
                    if pixels.layout == SampleLayout::Scalar {
                        [channel(0), channel(0), channel(0), 255]
                    } else {
                        [channel(0), channel(1), channel(2), 255]
                    }
                })
                .collect();
            let key = RegionKey {
                representation: RepresentationId([case as u8; 32]),
                region: ImageRegion {
                    x: 50_000,
                    y: 20_000,
                    width: 4,
                    height: 2,
                },
                reduction: 0,
                components: (0..pixels.layout.channels() as u16).collect(),
                stage: SourceStage(0),
            };
            let token = RequestToken {
                source_generation: 0,
                demand_epoch: 1,
                sequence: case as u64,
            };
            renderer.begin_frame();
            let admission = renderer
                .upload(
                    &device,
                    &queue,
                    RegionalUpload {
                        key: key.clone(),
                        token,
                        pixels,
                    },
                )
                .unwrap_or_else(|(_, error)| panic!("upload rejected: {error:?}"));
            assert_eq!(admission.evicted.len(), case);
            assert!(renderer.metrics().texture_bytes <= 64);
            renderer
                .prepare(
                    &device,
                    PaneId(1),
                    &[RegionalDraw {
                        key,
                        rect_ndc: [-1.0, 1.0, 1.0, -1.0],
                        uv: [0.0, 0.0, 1.0, 1.0],
                        display: RegionalDisplaySettings {
                            low: [0.0; 3],
                            high: [maximum; 3],
                            gamma: 1.0,
                        },
                    }],
                )
                .unwrap();
            let target = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("regional readback target"),
                size: wgpu::Extent3d {
                    width: 4,
                    height: 2,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = target.create_view(&wgpu::TextureViewDescriptor::default());
            let readback = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("regional readback bytes"),
                size: 512,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            {
                let attachments = [Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })];
                let mut pass = encoder
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        color_attachments: &attachments,
                        ..Default::default()
                    })
                    .forget_lifetime();
                let rect = PixelRect {
                    x: 0,
                    y: 0,
                    width: 4,
                    height: 2,
                };
                renderer.paint(PaneId(1), rect, rect, &mut pass);
            }
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &target,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(2),
                    },
                },
                target.size(),
            );
            queue.submit([encoder.finish()]);
            let (send, receive) = std::sync::mpsc::channel();
            readback
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    send.send(result).unwrap();
                });
            device
                .poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(std::time::Duration::from_secs(30)),
                })
                .unwrap();
            receive.recv().unwrap().unwrap();
            let mapped = readback.slice(..).get_mapped_range().unwrap();
            for (index, expected) in expected.iter().enumerate() {
                let offset = (index / 4) * 256 + (index % 4) * 4;
                let actual = &mapped[offset..offset + 4];
                assert!(
                    actual
                        .iter()
                        .zip(expected)
                        .all(|(a, e)| a.abs_diff(*e) <= 1),
                    "case {case} pixel {index}: {actual:?} != {expected:?}"
                );
            }
            drop(mapped);
            readback.unmap();
        }
        assert_eq!(renderer.metrics().uploads, 2);
        assert_eq!(renderer.metrics().evictions, 1);
        assert_eq!(renderer.metrics().upload_scratch_peak_bytes, 64);
    }
}
