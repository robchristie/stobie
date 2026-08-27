use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    AnnotationId, Camera, CameraState, ImagePoint, LayerId, LinkGroupId, PaneId, ResultId,
    SourceId, WorldPoint,
};

pub const TILE_SIZE: u32 = 256;
pub const PYRAMID_LEVELS: u8 = 10;
pub const RESULT_COUNT: u64 = 1_000_000;
pub const THUMBNAIL_COUNT: u64 = 100_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TileKey {
    pub source: SourceId,
    pub level: u8,
    pub x: u32,
    pub y: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DemandPriority {
    Prefetch,
    Visible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileDemand {
    pub key: TileKey,
    pub priority: DemandPriority,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ResourceKey {
    Tile(TileKey),
    Thumbnail(ResultId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceState {
    Missing,
    Queued,
    Decoding,
    Decoded,
    Resident,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Polygon {
    pub id: AnnotationId,
    pub layer: LayerId,
    pub vertices: Vec<WorldPoint>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub annotations: Vec<Polygon>,
    pub next_annotation_id: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActiveTool {
    Navigate,
    Polygon,
    EditVertex,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum GesturePreview {
    Polygon {
        layer: LayerId,
        vertices: Vec<WorldPoint>,
    },
    Vertex {
        annotation: AnnotationId,
        vertex: usize,
        original: WorldPoint,
        preview: WorldPoint,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub selected_result: Option<ResultId>,
    pub selected_annotation: Option<AnnotationId>,
    pub cameras: Vec<CameraState>,
    pub active_tools: BTreeMap<PaneId, ActiveTool>,
    #[serde(skip)]
    pub gesture: Option<GesturePreview>,
}

impl Default for Session {
    fn default() -> Self {
        let linked = Some(LinkGroupId(1));
        Self {
            selected_result: None,
            selected_annotation: None,
            cameras: (1..=4)
                .map(|id| CameraState {
                    pane: PaneId(id),
                    camera: Camera::default(),
                    link: if id <= 2 { linked } else { None },
                })
                .collect(),
            active_tools: (1..=4)
                .map(|id| (PaneId(id), ActiveTool::Navigate))
                .collect(),
            gesture: None,
        }
    }
}

impl Session {
    /// Restored application sessions must contain one valid camera for every image pane.
    pub fn validate_image_cameras(&self) -> Result<(), String> {
        let expected: BTreeSet<_> = (1..=4).map(PaneId).collect();
        let actual: BTreeSet<_> = self.cameras.iter().map(|state| state.pane).collect();
        if actual != expected || actual.len() != self.cameras.len() {
            return Err("session camera mappings must contain image panes 1–4 exactly once".into());
        }
        if self.cameras.iter().any(|state| {
            !state.camera.centre.x.is_finite()
                || !state.camera.centre.y.is_finite()
                || !state.camera.pixels_per_screen_point.is_finite()
                || state.camera.pixels_per_screen_point <= 0.0
        }) {
            return Err("session contains an invalid camera".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResultRecord {
    pub id: ResultId,
    pub position: ImagePoint,
    pub confidence: f32,
    pub category: u8,
}

pub fn result_at(index: u64) -> ResultRecord {
    let mixed = index.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(17);
    ResultRecord {
        id: ResultId(index),
        position: ImagePoint::new((mixed & 0x1_FFFF) as f64, ((mixed >> 21) & 0x1_FFFF) as f64),
        confidence: ((mixed >> 42) & 0xffff) as f32 / 65535.0,
        category: ((mixed >> 58) & 0x3) as u8,
    }
}

pub fn reconcile_demands(
    demands: impl IntoIterator<Item = TileDemand>,
) -> (Vec<TileDemand>, usize) {
    let mut merged = BTreeMap::<TileKey, TileDemand>::new();
    let mut duplicates = 0;
    for demand in demands {
        merged
            .entry(demand.key)
            .and_modify(|existing| {
                duplicates += 1;
                if demand.priority > existing.priority {
                    existing.priority = demand.priority;
                }
                existing.generation = existing.generation.max(demand.generation);
            })
            .or_insert(demand);
    }
    let mut output: Vec<_> = merged.into_values().collect();
    // Within one urgency class, request the coarsest pyramid coverage first so
    // a useful image can arrive before the finer visible set is complete.
    output.sort_by_key(|demand| std::cmp::Reverse((demand.priority, demand.key.level)));
    (output, duplicates)
}

pub fn visible_tile_demands(
    camera: Camera,
    viewport_points: (f64, f64),
    source: SourceId,
    generation: u64,
    visible: bool,
) -> Vec<TileDemand> {
    if !visible {
        return Vec::new();
    }
    let level = camera
        .pixels_per_screen_point
        .log2()
        .round()
        .clamp(0.0, (PYRAMID_LEVELS - 1) as f64) as u8;
    let world_tile = (TILE_SIZE as f64) * 2f64.powi(level as i32);
    let half_w = viewport_points.0 * camera.pixels_per_screen_point * 0.5;
    let half_h = viewport_points.1 * camera.pixels_per_screen_point * 0.5;
    let min_x = ((camera.centre.x - half_w) / world_tile).floor() as i64;
    let max_x = ((camera.centre.x + half_w) / world_tile).ceil() as i64;
    let min_y = ((camera.centre.y - half_h) / world_tile).floor() as i64;
    let max_y = ((camera.centre.y + half_h) / world_tile).ceil() as i64;
    let tiles_at_level = (131_072_u32 >> level).div_ceil(TILE_SIZE);
    let mut output = Vec::new();
    for y in (min_y - 1)..=(max_y + 1) {
        for x in (min_x - 1)..=(max_x + 1) {
            if x < 0 || y < 0 || x >= tiles_at_level as i64 || y >= tiles_at_level as i64 {
                continue;
            }
            let visible_x = x >= min_x && x <= max_x;
            let visible_y = y >= min_y && y <= max_y;
            output.push(TileDemand {
                key: TileKey {
                    source,
                    level,
                    x: x as u32,
                    y: y as u32,
                },
                priority: if visible_x && visible_y {
                    DemandPriority::Visible
                } else {
                    DemandPriority::Prefetch
                },
                generation,
            });
        }
    }
    output
}

pub fn unique_active_decode_keys(
    states: &BTreeMap<ResourceKey, ResourceState>,
) -> BTreeSet<TileKey> {
    states
        .iter()
        .filter_map(|(key, state)| match (key, state) {
            (ResourceKey::Tile(tile), ResourceState::Queued | ResourceState::Decoding) => {
                Some(*tile)
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_collections_are_generated_on_demand() {
        assert_eq!(RESULT_COUNT, 1_000_000);
        assert_eq!(THUMBNAIL_COUNT, 100_000);
        assert_eq!(result_at(7), result_at(7));
    }

    #[test]
    fn restored_camera_mapping_must_be_complete_and_unambiguous() {
        let mut session = Session::default();
        assert!(session.validate_image_cameras().is_ok());
        session.cameras.pop();
        assert!(session.validate_image_cameras().is_err());
        session.cameras.push(session.cameras[0]);
        assert!(session.validate_image_cameras().is_err());
    }

    #[test]
    fn demand_reconciliation_deduplicates_and_promotes_priority() {
        let key = TileKey {
            source: SourceId(1),
            level: 4,
            x: 2,
            y: 3,
        };
        let (merged, duplicate_count) = reconcile_demands([
            TileDemand {
                key,
                priority: DemandPriority::Prefetch,
                generation: 1,
            },
            TileDemand {
                key,
                priority: DemandPriority::Visible,
                generation: 1,
            },
        ]);
        assert_eq!(merged.len(), 1);
        assert_eq!(duplicate_count, 1);
        assert_eq!(merged[0].priority, DemandPriority::Visible);
    }

    #[test]
    fn visible_demands_schedule_coarse_coverage_before_fine_tiles() {
        let source = SourceId(1);
        let tile = |level| TileDemand {
            key: TileKey {
                source,
                level,
                x: 0,
                y: 0,
            },
            priority: DemandPriority::Visible,
            generation: 1,
        };
        let (ordered, _) = reconcile_demands([tile(4), tile(PYRAMID_LEVELS - 1), tile(6)]);
        assert_eq!(ordered[0].key.level, PYRAMID_LEVELS - 1);
        assert_eq!(ordered[1].key.level, 6);
        assert_eq!(ordered[2].key.level, 4);
    }

    #[test]
    fn hidden_view_has_no_high_resolution_demand() {
        assert!(
            visible_tile_demands(Camera::default(), (800.0, 600.0), SourceId(1), 1, false)
                .is_empty()
        );
    }

    #[test]
    fn demand_has_visible_and_prefetch_priorities() {
        let camera = Camera {
            pixels_per_screen_point: 8.0,
            ..Camera::default()
        };
        let demands = visible_tile_demands(camera, (800.0, 600.0), SourceId(1), 1, true);
        assert!(
            demands
                .iter()
                .any(|demand| demand.priority == DemandPriority::Visible)
        );
        assert!(
            demands
                .iter()
                .any(|demand| demand.priority == DemandPriority::Prefetch)
        );
    }
}
