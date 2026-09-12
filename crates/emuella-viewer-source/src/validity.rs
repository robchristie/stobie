//! Codec-neutral source-grid validity sidecars. No reconstructed sample tests.
use crate::{Profile, Region, sha256};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

pub const POLICY: &str = "source-validity-v1";
pub const COMPACT_POLICY: &str = "source-validity-v2";
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ValidityIdentity {
    pub source_sha256: String,
    pub bands: Vec<u16>,
    pub policy: String,
    /// Level-major, then row-major source tile. V1: SHA-256; v2: `state:SHA-256`.
    /// V2 states: 0 all-invalid, 1 all-valid, 2 legacy bitmap after a state byte.
    pub tile_sha256: Vec<Vec<String>>,
}
impl ValidityIdentity {
    pub fn validate(&self, p: &Profile) -> Result<()> {
        ensure!(
            matches!(self.policy.as_str(), POLICY | COMPACT_POLICY),
            "unsupported validity policy"
        );
        ensure!(
            self.source_sha256.len() == 64
                && self.source_sha256.bytes().all(|v| v.is_ascii_hexdigit()),
            "invalid mask source digest"
        );
        ensure!(
            self.bands.len() == usize::from(p.components) && self.bands.iter().all(|&b| b > 0),
            "invalid mask source bands"
        );
        ensure!(
            self.tile_sha256.len() == usize::from(p.decomposition_levels) + 1
                && self
                    .tile_sha256
                    .iter()
                    .all(|v| v.len() == p.tiles() as usize
                        && v.iter().all(|s| self.entry(s).is_ok())),
            "invalid mask digest catalogue"
        );
        ensure!(
            p.tile_edge.is_multiple_of(1 << p.decomposition_levels),
            "unaligned mask tiles"
        );
        Ok(())
    }
    /// Additional retained catalogue heap introduced by compact state tags.
    pub fn compact_metadata_bytes(&self) -> usize {
        if self.policy == COMPACT_POLICY {
            self.tile_sha256
                .iter()
                .flatten()
                .map(|s| s.capacity().saturating_sub(64))
                .sum()
        } else {
            0
        }
    }
    fn entry<'a>(&self, entry: &'a str) -> Result<(Option<u8>, &'a str)> {
        let (state, digest) = match self.policy.as_str() {
            POLICY => (None, entry),
            COMPACT_POLICY => {
                let (tag, digest) = entry
                    .split_once(':')
                    .ok_or_else(|| anyhow::anyhow!("mask state absent"))?;
                let state = match tag {
                    "0" => 0,
                    "1" => 1,
                    "2" => 2,
                    _ => anyhow::bail!("invalid mask state"),
                };
                (Some(state), digest)
            }
            _ => anyhow::bail!("unsupported validity policy"),
        };
        ensure!(
            digest.len() == 64
                && digest.bytes().all(|c| c.is_ascii_hexdigit())
                && (state.is_none() || digest.bytes().all(|c| !c.is_ascii_uppercase())),
            "invalid mask digest"
        );
        Ok((state, digest))
    }
    fn declared(&self, tile: u16, discard: u8) -> Result<(Option<u8>, &str)> {
        self.entry(
            self.tile_sha256
                .get(usize::from(discard))
                .and_then(|v| v.get(usize::from(tile)))
                .ok_or_else(|| anyhow::anyhow!("required mask absent"))?,
        )
    }
    pub fn byte_len(&self, p: &Profile, tile: u16, discard: u8) -> Result<usize> {
        let (w, h) = tile_size(p, tile, discard)?;
        let (state, _) = self.declared(tile, discard)?;
        Ok(match state {
            Some(0 | 1) => 1,
            _ => {
                (w * h).div_ceil(8) as usize
                    * usize::from(p.components)
                    * if discard == 0 { 1 } else { 2 }
                    + usize::from(state.is_some())
            }
        })
    }
    pub fn check(&self, p: &Profile, tile: u16, discard: u8, bytes: &[u8]) -> Result<()> {
        ensure!(
            bytes.len() == self.byte_len(p, tile, discard)?,
            "mask length mismatch"
        );
        let (state, digest) = self.declared(tile, discard)?;
        ensure!(digest == sha256(bytes), "mask digest mismatch");
        if let Some(state) = state {
            ensure!(bytes.first() == Some(&state), "mask state mismatch");
            if state != 2 {
                return Ok(());
            }
        }
        let (w, h) = tile_size(p, tile, discard)?;
        check_bitmap(
            (w * h) as usize,
            discard,
            &bytes[usize::from(state.is_some())..],
        )
    }
}
fn check_bitmap(pixels: usize, discard: u8, bytes: &[u8]) -> Result<()> {
    let plane = pixels.div_ceil(8);
    let planes = if discard == 0 { 1 } else { 2 };
    for band in bytes.chunks_exact(plane * planes) {
        for b in band.chunks_exact(plane) {
            if !pixels.is_multiple_of(8) {
                ensure!(b[plane - 1] >> (pixels % 8) == 0, "mask padding");
            }
        }
        if planes == 2 {
            ensure!(
                band[..plane]
                    .iter()
                    .zip(&band[plane..])
                    .all(|(a, b)| a & !b == 0),
                "mask all without any"
            );
        }
    }
    Ok(())
}
pub fn tile_size(p: &Profile, tile: u16, discard: u8) -> Result<(u32, u32)> {
    ensure!(
        u32::from(tile) < p.tiles() && discard <= p.decomposition_levels,
        "invalid mask tile/level"
    );
    let cols = p.width.div_ceil(p.tile_edge);
    let x = u32::from(tile) % cols * p.tile_edge;
    let y = u32::from(tile) / cols * p.tile_edge;
    Ok((
        (p.width - x).min(p.tile_edge).div_ceil(1 << discard),
        (p.height - y).min(p.tile_edge).div_ceil(1 << discard),
    ))
}
pub fn bit(bytes: &[u8], i: usize) -> bool {
    bytes[i / 8] & (1 << (i % 8)) != 0
}
fn set(bytes: &mut [u8], i: usize, value: bool) {
    if value {
        bytes[i / 8] |= 1 << (i % 8);
    }
}
/// Generate all levels from one native tile of GDAL nonzero validity bytes.
pub fn encode_tile(width: u32, height: u32, levels: u8, bands: &[Vec<u8>]) -> Result<Vec<Vec<u8>>> {
    encode_bitmap_tile(width, height, levels, bands, false)
}
fn encode_bitmap_tile(
    width: u32,
    height: u32,
    levels: u8,
    bands: &[Vec<u8>],
    compact: bool,
) -> Result<Vec<Vec<u8>>> {
    ensure!(
        width > 0
            && width <= 1024
            && height > 0
            && height <= 1024
            && levels <= 6
            && (1..=3).contains(&bands.len())
            && bands.iter().all(|b| b.len() == (width * height) as usize),
        "invalid native masks"
    );
    let mut output = Vec::new();
    for d in 0..=levels {
        let scale = 1 << d;
        let w = width.div_ceil(scale);
        let h = height.div_ceil(scale);
        let plane = (w * h).div_ceil(8) as usize;
        let planes = if d == 0 { 1 } else { 2 };
        let prefix = usize::from(compact);
        let mut bytes = vec![0; prefix + plane * planes * bands.len()];
        if compact {
            bytes[0] = 2;
        }
        for (c, band) in bands.iter().enumerate() {
            for y in 0..h {
                for x in 0..w {
                    let mut all = true;
                    let mut any = false;
                    for sy in y * scale..((y + 1) * scale).min(height) {
                        for sx in x * scale..((x + 1) * scale).min(width) {
                            let valid = band[(sy * width + sx) as usize] != 0;
                            all &= valid;
                            any |= valid;
                        }
                    }
                    let i = (y * w + x) as usize;
                    set(&mut bytes[prefix + c * plane * planes..], i, all);
                    if d > 0 {
                        set(&mut bytes[prefix + c * plane * planes + plane..], i, any);
                    }
                }
            }
        }
        output.push(bytes);
    }
    Ok(output)
}
/// Compact whole-tile states; constant source tiles never allocate packed tile planes.
pub fn encode_compact_tile(
    width: u32,
    height: u32,
    levels: u8,
    bands: &[Vec<u8>],
) -> Result<Vec<Vec<u8>>> {
    ensure!(
        width > 0
            && width <= 1024
            && height > 0
            && height <= 1024
            && levels <= 6
            && (1..=3).contains(&bands.len())
            && bands.iter().all(|b| b.len() == (width * height) as usize),
        "invalid native masks"
    );
    if bands.iter().flatten().all(|&v| v == 0) {
        return Ok(vec![vec![0]; usize::from(levels) + 1]);
    }
    if bands.iter().flatten().all(|&v| v != 0) {
        return Ok(vec![vec![1]; usize::from(levels) + 1]);
    }
    encode_bitmap_tile(width, height, levels, bands, true)
}
/// Authenticate the state as well as its exact encoded bytes in the existing catalogue.
pub fn catalogue_entry(policy: &str, bytes: &[u8]) -> Result<String> {
    match policy {
        POLICY => Ok(sha256(bytes)),
        COMPACT_POLICY => {
            let state = bytes
                .first()
                .filter(|&&s| s <= 2)
                .ok_or_else(|| anyhow::anyhow!("invalid mask state"))?;
            Ok(format!("{state}:{}", sha256(bytes)))
        }
        _ => anyhow::bail!("unsupported validity policy"),
    }
}
/// Display-only combination. Quality callers retain each band's all/any plane.
pub fn combine_region<'a>(
    p: &Profile,
    region: &Region,
    get: impl FnMut(u16, u8) -> Result<&'a [u8]>,
) -> Result<Vec<u8>> {
    combine_region_encoded(p, region, false, get)
}
pub fn combine_region_encoded<'a>(
    p: &Profile,
    region: &Region,
    compact: bool,
    mut get: impl FnMut(u16, u8) -> Result<&'a [u8]>,
) -> Result<Vec<u8>> {
    region.validate(p)?;
    let scale = 1 << region.discard;
    let x0 = region.x.div_ceil(scale);
    let y0 = region.y.div_ceil(scale);
    let w = (region.x + region.width).div_ceil(scale) - x0;
    let h = (region.y + region.height).div_ceil(scale) - y0;
    let edge = p.tile_edge / scale;
    let cols = p.width.div_ceil(p.tile_edge);
    let mut output = vec![1; (w * h) as usize];
    for tile in region.tiles(p)? {
        let encoded = get(tile, region.discard)?;
        let (constant, bytes) = if compact {
            match encoded.split_first() {
                Some((&s @ (0 | 1), rest)) if rest.is_empty() => (Some(s), rest),
                Some((&2, rest)) => (None, rest),
                _ => anyhow::bail!("invalid mask state"),
            }
        } else {
            (None, encoded)
        };
        let (tw, th) = tile_size(p, tile, region.discard)?;
        let tx = u32::from(tile) % cols * edge;
        let ty = u32::from(tile) / cols * edge;
        let plane = (tw * th).div_ceil(8) as usize;
        let stride = plane * if region.discard == 0 { 1 } else { 2 };
        ensure!(
            constant.is_some() || bytes.len() == stride * usize::from(p.components),
            "mask length mismatch"
        );
        for y in y0.max(ty)..(y0 + h).min(ty + th) {
            for x in x0.max(tx)..(x0 + w).min(tx + tw) {
                let i = ((y - ty) * tw + x - tx) as usize;
                output[((y - y0) * w + x - x0) as usize] = constant.unwrap_or_else(|| {
                    u8::from(
                        region
                            .components
                            .iter()
                            .all(|&c| bit(&bytes[usize::from(c) * stride..], i)),
                    )
                });
            }
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_constants_mixed_all_levels_and_clipped_regions_match_legacy() {
        let p = Profile {
            width: 517,
            height: 261,
            tile_edge: 256,
            decomposition_levels: 6,
            bits_per_sample: 8,
            components: 3,
            bits_per_pixel: 3.0,
        };
        for uniform in [Some(0), Some(255), None] {
            let mut compact = Vec::new();
            let mut legacy = Vec::new();
            for tile in 0..p.tiles() as u16 {
                let (w, h) = tile_size(&p, tile, 0).unwrap();
                // Component order is deliberately 3,2,1, including differing validity.
                let bands: Vec<_> = [3, 2, 1]
                    .into_iter()
                    .map(|c| {
                        (0..w * h)
                            .map(|i| {
                                uniform.unwrap_or(u8::from((i + c + u32::from(tile)) % 19 != 0))
                            })
                            .collect()
                    })
                    .collect();
                compact.push(encode_compact_tile(w, h, 6, &bands).unwrap());
                legacy.push(encode_tile(w, h, 6, &bands).unwrap());
            }
            let identity = ValidityIdentity {
                source_sha256: "a".repeat(64),
                bands: vec![3, 2, 1],
                policy: COMPACT_POLICY.into(),
                tile_sha256: (0..7)
                    .map(|d| {
                        compact
                            .iter()
                            .map(|t| catalogue_entry(COMPACT_POLICY, &t[d]).unwrap())
                            .collect()
                    })
                    .collect(),
            };
            identity.validate(&p).unwrap();
            for tile in 0..p.tiles() as u16 {
                for d in 0..7 {
                    let bytes = &compact[tile as usize][d as usize];
                    identity.check(&p, tile, d, bytes).unwrap();
                    assert_eq!(identity.byte_len(&p, tile, d).unwrap(), bytes.len());
                    assert_eq!(
                        bytes.capacity(),
                        bytes.len(),
                        "encoded mask allocation is exact"
                    );
                    if uniform.is_some() {
                        assert_eq!(bytes.len(), 1);
                    } else {
                        assert_eq!(&bytes[1..], legacy[tile as usize][d as usize]);
                    }
                }
            }
            for d in 0..7 {
                for components in [vec![0], vec![1], vec![0, 1, 2]] {
                    for (x, y, width, height) in
                        [(0, 0, 517, 261), (251, 249, 266, 12), (512, 256, 5, 5)]
                    {
                        let region = Region {
                            x,
                            y,
                            width,
                            height,
                            discard: d,
                            components: components.clone(),
                        };
                        let a =
                            combine_region(&p, &region, |t, d| Ok(&legacy[t as usize][d as usize]))
                                .unwrap();
                        let b = combine_region_encoded(&p, &region, true, |t, d| {
                            Ok(&compact[t as usize][d as usize])
                        })
                        .unwrap();
                        assert_eq!(a, b);
                        assert!(
                            combine_region_encoded(&p, &region, true, |_, _| anyhow::bail!(
                                "required mask absent"
                            ))
                            .is_err()
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn compact_catalogue_and_payload_are_strict_and_fail_closed() {
        let p = Profile {
            width: 3,
            height: 3,
            tile_edge: 256,
            decomposition_levels: 1,
            bits_per_sample: 8,
            components: 1,
            bits_per_pixel: 2.0,
        };
        let mut v = ValidityIdentity {
            source_sha256: "a".repeat(64),
            bands: vec![1],
            policy: COMPACT_POLICY.into(),
            tile_sha256: vec![vec![catalogue_entry(COMPACT_POLICY, &[1]).unwrap()]; 2],
        };
        v.validate(&p).unwrap();
        assert!(v.check(&p, 0, 0, &[]).is_err());
        assert!(v.check(&p, 0, 0, &[0]).is_err());
        assert!(v.check(&p, 0, 0, &[1, 0]).is_err());
        assert!(v.byte_len(&p, 1, 0).is_err());
        for tag in ["", "00", "3", "-1", " 1", "1:"] {
            v.tile_sha256[0][0] = format!("{tag}:{}", sha256(&[1]));
            assert!(v.validate(&p).is_err());
        }
        v.tile_sha256[0][0] = format!("0:{}", sha256(&[1]));
        assert!(v.check(&p, 0, 0, &[1]).is_err());
        for (d, bytes) in [(0, vec![2, 255, 255]), (1, vec![2, 1, 0])] {
            v.tile_sha256[d][0] = catalogue_entry(COMPACT_POLICY, &bytes).unwrap();
            assert!(v.check(&p, 0, d as u8, &bytes).is_err());
        }
    }
    #[test]
    fn clipped_footprints_keep_all_any_distinct_and_padding_canonical() {
        let levels = encode_tile(
            3,
            3,
            2,
            &[vec![255, 255, 255, 255, 0, 255, 255, 255, 255], vec![0; 9]],
        )
        .unwrap();
        assert_eq!(levels[0], vec![0xef, 1, 0, 0]);
        assert_eq!(levels[1], vec![0b1110, 0b1111, 0, 0]);
        assert_eq!(levels[2], vec![0, 1, 0, 0]);
    }
    #[test]
    fn malformed_padding_and_all_without_any_fail_even_with_matching_hash() {
        let p = Profile {
            width: 3,
            height: 3,
            tile_edge: 256,
            decomposition_levels: 2,
            bits_per_sample: 8,
            components: 1,
            bits_per_pixel: 2.0,
        };
        let mut v = ValidityIdentity {
            source_sha256: "a".repeat(64),
            bands: vec![1],
            policy: POLICY.into(),
            tile_sha256: vec![vec![String::new()]; 3],
        };
        for (d, bytes) in [(0, vec![255, 255]), (1, vec![1, 0])] {
            v.tile_sha256[d][0] = sha256(&bytes);
            assert!(v.check(&p, 0, d as u8, &bytes).is_err());
        }
    }
}
