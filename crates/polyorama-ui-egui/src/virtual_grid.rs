use std::ops::Range;

use egui::{Id, IdSalt, Pos2, Rect, ScrollArea, Ui, UiBuilder, Vec2, scroll_area::State};

/// Egui presentation for a large logical two-dimensional collection.
///
/// The presenter owns the exact scroll extent and positions only the visible
/// rows plus caller-selected overscan. Domain panes retain responsibility for
/// item IDs, painting, selection and demand generation.
#[derive(Clone, Copy, Debug)]
pub struct VirtualGridPresenter {
    cell_pitch: Vec2,
    overscan_rows: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VirtualGridLayout {
    pub columns: usize,
    pub total_rows: usize,
    pub visible_items: Range<usize>,
    pub materialised_items: Range<usize>,
}

#[derive(Debug)]
pub struct VirtualGridOutput<R> {
    pub inner: R,
    pub layout: VirtualGridLayout,
    pub scroll_offset_y: f32,
    pub content_height: f32,
    pub viewport_height: f32,
    pub wheel_delta_y: f32,
}

impl VirtualGridPresenter {
    pub fn new(cell_pitch: Vec2, overscan_rows: usize) -> Self {
        Self {
            cell_pitch: Vec2::new(cell_pitch.x.max(1.0), cell_pitch.y.max(1.0)),
            overscan_rows,
        }
    }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        id: Id,
        total_items: usize,
        add_contents: impl FnOnce(&mut Ui, &VirtualGridLayout, Pos2) -> R,
    ) -> VirtualGridOutput<R> {
        let columns = (ui.available_width() / self.cell_pitch.x).floor().max(1.0) as usize;
        let total_rows = total_items.div_ceil(columns);
        let item_spacing_y = ui.spacing().item_spacing.y;
        let row_height_without_spacing = (self.cell_pitch.y - item_spacing_y).max(1.0);
        let available_rect = ui.available_rect_before_wrap();
        let scroll_id = ui.make_persistent_id(IdSalt::new(id));
        let wheel_delta_y = ui.input(|input| {
            input
                .pointer
                .hover_pos()
                .filter(|pointer| available_rect.contains(*pointer))
                .map_or(0.0, |_| input.smooth_scroll_delta.y)
        });
        let requested_offset =
            State::load(ui.ctx(), scroll_id).map_or(0.0, |state| state.offset.y) - wheel_delta_y;
        if wheel_delta_y != 0.0 {
            ui.input_mut(|input| input.smooth_scroll_delta.y = 0.0);
        }
        let mut captured_layout = None;
        let output = ScrollArea::vertical()
            .id_salt(id)
            .vertical_scroll_offset(requested_offset.max(0.0))
            .show_rows(
                ui,
                row_height_without_spacing,
                total_rows,
                |ui, visible_rows| {
                    let materialised_rows = visible_rows.start.saturating_sub(self.overscan_rows)
                        ..visible_rows
                            .end
                            .saturating_add(self.overscan_rows)
                            .min(total_rows);
                    let visible_items = (visible_rows.start * columns).min(total_items)
                        ..(visible_rows.end * columns).min(total_items);
                    let materialised_items = (materialised_rows.start * columns).min(total_items)
                        ..(materialised_rows.end * columns).min(total_items);
                    let content_origin = Pos2::new(
                        ui.max_rect().left(),
                        ui.max_rect().top() - visible_rows.start as f32 * self.cell_pitch.y,
                    );
                    let materialised_rect = Rect::from_min_max(
                        Pos2::new(
                            content_origin.x,
                            content_origin.y + materialised_rows.start as f32 * self.cell_pitch.y,
                        ),
                        Pos2::new(
                            ui.max_rect().right(),
                            content_origin.y + materialised_rows.end as f32 * self.cell_pitch.y,
                        ),
                    );
                    let layout = VirtualGridLayout {
                        columns,
                        total_rows,
                        visible_items,
                        materialised_items,
                    };
                    captured_layout = Some(layout.clone());
                    ui.scope_builder(UiBuilder::new().max_rect(materialised_rect), |grid_ui| {
                        grid_ui.skip_ahead_auto_ids(layout.materialised_items.start);
                        add_contents(grid_ui, &layout, content_origin)
                    })
                    .inner
                },
            );
        VirtualGridOutput {
            inner: output.inner,
            layout: captured_layout.expect("virtual grid closure always runs"),
            scroll_offset_y: output.state.offset.y,
            content_height: output.content_size.y,
            viewport_height: output.inner_rect.height(),
            wheel_delta_y,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;

    #[test]
    fn presenter_establishes_exact_scroll_extent_and_bounded_materialisation() {
        egui::__run_test_ui(|ui| {
            let spacing_y = ui.spacing().item_spacing.y;
            let output = ui
                .scope_builder(
                    UiBuilder::new().max_rect(Rect::from_min_size(
                        ui.min_rect().min,
                        Vec2::new(320.0, 240.0),
                    )),
                    |ui| {
                        VirtualGridPresenter::new(Vec2::new(106.0, 96.0), 2).show(
                            ui,
                            Id::new("test-grid"),
                            100_000,
                            |_ui, layout, _origin| layout.clone(),
                        )
                    },
                )
                .inner;

            assert_eq!(output.layout.columns, 3);
            assert_eq!(output.layout.total_rows, 33_334);
            assert!((output.content_height - (33_334.0 * 96.0 - spacing_y)).abs() < 0.1);
            assert!(output.content_height > output.viewport_height);
            assert!(output.layout.materialised_items.len() < 40);
        });
    }

    #[test]
    fn presenter_routes_physical_wheel_input_into_retained_scroll_state() {
        fn run_frame(ctx: &egui::Context, events: Vec<egui::Event>) -> f32 {
            let captured = Rc::new(RefCell::new(None));
            let output = captured.clone();
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 240.0))),
                events,
                ..Default::default()
            };
            let mut full_output = ctx.run_ui(input, |ui| {
                let grid = VirtualGridPresenter::new(Vec2::new(106.0, 96.0), 2).show(
                    ui,
                    Id::new("wheel-grid"),
                    100_000,
                    |_ui, _layout, _origin| {},
                );
                *output.borrow_mut() = Some(grid.scroll_offset_y);
            });
            full_output.textures_delta.clear();
            captured.borrow_mut().take().unwrap()
        }

        let ctx = egui::Context::default();
        assert_eq!(
            run_frame(
                &ctx,
                vec![egui::Event::PointerMoved(Pos2::new(100.0, 100.0))]
            ),
            0.0
        );
        let offset = run_frame(
            &ctx,
            vec![
                egui::Event::PointerMoved(Pos2::new(100.0, 100.0)),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -1_500.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert!(offset > 0.0);
        let retained = run_frame(
            &ctx,
            vec![egui::Event::PointerMoved(Pos2::new(100.0, 100.0))],
        );
        assert!(retained >= offset);
    }
}
