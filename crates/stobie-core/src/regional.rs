//! Source-independent regional requests. Coordinates are full-resolution image pixels.

use serde::{Deserialize, Serialize};

use crate::{DemandPriority, VirtualGrid};

/// Immutable source representation identity, supplied by the source adapter.
///
/// Its digest must cover source content, preparation policy/version and all encoding
/// choices that affect reconstruction. Display stretch and colour mapping are excluded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RepresentationId(pub [u8; 32]);

/// Quality/precision selection interpreted exclusively by the source adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SourceStage(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RegionConsumerId(pub u64);

/// A non-empty half-open rectangle, relative to the parent image (never a chip identity).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ImageRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl ImageRegion {
    pub fn is_valid(self) -> bool {
        self.width > 0
            && self.height > 0
            && self.x.checked_add(self.width).is_some()
            && self.y.checked_add(self.height).is_some()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RegionKey {
    pub representation: RepresentationId,
    pub region: ImageRegion,
    /// Number of powers of two by which the source is reduced.
    pub reduction: u8,
    /// Ordered source components; order is part of decoded identity.
    pub components: Vec<u16>,
    pub stage: SourceStage,
}

impl RegionKey {
    pub fn is_valid(&self) -> bool {
        self.region.is_valid()
            && self.reduction < 32
            && matches!(self.components.len(), 1 | 3)
            && self
                .components
                .iter()
                .enumerate()
                .all(|(index, value)| !self.components[..index].contains(value))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionDemand {
    pub consumer: RegionConsumerId,
    pub key: RegionKey,
    pub priority: DemandPriority,
    /// Upper bound on decoded sample bytes, including edge/alignment expansion.
    /// The worker must enforce this bound before allocating its output.
    pub max_decoded_bytes: usize,
}

/// Construct only the virtual grid's visible and bounded overscan requests.
/// `region_at` is called only for materialised items, even for a million detections.
/// Exact overlaps can share work; nearby or distant rectangles remain independent.
pub fn regional_grid_demands(
    grid: &VirtualGrid,
    mut region_at: impl FnMut(usize, DemandPriority) -> RegionDemand,
) -> Vec<RegionDemand> {
    grid.materialised_items
        .clone()
        .map(|index| {
            region_at(
                index,
                if grid.visible_items.contains(&index) {
                    DemandPriority::Visible
                } else {
                    DemandPriority::Prefetch
                },
            )
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SampleLayout {
    Scalar,
    Rgb,
}

impl SampleLayout {
    pub fn channels(self) -> usize {
        match self {
            Self::Scalar => 1,
            Self::Rgb => 3,
        }
    }
}

/// Native unsigned samples, tightly interleaved in component order. No display mapping.
/// Adapters perform any source-specific colour transform before constructing RGB.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionalPixels {
    pub width: u32,
    pub height: u32,
    pub layout: SampleLayout,
    pub precision: u8,
    pub samples: Vec<u16>,
}

impl RegionalPixels {
    pub fn byte_len(&self) -> usize {
        self.samples.len().saturating_mul(2)
    }

    /// Backing sample allocation, including spare capacity retained by an adapter.
    pub fn allocation_bytes(&self) -> usize {
        self.samples.capacity().saturating_mul(2)
    }

    pub fn is_valid(&self) -> bool {
        let expected = (self.width as usize)
            .checked_mul(self.height as usize)
            .and_then(|pixels| pixels.checked_mul(self.layout.channels()));
        self.width > 0
            && self.height > 0
            && (1..=16).contains(&self.precision)
            && expected == Some(self.samples.len())
            && self
                .samples
                .iter()
                .all(|&sample| u32::from(sample) < (1_u32 << self.precision))
    }
}

/// Additive binary-validity envelope. Existing `RegionalPixels` literals and
/// unmasked runtime/renderer entry points remain source compatible.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionalFrame {
    #[serde(flatten)]
    pub pixels: RegionalPixels,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validity: Option<Vec<u8>>,
}
impl From<RegionalPixels> for RegionalFrame {
    fn from(pixels: RegionalPixels) -> Self {
        Self {
            pixels,
            validity: None,
        }
    }
}
impl std::ops::Deref for RegionalFrame {
    type Target = RegionalPixels;
    fn deref(&self) -> &Self::Target {
        &self.pixels
    }
}
impl RegionalFrame {
    pub fn byte_len(&self) -> usize {
        self.pixels
            .byte_len()
            .saturating_add(self.validity.as_ref().map_or(0, Vec::len))
    }
    pub fn allocation_bytes(&self) -> usize {
        self.pixels
            .allocation_bytes()
            .saturating_add(self.validity.as_ref().map_or(0, Vec::capacity))
    }
    pub fn is_valid(&self) -> bool {
        self.pixels.is_valid()
            && self.validity.as_ref().is_none_or(|v| {
                v.len() == self.width as usize * self.height as usize && v.iter().all(|&b| b <= 1)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_virtual_grid;

    #[test]
    fn scattered_gallery_constructs_only_visible_and_overscan_parent_regions() {
        let grid = layout_virtual_grid(100_000, 5, 100..106, 2);
        let mut calls = 0;
        let demands = regional_grid_demands(&grid, |index, priority| {
            calls += 1;
            RegionDemand {
                consumer: RegionConsumerId(index as u64),
                key: RegionKey {
                    representation: RepresentationId([7; 32]),
                    region: ImageRegion {
                        x: (index as u32 * 997) % 100_000,
                        y: (index as u32 * 313) % 100_000,
                        width: 128,
                        height: 128,
                    },
                    reduction: 1,
                    components: vec![0],
                    stage: SourceStage(0),
                },
                priority,
                max_decoded_bytes: 8192,
            }
        });
        assert_eq!(calls, 50);
        assert_eq!(demands.len(), 50);
        assert_eq!(
            demands
                .iter()
                .filter(|d| d.priority == DemandPriority::Visible)
                .count(),
            30
        );
        assert!(demands.iter().all(|d| d.key.region.width == 128));
    }

    #[test]
    fn invalid_geometry_and_sample_precision_are_rejected() {
        assert!(
            !ImageRegion {
                x: u32::MAX,
                y: 0,
                width: 1,
                height: 1
            }
            .is_valid()
        );
        let mut pixels = RegionalPixels {
            width: 1,
            height: 1,
            layout: SampleLayout::Scalar,
            precision: 11,
            samples: vec![2047],
        };
        assert!(pixels.is_valid());
        pixels.samples[0] = 2048;
        assert!(!pixels.is_valid());
    }
}
