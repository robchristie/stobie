use eframe::egui;
use polyorama_core::{
    DemandPriority, ImageIntent, PaneId, ResultId, SourceId, THUMBNAIL_COUNT, TileDemand, TileKey,
    VirtualisationMetrics,
};
use polyorama_ui_egui::{
    DesignTokens, DomainReference, SemanticUiId, TextOverflow, TextRole, ThumbnailCellSpec,
    ThumbnailState, UiNode, UiRole, VirtualGridPresenter, measured_content_label, thumbnail_cell,
    thumbnail_cell_side,
};

use crate::thumbnail_cache::ThumbnailCache;

use super::FrameOutput;

#[derive(Clone, Copy)]
pub struct ThumbnailPaneView<'a> {
    pub selected_result: Option<ResultId>,
    pub generation: u64,
    pub tokens: &'a DesignTokens,
    pub font_scale: f32,
}

pub fn show(
    ui: &mut egui::Ui,
    view: ThumbnailPaneView<'_>,
    cache: &mut ThumbnailCache,
    virtualisation: &mut VirtualisationMetrics,
    outputs: &mut FrameOutput,
) {
    let ThumbnailPaneView {
        selected_result,
        generation,
        tokens,
        font_scale,
    } = view;
    measured_content_label(
        ui,
        6_000,
        &format!(
            "{} logical thumbnails · progressive worker decode",
            THUMBNAIL_COUNT
        ),
        TextRole::Secondary,
        TextOverflow::Ellipsis,
        1,
        tokens,
        font_scale,
        &mut outputs.ui_geometry.text_layouts,
    );
    let side = thumbnail_cell_side(tokens, font_scale);
    let gap = tokens.spacing.inline.0;
    let cell = egui::vec2(side + gap, side + gap);
    let output = VirtualGridPresenter::new(cell, 2).show(
        ui,
        ui.id().with("thumbnail-grid"),
        THUMBNAIL_COUNT as usize,
        |grid_ui, layout, origin| {
            for index in layout.materialised_items.clone() {
                let row = index / layout.columns;
                let column = index % layout.columns;
                let rect = egui::Rect::from_min_size(
                    origin + egui::vec2(column as f32 * cell.x, row as f32 * cell.y),
                    egui::vec2(side, side),
                );
                let key = TileKey {
                    source: SourceId(2),
                    level: 0,
                    x: index as u32,
                    y: 0,
                };
                outputs.demands.push(TileDemand {
                    key,
                    priority: if layout.visible_items.contains(&index) {
                        DemandPriority::Visible
                    } else {
                        DemandPriority::Prefetch
                    },
                    generation,
                });
                let texture = cache.texture(key);
                let selected = selected_result == Some(ResultId(index as u64));
                let state = if texture.is_some() {
                    ThumbnailState::Resident
                } else {
                    ThumbnailState::Loading
                };
                let label = format!("Result #{index}");
                let response = grid_ui
                    .scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                        ui.set_width(side);
                        thumbnail_cell(
                            ui,
                            ThumbnailCellSpec {
                                instance: index as u64,
                                label: &label,
                                state,
                                selected,
                                texture,
                            },
                            tokens,
                            font_scale,
                            &mut outputs.ui_geometry.text_layouts,
                        )
                    })
                    .inner;
                if response.clicked() {
                    outputs.intents.push(ImageIntent::SelectResult {
                        result: ResultId(index as u64),
                    });
                }
                let inside_root = outputs
                    .ui_geometry
                    .root
                    .is_some_and(|root| root.contains(response.rect.into(), 1.0));
                if response.rect.intersects(grid_ui.clip_rect()) && inside_root {
                    outputs.ui_geometry.record_node(UiNode {
                        id: SemanticUiId::new(format!("polyorama.thumbnail-cell.{index}")),
                        parent: Some(SemanticUiId::pane(PaneId(6))),
                        role: UiRole::ThumbnailCell,
                        name: format!("{label}; {state:?}"),
                        description: None,
                        rect: response.rect.into(),
                        enabled: true,
                        focused: response.has_focus(),
                        selected,
                        checked: None,
                        expanded: None,
                        pane: Some(PaneId(6)),
                        domain_reference: Some(DomainReference::Thumbnail(key)),
                        actions: Vec::new(),
                        disabled_reason: None,
                    });
                }
            }
        },
    );
    virtualisation.visible_thumbnails = (
        output.layout.visible_items.start,
        output.layout.visible_items.end,
    );
    virtualisation.materialised_thumbnails = output.layout.materialised_items.len();
    virtualisation.materialised_thumbnail_range = (
        output.layout.materialised_items.start,
        output.layout.materialised_items.end,
    );
    virtualisation.thumbnail_columns = output.layout.columns;
    virtualisation.thumbnail_total_rows = output.layout.total_rows;
    virtualisation.thumbnail_scroll_offset_y = output.scroll_offset_y;
    virtualisation.thumbnail_content_height = output.content_height;
    virtualisation.thumbnail_viewport_height = output.viewport_height;
    outputs.ui_geometry.thumbnail_scroll = Some(output.viewport_rect.into());
    let mut scroll = UiNode::container(
        SemanticUiId::new("pane.6.thumbnails.scroll"),
        Some(SemanticUiId::pane(PaneId(6))),
        UiRole::ScrollArea,
        output.viewport_rect.into(),
    );
    scroll.name = "Thumbnails".into();
    scroll.pane = Some(PaneId(6));
    outputs.ui_geometry.record_node(scroll);
    if output.wheel_delta_y != 0.0 {
        virtualisation.thumbnail_wheel_input_frames += 1;
        virtualisation.thumbnail_wheel_delta_y += output.wheel_delta_y;
    }
}
