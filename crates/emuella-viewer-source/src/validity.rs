//! Codec-neutral source-grid validity sidecars. No reconstructed sample tests.
use crate::{Profile, Region, sha256};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

pub const POLICY: &str = "source-validity-v1";
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ValidityIdentity {
    pub source_sha256: String,
    pub bands: Vec<u16>,
    pub policy: String,
    /// Level-major, then row-major source tile.
    pub tile_sha256: Vec<Vec<String>>,
}
impl ValidityIdentity {
    pub fn validate(&self, p: &Profile) -> Result<()> {
        ensure!(self.policy == POLICY, "unsupported validity policy");
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
                        && v.iter()
                            .all(|s| s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit()))),
            "invalid mask digest catalogue"
        );
        ensure!(
            p.tile_edge.is_multiple_of(1 << p.decomposition_levels),
            "unaligned mask tiles"
        );
        Ok(())
    }
    pub fn check(&self, p: &Profile, tile: u16, discard: u8, bytes: &[u8]) -> Result<()> {
        let (w, h) = tile_size(p, tile, discard)?;
        let pixels = (w * h) as usize;
        let plane = pixels.div_ceil(8);
        let planes = if discard == 0 { 1 } else { 2 };
        ensure!(
            bytes.len() == plane * usize::from(p.components) * planes,
            "mask length mismatch"
        );
        ensure!(
            self.tile_sha256
                .get(usize::from(discard))
                .and_then(|v| v.get(usize::from(tile)))
                == Some(&sha256(bytes)),
            "mask digest mismatch"
        );
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
        let mut bytes = vec![0; plane * planes * bands.len()];
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
                    set(&mut bytes[c * plane * planes..], i, all);
                    if d > 0 {
                        set(&mut bytes[c * plane * planes + plane..], i, any);
                    }
                }
            }
        }
        output.push(bytes);
    }
    Ok(output)
}
/// Display-only combination. Quality callers retain each band's all/any plane.
pub fn combine_region<'a>(
    p: &Profile,
    region: &Region,
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
        let bytes = get(tile, region.discard)?;
        let (tw, th) = tile_size(p, tile, region.discard)?;
        let tx = u32::from(tile) % cols * edge;
        let ty = u32::from(tile) / cols * edge;
        let plane = (tw * th).div_ceil(8) as usize;
        let stride = plane * if region.discard == 0 { 1 } else { 2 };
        ensure!(
            bytes.len() == stride * usize::from(p.components),
            "mask length mismatch"
        );
        for y in y0.max(ty)..(y0 + h).min(ty + th) {
            for x in x0.max(tx)..(x0 + w).min(tx + tw) {
                let i = ((y - ty) * tw + x - tx) as usize;
                output[((y - y0) * w + x - x0) as usize] = u8::from(
                    region
                        .components
                        .iter()
                        .all(|&c| bit(&bytes[usize::from(c) * stride..], i)),
                );
            }
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
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
