use egui::{Id, IdSalt, Pos2, Rect, ScrollArea, Ui, UiBuilder, Vec2, scroll_area::State};
use polyorama_core::{VirtualGrid, layout_virtual_grid};

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

pub type VirtualGridLayout = VirtualGrid;

#[derive(Debug)]
pub struct VirtualGridOutput<R> {
    pub inner: R,
    pub layout: VirtualGridLayout,
    pub viewport_rect: Rect,
    pub scroll_offset_y: f32,
    pub content_height: f32,
    pub viewport_height: f32,
    /// Effective vertical wheel delta consumed by this grid this frame.
    ///
    /// This is zero when the grid could not move. When egui is configured to
    /// route both wheel axes into the only enabled scroll direction, it is the
    /// sum of those axes.
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
        let current_offset = State::load(ui.ctx(), scroll_id).map_or(0.0, |state| state.offset.y);
        let content_height = (total_rows as f32 * self.cell_pitch.y - item_spacing_y).max(0.0);
        let max_offset = (content_height - available_rect.height()).max(0.0);
        let combine_wheel_axes = ui.style().always_scroll_the_only_direction;
        let wheel_delta_y = ui.input(|input| {
            input
                .pointer
                .hover_pos()
                .filter(|pointer| available_rect.contains(*pointer))
                .map_or(0.0, |_| {
                    if combine_wheel_axes {
                        input.smooth_scroll_delta.x + input.smooth_scroll_delta.y
                    } else {
                        input.smooth_scroll_delta.y
                    }
                })
        });
        let can_move_up = current_offset > 0.0 && wheel_delta_y > 0.0;
        let can_move_down = current_offset < max_offset && wheel_delta_y < 0.0;
        let requested_offset = (can_move_up || can_move_down)
            .then_some((current_offset - wheel_delta_y).clamp(0.0, max_offset));
        if requested_offset.is_some() {
            ui.input_mut(|input| {
                if combine_wheel_axes {
                    input.smooth_scroll_delta = Vec2::ZERO;
                } else {
                    input.smooth_scroll_delta.y = 0.0;
                }
            });
        }
        let mut captured_layout = None;
        let mut scroll_area = ScrollArea::vertical().id_salt(id);
        if let Some(offset) = requested_offset {
            scroll_area = scroll_area.vertical_scroll_offset(offset);
        }
        let output = scroll_area.show_rows(
            ui,
            row_height_without_spacing,
            total_rows,
            |ui, visible_rows| {
                let layout =
                    layout_virtual_grid(total_items, columns, visible_rows, self.overscan_rows);
                let content_origin = Pos2::new(
                    ui.max_rect().left(),
                    ui.max_rect().top() - layout.visible_rows.start as f32 * self.cell_pitch.y,
                );
                let materialised_rect = Rect::from_min_max(
                    Pos2::new(
                        content_origin.x,
                        content_origin.y
                            + layout.materialised_rows.start as f32 * self.cell_pitch.y,
                    ),
                    Pos2::new(
                        ui.max_rect().right(),
                        content_origin.y + layout.materialised_rows.end as f32 * self.cell_pitch.y,
                    ),
                );
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
            viewport_rect: output.inner_rect,
            scroll_offset_y: output.state.offset.y,
            content_height: output.content_size.y,
            viewport_height: output.inner_rect.height(),
            wheel_delta_y: requested_offset.map_or(0.0, |_| wheel_delta_y),
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
        fn run_frame(
            ctx: &egui::Context,
            id: &'static str,
            total_items: usize,
            delta: Option<Vec2>,
        ) -> (f32, Vec2, f32) {
            let captured = Rc::new(RefCell::new(None));
            let output = captured.clone();
            let mut events = vec![egui::Event::PointerMoved(Pos2::new(100.0, 100.0))];
            if let Some(delta) = delta {
                events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta,
                    phase: egui::TouchPhase::Move,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 240.0))),
                events,
                ..Default::default()
            };
            let mut full_output = ctx.run_ui(input, |ui| {
                let grid = VirtualGridPresenter::new(Vec2::new(106.0, 96.0), 2).show(
                    ui,
                    Id::new(id),
                    total_items,
                    |_ui, _layout, _origin| {},
                );
                *output.borrow_mut() = Some((
                    grid.scroll_offset_y,
                    ui.input(|input| input.smooth_scroll_delta),
                    grid.wheel_delta_y,
                ));
            });
            full_output.textures_delta.clear();
            captured.borrow_mut().take().unwrap()
        }

        let ctx = egui::Context::default();
        assert_eq!(run_frame(&ctx, "wheel-grid", 100_000, None).0, 0.0);
        let (offset, remaining, consumed) =
            run_frame(&ctx, "wheel-grid", 100_000, Some(Vec2::new(0.0, -4.0)));
        assert!(offset > 0.0);
        assert_eq!(remaining, Vec2::ZERO);
        assert_eq!(consumed, -4.0);
        let (retained, _, _) = run_frame(&ctx, "wheel-grid", 100_000, None);
        assert!(retained >= offset);

        let (short_offset, short_remaining, short_consumed) =
            run_frame(&ctx, "short-grid", 1, Some(Vec2::new(0.0, -4.0)));
        assert_eq!(short_offset, 0.0);
        assert_eq!(short_remaining, Vec2::new(0.0, -4.0));
        assert_eq!(short_consumed, 0.0);

        let (top_offset, top_remaining, top_consumed) =
            run_frame(&ctx, "top-grid", 100_000, Some(Vec2::new(0.0, 4.0)));
        assert_eq!(top_offset, 0.0);
        assert_eq!(top_remaining, Vec2::new(0.0, 4.0));
        assert_eq!(top_consumed, 0.0);

        let horizontal_ctx = egui::Context::default();
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            let mut style = (*horizontal_ctx.style_of(theme)).clone();
            style.always_scroll_the_only_direction = true;
            horizontal_ctx.set_style_of(theme, style);
        }
        let (horizontal_offset, horizontal_remaining, horizontal_consumed) = run_frame(
            &horizontal_ctx,
            "horizontal-grid",
            100_000,
            Some(Vec2::new(-4.0, 0.0)),
        );
        assert!(horizontal_offset > 0.0);
        assert_eq!(horizontal_remaining, Vec2::ZERO);
        assert_eq!(horizontal_consumed, -4.0);

        let bottom_ctx = egui::Context::default();
        let mut bottom_offset = 0.0;
        for _ in 0..20 {
            bottom_offset = run_frame(&bottom_ctx, "bottom-grid", 9, Some(Vec2::new(0.0, -4.0))).0;
        }
        assert!(bottom_offset > 0.0);
        let (stuck_offset, bottom_remaining, bottom_consumed) =
            run_frame(&bottom_ctx, "bottom-grid", 9, Some(Vec2::new(0.0, -4.0)));
        assert_eq!(stuck_offset, bottom_offset);
        assert_eq!(bottom_remaining, Vec2::new(0.0, -4.0));
        assert_eq!(bottom_consumed, 0.0);
    }
}
