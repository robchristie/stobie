//! Application-owned immutable representation, JPP delivery and shared regional cache.
//! Transport and request generations belong to the caller. No UI framework dependency.
use anyhow::{Result, anyhow, ensure};
use codec::{
    TileRegionRequest,
    ht_indexed::{IndexedLossyHt, TiledLossyHtProfile},
};
pub use emuella_j2k_codestream as codec;
pub use emuella_jpip as jpip;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

mod admission;
use admission::ActiveRequest;
pub use admission::RequestScope;
pub mod validity;

pub fn checked<T, E: std::fmt::Debug>(result: std::result::Result<T, E>) -> Result<T> {
    result.map_err(|e| anyhow!("{e:?}"))
}
/// Validate transport header multiplicity before the protocol owner parses values.
/// Browser-combined values remain intact, so ambiguous geometry cannot be truncated.
pub fn response_fields<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Result<jpip::ResponseFields> {
    let names = ["JPIP-tid", "JPIP-fsiz", "JPIP-roff", "JPIP-rsiz"];
    let mut values = [None; 4];
    for (name, value) in headers {
        if let Some(index) = names.iter().position(|n| name.eq_ignore_ascii_case(n)) {
            if let Some(previous) = values[index] {
                ensure!(
                    previous == value,
                    "conflicting {} response fields",
                    names[index]
                );
            }
            values[index] = Some(value);
        }
    }
    let mut required = [""; 4];
    for (index, value) in values.into_iter().enumerate() {
        required[index] = value.ok_or_else(|| anyhow!("missing {}", names[index]))?;
    }
    checked(jpip::ResponseFields::parse(
        required[0],
        required[1],
        required[2],
        required[3],
    ))
}

pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    pub width: u32,
    pub height: u32,
    pub tile_edge: u32,
    pub decomposition_levels: u8,
    pub bits_per_sample: u8,
    pub components: u16,
    pub bits_per_pixel: f32,
}
impl Profile {
    pub fn codec(&self) -> TiledLossyHtProfile {
        TiledLossyHtProfile {
            width: self.width,
            height: self.height,
            tile_edge: self.tile_edge,
            decomposition_levels: self.decomposition_levels,
            bits_per_sample: self.bits_per_sample,
            components: self.components,
            bits_per_pixel: self.bits_per_pixel,
        }
    }
    pub fn tiles(&self) -> u32 {
        self.width.div_ceil(self.tile_edge) * self.height.div_ceil(self.tile_edge)
    }
}
/// Every field participates in identity. Display stretch is deliberately external.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Identity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validity: Option<validity::ValidityIdentity>,
    pub source_sha256: String,
    pub bands: Vec<u16>,
    pub profile: Profile,
    pub codec_revision: String,
    pub encoding_contract: String,
    pub spatial_policy_sha256: String,
    pub payload_sha256: String,
    pub descriptor_format: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    pub target: String,
    pub tid: String,
    pub identity: Identity,
    pub encoded_bytes: u64,
    pub main_header_bytes: u64,
    /// Hashes authenticate selected descriptors without transferring the full index.
    pub descriptor_sha256: Vec<String>,
}
impl Manifest {
    pub fn seal(&mut self) -> Result<()> {
        self.tid = sha256(&serde_json::to_vec(&(
            &self.identity,
            self.encoded_bytes,
            self.main_header_bytes,
            &self.descriptor_sha256,
        ))?);
        Ok(())
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.target.is_empty()
                && self.target.len() <= 128
                && self
                    .target
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-._".contains(&b)),
            "invalid target"
        );
        let mut expected = self.clone();
        expected.seal()?;
        ensure!(self.tid == expected.tid, "manifest identity mismatch");
        let p = &self.identity.profile;
        let _ = self.sparse()?;
        if let Some(v) = &self.identity.validity {
            v.validate(p)?;
        }
        ensure!(
            self.descriptor_sha256.len() == p.tiles() as usize,
            "descriptor count mismatch"
        );
        ensure!(
            self.identity.bands.len() == usize::from(p.components),
            "band count mismatch"
        );
        Ok(())
    }
    pub fn sparse(&self) -> Result<IndexedLossyHt> {
        checked(IndexedLossyHt::sparse(
            self.identity.profile.codec(),
            self.encoded_bytes,
            0..self.main_header_bytes,
        ))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub discard: u8,
    pub components: Vec<u16>,
}
impl Region {
    pub fn rect(&self) -> TileRegionRequest {
        TileRegionRequest {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }
    pub fn validate(&self, p: &Profile) -> Result<()> {
        ensure!(
            self.width > 0
                && self.height > 0
                && self.x.checked_add(self.width).is_some_and(|x| x <= p.width)
                && self
                    .y
                    .checked_add(self.height)
                    .is_some_and(|y| y <= p.height),
            "invalid region"
        );
        ensure!(
            self.discard <= p.decomposition_levels
                && !self.components.is_empty()
                && self.components.iter().all(|&c| c < p.components)
                && self.components.windows(2).all(|v| v[0] < v[1]),
            "invalid resolution/components"
        );
        Ok(())
    }
    pub fn tiles(&self, p: &Profile) -> Result<Vec<u16>> {
        self.validate(p)?;
        let cols = p.width.div_ceil(p.tile_edge);
        let mut tiles = Vec::new();
        for y in self.y / p.tile_edge..=(self.y + self.height - 1) / p.tile_edge {
            for x in self.x / p.tile_edge..=(self.x + self.width - 1) / p.tile_edge {
                tiles.push((y * cols + x) as u16);
            }
        }
        Ok(tiles)
    }
}
/// Round down to a supported resolution; map effective coordinates to full grid.
pub fn effective_region(
    manifest: &Manifest,
    request: &jpip::Request,
) -> Result<(Region, jpip::ResponseFields)> {
    checked(request.validate())?;
    let p = &manifest.identity.profile;
    ensure!(
        request.target == manifest.target && request.layers.is_none_or(|n| n == 1),
        "unsupported target/quality layer"
    );
    let discard = (0..=p.decomposition_levels)
        .find(|&d| {
            p.width.div_ceil(1 << d) <= request.frame[0]
                && p.height.div_ceil(1 << d) <= request.frame[1]
        })
        .unwrap_or(p.decomposition_levels);
    let scale = 1 << discard;
    let frame = [p.width.div_ceil(scale), p.height.div_ceil(scale)];
    // Preserve the requested normalised window when the server rounds fsiz.
    let mut offset = [0; 2];
    let mut size = [0; 2];
    for axis in 0..2 {
        offset[axis] = (u64::from(request.offset[axis]) * u64::from(frame[axis])
            / u64::from(request.frame[axis])) as u32;
        let end = (u64::from(request.offset[axis] + request.size[axis]) * u64::from(frame[axis]))
            .div_ceil(u64::from(request.frame[axis])) as u32;
        size[axis] = end.min(frame[axis]) - offset[axis];
    }
    let x = offset[0] * scale;
    let y = offset[1] * scale;
    let region = Region {
        x,
        y,
        width: (size[0] * scale).min(p.width - x),
        height: (size[1] * scale).min(p.height - y),
        discard,
        components: request.components.clone(),
    };
    region.validate(p)?;
    Ok((
        region,
        jpip::ResponseFields {
            tid: manifest.tid.clone(),
            frame,
            offset,
            size,
        },
    ))
}
pub fn precinct_key(
    p: &Profile,
    tile: u16,
    component: u16,
    resolution: u8,
) -> Result<jpip::BinKey> {
    checked(jpip::BinKey::new(
        0,
        checked(jpip::precinct_id(
            u64::from(tile),
            u64::from(p.tiles()),
            u64::from(component),
            u64::from(p.components),
            u64::from(resolution),
        ))?,
    ))
}
/// Positioned source ranges for standard bins; packet headers and bodies are contiguous.
pub fn bin_ranges(index: &IndexedLossyHt) -> Result<BTreeMap<jpip::BinKey, Range<u64>>> {
    let p = index.profile();
    let profile = Profile {
        width: p.width,
        height: p.height,
        tile_edge: p.tile_edge,
        decomposition_levels: p.decomposition_levels,
        bits_per_sample: p.bits_per_sample,
        components: p.components,
        bits_per_pixel: p.bits_per_pixel,
    };
    let mut ranges = BTreeMap::new();
    ranges.insert(checked(jpip::BinKey::new(6, 0))?, index.main_header());
    ranges.insert(checked(jpip::BinKey::new(8, 0))?, 0..0);
    let tiles: BTreeSet<_> = index.precincts().iter().map(|p| p.tile).collect();
    for (tile, range) in tiles.iter().zip(index.tile_headers()) {
        ranges.insert(
            checked(jpip::BinKey::new(2, u64::from(*tile)))?,
            range.clone(),
        );
    }
    for packet in index.precincts() {
        ranges.insert(
            precinct_key(&profile, packet.tile, packet.component, packet.resolution)?,
            packet.header.start..packet.body.end,
        );
    }
    Ok(ranges)
}
pub fn demands(index: &IndexedLossyHt, region: &Region) -> Result<Vec<jpip::Demand>> {
    let plan = checked(index.plan(region.rect(), region.discard, &region.components))?;
    let mut keys = vec![
        checked(jpip::BinKey::new(6, 0))?,
        checked(jpip::BinKey::new(8, 0))?,
    ];
    let selected_tiles: BTreeSet<_> = plan
        .precinct_indices()
        .iter()
        .map(|&i| index.precincts()[i].tile)
        .collect();
    for tile in selected_tiles {
        keys.push(checked(jpip::BinKey::new(2, u64::from(tile)))?);
    }
    let p = index.profile();
    let tiles =
        u64::from(p.width.div_ceil(p.tile_edge)) * u64::from(p.height.div_ceil(p.tile_edge));
    for &i in plan.precinct_indices() {
        let v = &index.precincts()[i];
        keys.push(checked(jpip::BinKey::new(
            0,
            checked(jpip::precinct_id(
                u64::from(v.tile),
                tiles,
                u64::from(v.component),
                u64::from(p.components),
                u64::from(v.resolution),
            ))?,
        ))?);
    }
    Ok(keys
        .into_iter()
        .map(|key| jpip::Demand { key, prefix: None })
        .collect())
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ClientLimits {
    pub compressed_bytes: usize,
    pub descriptor_bytes: usize,
    pub representations: usize,
    pub decode_workspace_bytes: u64,
}
impl Default for ClientLimits {
    fn default() -> Self {
        Self {
            compressed_bytes: 64 << 20,
            descriptor_bytes: 16 << 20,
            representations: 32,
            decode_workspace_bytes: 64 << 20,
        }
    }
}
struct Resident {
    manifest: Manifest,
    cache: jpip::Cache,
    descriptors: BTreeMap<u16, Vec<u8>>,
    masks: BTreeMap<(u8, u16), Vec<u8>>,
    index: IndexedLossyHt,
    used: u64,
}
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct ClientMetrics {
    pub received_jpp_bytes: u64,
    pub received_mask_bytes: u64,
    pub peak_mask_bytes: usize,
    pub mask_evictions: u64,
    pub received_descriptor_bytes: u64,
    pub decode_count: u64,
    pub decoded_pixels: u64,
    pub selected_code_blocks: u64,
    pub selected_block_coefficients: u64,
    pub peak_codec_workspace_bytes: u64,
    pub synthesis_coefficients_loaded: u64,
    pub synthesis_horizontal_values: u64,
    pub synthesis_vertical_values: u64,
    pub synthesis_lifting_updates: u64,
    pub synthesis_output_samples: u64,

    pub compressed_read_bytes: u64,
    pub representation_evictions: u64,
    pub compressed_bin_evictions: u64,
    pub peak_compressed_bytes: usize,
    pub peak_descriptor_bytes: usize,
}
pub struct SharedClient {
    limits: ClientLimits,
    entries: BTreeMap<String, Resident>,
    clock: u64,
    pub metrics: ClientMetrics,
    active_request: Option<ActiveRequest>,
    next_request: u64,
}
pub struct ResponseReader {
    tid: String,
    decoder: jpip::Decoder,
    scope: Option<RequestScope>,
}
#[derive(Debug)]
pub struct DecodedRegion {
    pub width: u32,
    pub height: u32,
    pub bits_per_sample: u8,
    pub components: Vec<u16>,
    pub planes: Vec<Vec<u8>>,
    pub validity: Option<Vec<u8>>,
}
impl SharedClient {
    pub fn new(limits: ClientLimits) -> Self {
        Self {
            limits,
            entries: BTreeMap::new(),
            clock: 0,
            metrics: ClientMetrics::default(),
            active_request: None,
            next_request: 0,
        }
    }
    pub fn register(&mut self, manifest: Manifest) -> Result<()> {
        if let Some(a) = &self.active_request {
            ensure!(a.tid == manifest.tid, "registration outside active request");
        }
        manifest.validate()?;
        ensure!(
            self.limits.representations > 0,
            "no representation capacity"
        );
        if let Some(existing) = self.entries.get(&manifest.tid) {
            ensure!(
                existing.manifest.identity == manifest.identity,
                "identity collision"
            );
            return Ok(());
        }
        while self.entries.len() >= self.limits.representations {
            self.evict_other("")?;
        }
        let mut cache = jpip::Cache::new(jpip::CacheLimits {
            bytes: self.limits.compressed_bytes,
            bins: admission::BIN_LIMIT,
            ranges_per_bin: 16,
            max_bin_length: admission::MAX_BIN_LENGTH,
        });
        checked(cache.bind_identity(&manifest.tid))?;
        let index = manifest.sparse()?;
        self.entries.insert(
            manifest.tid.clone(),
            Resident {
                manifest,
                cache,
                descriptors: BTreeMap::new(),
                masks: BTreeMap::new(),
                index,
                used: self.clock,
            },
        );
        Ok(())
    }
    fn evict_other(&mut self, keep: &str) -> Result<()> {
        let key = self
            .entries
            .iter()
            .filter(|(k, _)| k.as_str() != keep)
            .min_by_key(|(_, r)| r.used)
            .map(|(k, _)| k.clone())
            .ok_or_else(|| anyhow!("global budget exceeded"))?;
        self.entries.remove(&key);
        self.metrics.representation_evictions += 1;
        Ok(())
    }
    fn touch(&mut self, tid: &str) -> Result<&mut Resident> {
        self.clock += 1;
        let r = self
            .entries
            .get_mut(tid)
            .ok_or_else(|| anyhow!("representation absent; register again after eviction"))?;
        r.used = self.clock;
        Ok(r)
    }
    pub fn resident_bytes(&self) -> (usize, usize) {
        (
            self.entries
                .values()
                .map(|r| r.cache.bytes() + r.masks.values().map(Vec::len).sum::<usize>())
                .sum(),
            self.entries
                .values()
                .map(|r| {
                    r.descriptors.values().map(Vec::len).sum::<usize>()
                        + r.index.retained_heap_bytes() as usize
                })
                .sum(),
        )
    }
    // Admitted cache high-water marks; codec admission scratch and transport
    // buffers have separate limits and remain part of process memory evidence.
    fn observe_resident(&mut self) {
        let mask_bytes = self.mask_bytes();
        self.metrics.peak_mask_bytes = self.metrics.peak_mask_bytes.max(mask_bytes);
        let (compressed, descriptors) = self.resident_bytes();
        self.metrics.peak_compressed_bytes = self.metrics.peak_compressed_bytes.max(compressed);
        self.metrics.peak_descriptor_bytes = self.metrics.peak_descriptor_bytes.max(descriptors);
    }
    pub fn mask_bytes(&self) -> usize {
        self.entries
            .values()
            .flat_map(|r| r.masks.values())
            .map(Vec::len)
            .sum()
    }
    pub fn missing_masks(&mut self, tid: &str, region: &Region) -> Result<Vec<u16>> {
        let limit = self.limits.compressed_bytes;
        let r = self.touch(tid)?;
        let tiles = region.tiles(&r.manifest.identity.profile)?;
        if r.manifest.identity.validity.is_none() {
            return Ok(Vec::new());
        }
        ensure!(tiles.len() <= 64, "mask regional tile limit");
        let mut bytes = 0usize;
        for &t in &tiles {
            let (w, h) = validity::tile_size(&r.manifest.identity.profile, t, region.discard)?;
            bytes += (w * h).div_ceil(8) as usize
                * usize::from(r.manifest.identity.profile.components)
                * if region.discard == 0 { 1 } else { 2 };
        }
        ensure!(bytes <= limit, "regional masks exceed global budget");
        Ok(tiles
            .into_iter()
            .filter(|t| !r.masks.contains_key(&(region.discard, *t)))
            .collect())
    }
    pub fn install_mask(&mut self, tid: &str, tile: u16, discard: u8, bytes: &[u8]) -> Result<()> {
        if let Some(a) = &self.active_request {
            ensure!(
                a.tid == tid && a.masks.contains(&(discard, tile)),
                "mask outside active request"
            );
        }
        self.metrics.received_mask_bytes += bytes.len() as u64;
        let r = self.touch(tid)?;
        r.manifest
            .identity
            .validity
            .as_ref()
            .ok_or_else(|| anyhow!("mask not declared"))?
            .check(&r.manifest.identity.profile, tile, discard, bytes)?;
        ensure!(
            bytes.len() <= self.limits.compressed_bytes,
            "mask exceeds global budget"
        );
        if self.touch(tid)?.masks.contains_key(&(discard, tile)) {
            return Ok(());
        }
        if self.active_request.is_some() {
            ensure!(
                self.resident_bytes().0.saturating_add(bytes.len()) <= self.limits.compressed_bytes,
                "working-set admission: mask reservation exceeded"
            );
        }
        // Legacy unscoped eviction policy.
        // Reclaim mask entries first, including the active representation. A region
        // whose complete mask working set cannot fit fails before publication.
        while self.resident_bytes().0.saturating_add(bytes.len()) > self.limits.compressed_bytes {
            let victim = self
                .entries
                .iter()
                .filter(|(_, r)| !r.masks.is_empty())
                .min_by_key(|(_, r)| r.used)
                .map(|(k, r)| (k.clone(), *r.masks.keys().next().unwrap()));
            if let Some((key, mask)) = victim {
                self.entries.get_mut(&key).unwrap().masks.remove(&mask);
                self.metrics.mask_evictions += 1;
            } else {
                let r = self.touch(tid)?;
                if let Some((key, _)) = r.cache.model().into_iter().next() {
                    r.cache.evict(key);
                    self.metrics.compressed_bin_evictions += 1;
                } else {
                    self.evict_other(tid)?;
                }
            }
        }
        self.touch(tid)?
            .masks
            .insert((discard, tile), bytes.to_vec());
        self.observe_resident();
        Ok(())
    }
    pub fn missing_tiles(&mut self, tid: &str, region: &Region) -> Result<Vec<u16>> {
        let r = self.touch(tid)?;
        Ok(region
            .tiles(&r.manifest.identity.profile)?
            .into_iter()
            .filter(|t| !r.descriptors.contains_key(t))
            .collect())
    }
    pub fn install_descriptor(&mut self, tid: &str, tile: u16, bytes: &[u8]) -> Result<()> {
        ensure!(
            self.active_request.is_none(),
            "descriptor admission during active request"
        );
        self.metrics.received_descriptor_bytes += bytes.len() as u64;
        let r = self.touch(tid)?;
        ensure!(
            r.manifest.descriptor_sha256.get(usize::from(tile)) == Some(&sha256(bytes)),
            "descriptor digest mismatch"
        );
        if r.descriptors.contains_key(&tile) {
            return Ok(());
        }
        // Build a replacement before admitting memory; old selected metadata can be evicted.
        let mut index = r.manifest.sparse()?;
        checked(index.import_tile_descriptor(bytes))?;
        ensure!(
            index.precincts().iter().all(|p| p.tile == tile),
            "wrong descriptor tile"
        );
        let minimum = index.retained_heap_bytes() as usize + bytes.len();
        ensure!(
            minimum <= self.limits.descriptor_bytes,
            "descriptor exceeds global budget"
        );
        let r = self.touch(tid)?;
        checked(r.index.import_tile_descriptor(bytes))?;
        r.descriptors.insert(tile, bytes.to_vec());
        while self.resident_bytes().1 > self.limits.descriptor_bytes {
            // Descriptor pressure must never discard reusable compressed bins.
            let victim = self
                .entries
                .iter()
                .filter(|(key, r)| key.as_str() != tid && !r.descriptors.is_empty())
                .min_by_key(|(_, r)| r.used)
                .map(|(key, _)| key.clone());
            if let Some(victim) = victim {
                let r = self.entries.get_mut(&victim).expect("selected resident");
                r.descriptors.clear();
                r.index = r.manifest.sparse()?;
            } else {
                let r = self.touch(tid)?;
                r.descriptors.retain(|&t, _| t == tile);
                r.index = index;
                break;
            }
        }
        self.observe_resident();
        Ok(())
    }
    pub fn request(
        &mut self,
        tid: &str,
        region: &Region,
        max_length: u64,
    ) -> Result<jpip::Request> {
        self.check_request(tid, region)?;
        let r = self.touch(tid)?;
        let p = &r.manifest.identity.profile;
        region.validate(p)?;
        let scale = 1 << region.discard;
        let offset = [region.x / scale, region.y / scale];
        let size = [
            (region.x + region.width).div_ceil(scale) - offset[0],
            (region.y + region.height).div_ceil(scale) - offset[1],
        ];
        let wanted: BTreeSet<_> = demands(&r.index, region)?
            .into_iter()
            .map(|d| d.key)
            .collect();
        Ok(jpip::Request {
            target: r.manifest.target.clone(),
            tid: tid.into(),
            frame: [p.width.div_ceil(scale), p.height.div_ceil(scale)],
            offset,
            size,
            components: region.components.clone(),
            layers: Some(1),
            max_length,
            extended: true,
            model: r
                .cache
                .model()
                .into_iter()
                .filter(|(k, _)| wanted.contains(k))
                .collect(),
        })
    }
    /// Validate every transport field before creating a reader or touching cache state.
    /// The caller must still reject stale application generations before this call.
    pub fn begin_response_headers<'a>(
        &mut self,
        tid: &str,
        headers: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<ResponseReader> {
        let fields = response_fields(headers)?;
        self.begin_response(tid, &fields)
    }
    /// Call only after transport generation checks and case-insensitive header parsing.
    pub fn begin_response(
        &mut self,
        tid: &str,
        fields: &jpip::ResponseFields,
    ) -> Result<ResponseReader> {
        ensure!(
            fields.tid == tid,
            "response identity changed; reload manifest"
        );
        if let Some(a) = &self.active_request {
            ensure!(a.tid == tid, "response outside active request");
        }
        self.touch(tid)?;
        Ok(ResponseReader {
            tid: tid.into(),
            decoder: jpip::Decoder::new(8 << 20),
            scope: self.active_request.as_ref().map(|a| a.scope),
        })
    }
    pub fn receive(&mut self, reader: &mut ResponseReader, bytes: &[u8]) -> Result<()> {
        ensure!(
            reader.scope == self.active_request.as_ref().map(|a| a.scope),
            "stale request response"
        );
        self.metrics.received_jpp_bytes += bytes.len() as u64;
        let mut admission_error = None;
        let tid = reader.tid.clone();
        let result = reader.decoder.push(bytes, |event| {
            if let jpip::Event::Data(message) = event {
                if let Some(a) = &self.active_request {
                    let length = a.bin_length(message.key);
                    let end = message.offset.checked_add(message.bytes.len() as u64);
                    if !length.zip(end).is_some_and(|(length, end)| end <= length && (!message.final_bin || end == length)) {
                        admission_error = Some("working-set admission: response bin outside authenticated demand lengths");
                        return Err(jpip::Error::Limit);
                    }
                    let r = self.entries.get_mut(&tid).ok_or(jpip::Error::Identity)?;
                    // Full payload and bin-count space were reserved at admission.
                    // The owner still validates overlap, final lengths and range count.
                    let before = r.cache.eviction_count();
                    let result = r.cache.insert(message);
                    let evicted = r.cache.eviction_count() - before;
                    self.metrics.compressed_bin_evictions += evicted;
                    result?;
                    if evicted != 0 {
                        admission_error = Some("working-set admission: reserved dependencies were evicted");
                        return Err(jpip::Error::Limit);
                    }
                    self.observe_resident();
                    return Ok(());
                }
                // Reserve only actual emitted payload. Headers and EOR bodies
                // consume transport space, not compressed-cache occupancy.
                while self.entries.len() > 1
                    && self.resident_bytes().0.saturating_add(message.bytes.len())
                        > self.limits.compressed_bytes
                {
                    self.evict_other(&tid).map_err(|_| jpip::Error::Limit)?;
                }
                if self.mask_bytes() > 0 {
                    while self.resident_bytes().0.saturating_add(message.bytes.len())
                        > self.limits.compressed_bytes
                    {
                        let r = self.touch(&tid).map_err(|_| jpip::Error::Identity)?;
                        let key = r
                            .cache
                            .model()
                            .into_iter()
                            .find(|(key, _)| *key != message.key)
                            .map(|(key, _)| key)
                            .ok_or(jpip::Error::Limit)?;
                        r.cache.evict(key);
                        self.metrics.compressed_bin_evictions += 1;
                    }
                }
                let r = self.touch(&tid).map_err(|_| jpip::Error::Identity)?;
                let before = r.cache.eviction_count();
                let result = r.cache.insert(message);
                let evicted = r.cache.eviction_count() - before;
                self.metrics.compressed_bin_evictions += evicted;
                result?;
                self.observe_resident();
            }
            Ok(())
        });
        if let Some(error) = admission_error {
            return Err(anyhow!(error));
        }
        checked(result)?;
        ensure!(
            self.resident_bytes().0 <= self.limits.compressed_bytes,
            "global compressed budget"
        );
        Ok(())
    }
    pub fn finish(&self, reader: ResponseReader) -> Result<()> {
        ensure!(
            reader.scope == self.active_request.as_ref().map(|a| a.scope),
            "stale request response"
        );
        checked(reader.decoder.finish())
    }
    /// True only when all selected descriptors and mandatory bins are complete.
    /// This performs no entropy decode. A subsequent decode error is permanent for
    /// that representation/region and should not trigger empty transport retries.
    pub fn ready(&mut self, tid: &str, region: &Region) -> Result<bool> {
        self.check_request(tid, region)?;
        if !self.missing_tiles(tid, region)?.is_empty()
            || !self.missing_masks(tid, region)?.is_empty()
        {
            return Ok(false);
        }
        let r = self.touch(tid)?;
        Ok(demands(&r.index, region)?
            .into_iter()
            .all(|d| r.cache.is_complete(d.key)))
    }
    pub fn decode(&mut self, tid: &str, region: &Region) -> Result<DecodedRegion> {
        self.check_request(tid, region)?;
        let workspace_limit = self.limits.decode_workspace_bytes;
        ensure!(
            self.missing_masks(tid, region)?.is_empty(),
            "required masks incomplete"
        );
        let r = self.touch(tid)?;
        let plan = checked(
            r.index
                .plan(region.rect(), region.discard, &region.components),
        )?;
        let ranges = bin_ranges(&r.index)?;
        for demand in demands(&r.index, region)? {
            ensure!(
                r.cache.is_complete(demand.key),
                "compressed dependencies incomplete"
            );
        }
        let mut reads = 0;
        let mut read_error = None;
        let mut workspace =
            codec::ht_lossy::LossyHtSpatialRegionWorkspace::with_maximum_bytes(workspace_limit);
        let decoded = plan.decode_with_report(
            |offset, out| {
                let (key, range) = ranges
                    .iter()
                    .find(|(k, r)| {
                        k.class == 0 && r.start <= offset && offset + out.len() as u64 <= r.end
                    })
                    .ok_or_else(|| {
                        read_error =
                            Some(format!("no precinct covers source {offset}+{}", out.len()));
                        codec::CodestreamError::SizeOverflow
                    })?;
                r.cache
                    .read(*key, offset - range.start, out)
                    .map_err(|error| {
                        read_error = Some(format!(
                            "cached precinct {key:?} offset {}: {error:?}",
                            offset - range.start
                        ));
                        codec::CodestreamError::SizeOverflow
                    })?;
                reads += out.len() as u64;
                Ok(())
            },
            &mut workspace,
        );
        if let Some(error) = read_error {
            return Err(anyhow!(error));
        }
        let (planes, report) = checked(decoded)?;
        let output = plan.output_region();
        let blocks = plan.selected_code_blocks();
        let coefficients = plan.selected_block_coefficients();
        let workspace_bytes = plan.required_workspace_bytes();
        let bits = r.manifest.identity.profile.bits_per_sample;
        let validity = if r.manifest.identity.validity.is_some() {
            Some(validity::combine_region(
                &r.manifest.identity.profile,
                region,
                |tile, discard| {
                    r.masks
                        .get(&(discard, tile))
                        .map(Vec::as_slice)
                        .ok_or_else(|| anyhow!("required mask absent"))
                },
            )?)
        } else {
            None
        };
        self.metrics.synthesis_coefficients_loaded += report.work.coefficients_loaded;
        self.metrics.synthesis_horizontal_values += report.work.horizontal_values;
        self.metrics.synthesis_vertical_values += report.work.vertical_values;
        self.metrics.synthesis_lifting_updates += report.work.lifting_updates;
        self.metrics.synthesis_output_samples += report.work.output_samples;
        self.metrics.decode_count += 1;
        self.metrics.decoded_pixels += u64::from(output.width) * u64::from(output.height);
        self.metrics.selected_code_blocks += blocks as u64;
        self.metrics.selected_block_coefficients += coefficients;
        self.metrics.peak_codec_workspace_bytes =
            self.metrics.peak_codec_workspace_bytes.max(workspace_bytes);
        self.metrics.compressed_read_bytes += reads;
        Ok(DecodedRegion {
            width: output.width,
            height: output.height,
            bits_per_sample: bits,
            components: region.components.clone(),
            planes,
            validity,
        })
    }
}

#[cfg(test)]
mod admission_tests;
