//! Immediate-mode presentation and semantic pane interfaces.

mod actions;
mod virtual_grid;

mod components;
mod generated_tokens;
mod pane_content;
mod preferences;
mod preferences_control;
mod responsive;
mod semantics;
mod style;
#[cfg(test)]
mod test_actions;
mod text;
mod text_coverage;
mod typography;

pub use actions::*;
pub use components::*;
pub use generated_tokens::*;
pub use pane_content::*;
pub use preferences::*;
pub use preferences_control::*;
pub use responsive::*;
pub use semantics::*;
pub use style::*;
pub use text::*;
pub use text_coverage::*;
pub use typography::*;
pub use virtual_grid::*;

use egui::{Pos2, Rect, Ui};
use polyorama_core::{
    Command, DockDrop, DockNode, DockNodeId, LogicalPoint, PaneId, PhysicalPoint, SplitAxis,
    ViewportPoint, Workspace,
};
use polyorama_render_wgpu::{ImageRenderRequest, PixelRect, RenderPlan, ScalarRenderer};
use std::sync::{Arc, RwLock};

const TAB_VISUAL_HEIGHT: f32 = 24.0;
const SPLITTER_KEY_STEP: f32 = 0.05;

#[derive(Clone, Copy)]
pub struct DockTextContext {
    pub tokens: DesignTokens,
    pub font_scale: f32,
}

#[derive(Default)]
pub struct DockBehaviour {
    pub dragging: Option<PaneId>,
    pending: Option<DockAction>,
    split_preview: Option<SplitPreview>,
    interaction_active: bool,
}

impl DockBehaviour {
    pub fn interaction_active(&self) -> bool {
        self.interaction_active
    }

    fn finish_frame(&mut self, pointer_down: bool) {
        if !pointer_down {
            self.dragging = None;
            self.split_preview = None;
        }
        self.interaction_active = self.split_preview.is_some() || self.dragging.is_some();
    }
}

#[derive(Clone, Copy)]
struct SplitPreview {
    node: DockNodeId,
    before: f32,
    after: f32,
    pointer_origin: Pos2,
}

enum DockAction {
    Activate(PaneId),
    Move {
        pane: PaneId,
        target: PaneId,
        drop: DockDrop,
    },
    Resize {
        node: DockNodeId,
        before: f32,
        after: f32,
    },
}

pub trait PanePresenter {
    fn title(&self, pane: PaneId) -> &'static str;
    fn pane_ui(&mut self, ui: &mut Ui, pane: PaneId, pane_rect: Rect);
    fn record_tab_rect(&mut self, _pane: PaneId, _rect: Rect, _selected: bool, _focused: bool) {}
    fn record_text_layout(&mut self, _observation: TextLayoutObservation) {}
    fn record_splitter_rect(
        &mut self,
        _node: DockNodeId,
        _rect: Rect,
        _horizontal: bool,
        _focused: bool,
    ) {
    }
}

pub fn dock_workspace(
    ui: &mut Ui,
    workspace: &mut Workspace,
    behaviour: &mut DockBehaviour,
    presenter: &mut impl PanePresenter,
    text_context: DockTextContext,
) -> Option<Command> {
    behaviour.interaction_active = false;
    let rect = ui.available_rect_before_wrap();
    render_node(
        ui,
        &mut workspace.root,
        rect,
        behaviour,
        presenter,
        text_context,
    );
    behaviour.finish_frame(ui.input(|input| input.pointer.any_down()));
    if let Some(action) = behaviour.pending.take() {
        match action {
            DockAction::Activate(pane) => {
                workspace.activate(pane);
            }
            DockAction::Move { pane, target, drop } => {
                workspace.move_pane(pane, target, drop);
            }
            DockAction::Resize {
                node,
                before,
                after,
            } => {
                return Some(Command::ResizeSplit {
                    node,
                    before,
                    after,
                });
            }
        }
    }
    None
}

fn render_node(
    ui: &mut Ui,
    node: &mut DockNode,
    rect: Rect,
    behaviour: &mut DockBehaviour,
    presenter: &mut impl PanePresenter,
    text_context: DockTextContext,
) {
    match node {
        DockNode::Split {
            id,
            axis,
            fraction,
            first,
            second,
        } => {
            let node = *id;
            let horizontal = *axis == SplitAxis::Horizontal;
            let length = if horizontal {
                rect.width()
            } else {
                rect.height()
            };
            let initial_fraction = behaviour
                .split_preview
                .filter(|preview| preview.node == node)
                .map_or(*fraction, |preview| preview.after);
            let (_, visual_hit_rect, _) = split_rects(rect, horizontal, initial_fraction);
            let minimum_hit = text_context.tokens.geometry.minimum_hit_size.0;
            let hit_rect = minimum_hit_rect(visual_hit_rect, minimum_hit, rect);
            let response = ui.interact(
                hit_rect,
                egui::Id::new(("polyorama.dock.splitter", node.0)),
                egui::Sense::click_and_drag(),
            );
            response.widget_info(|| egui::WidgetInfo::new(egui::WidgetType::ResizeHandle));
            let splitter_author_id = node.0;
            ui.ctx().accesskit_node_builder(response.id, |node| {
                use egui::accesskit::{Action, Orientation, Role};
                node.set_role(Role::Splitter);
                node.set_label(if horizontal {
                    "Vertical splitter"
                } else {
                    "Horizontal splitter"
                });
                node.set_description("Resize adjacent dock panes");
                node.set_author_id(format!("polyorama.dock.splitter.{splitter_author_id}"));
                node.set_orientation(if horizontal {
                    Orientation::Vertical
                } else {
                    Orientation::Horizontal
                });
                node.set_numeric_value(f64::from(initial_fraction));
                node.set_min_numeric_value(0.1);
                node.set_max_numeric_value(0.9);
                node.set_numeric_value_step(f64::from(SPLITTER_KEY_STEP));
                node.add_action(Action::Increment);
                node.add_action(Action::Decrement);
            });
            if response.drag_started() {
                let pointer = response
                    .interact_pointer_pos()
                    .unwrap_or_else(|| hit_rect.center());
                behaviour.split_preview = Some(SplitPreview {
                    node,
                    before: *fraction,
                    after: *fraction,
                    pointer_origin: pointer - response.total_drag_delta().unwrap_or_default(),
                });
            }
            if response.dragged() || response.drag_stopped() {
                let pointer = response.interact_pointer_pos();
                if let Some(preview) = behaviour
                    .split_preview
                    .as_mut()
                    .filter(|preview| preview.node == node)
                    && let Some(pointer) = pointer
                {
                    let total_delta = pointer - preview.pointer_origin;
                    let axis_delta = if horizontal {
                        total_delta.x
                    } else {
                        total_delta.y
                    };
                    preview.after =
                        ((preview.before * length + axis_delta) / length).clamp(0.1, 0.9);
                }
            }
            let (accesskit_decrement, accesskit_increment) = ui.input(|input| {
                use egui::accesskit::Action;
                (
                    input.num_accesskit_action_requests(response.id, Action::Decrement),
                    input.num_accesskit_action_requests(response.id, Action::Increment),
                )
            });
            let (keyboard_decrement, keyboard_increment) = if response.has_focus() {
                ui.memory_mut(|memory| {
                    memory.set_focus_lock_filter(
                        response.id,
                        egui::EventFilter {
                            horizontal_arrows: horizontal,
                            vertical_arrows: !horizontal,
                            ..Default::default()
                        },
                    );
                });
                ui.input_mut(|input| {
                    let keys = if horizontal {
                        (egui::Key::ArrowLeft, egui::Key::ArrowRight)
                    } else {
                        (egui::Key::ArrowUp, egui::Key::ArrowDown)
                    };
                    (
                        usize::from(input.consume_key(egui::Modifiers::NONE, keys.0)),
                        usize::from(input.consume_key(egui::Modifiers::NONE, keys.1)),
                    )
                })
            } else {
                (0, 0)
            };
            if keyboard_decrement + keyboard_increment > 0 {
                // A splitter can gain focus after egui has already interpreted this
                // pass's arrow key as spatial focus navigation. Cancel that pending
                // move when the splitter consumes the key so end-of-pass processing
                // cannot move focus away again.
                ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
            }
            let after = adjusted_split_fraction(
                *fraction,
                (keyboard_increment + accesskit_increment) as i32
                    - (keyboard_decrement + accesskit_decrement) as i32,
            );
            if after != *fraction {
                behaviour.pending = Some(DockAction::Resize {
                    node,
                    before: *fraction,
                    after,
                });
            }
            let shown_fraction = behaviour
                .split_preview
                .filter(|preview| preview.node == node)
                .map_or(*fraction, |preview| preview.after);
            let (first_rect, split_rect, second_rect) =
                split_rects(rect, horizontal, shown_fraction);
            let current_hit_rect = minimum_hit_rect(split_rect, minimum_hit, rect);
            presenter.record_splitter_rect(
                node,
                current_hit_rect,
                horizontal,
                response.has_focus(),
            );
            if response.drag_stopped()
                && let Some(preview) = behaviour
                    .split_preview
                    .take()
                    .filter(|preview| preview.node == node)
                && preview.before != preview.after
            {
                behaviour.pending = Some(DockAction::Resize {
                    node,
                    before: preview.before,
                    after: preview.after,
                });
            }
            paint_splitter(
                ui.painter(),
                split_rect,
                SplitterVisualState {
                    hovered: response.hovered(),
                    active: response.is_pointer_button_down_on()
                        || behaviour
                            .split_preview
                            .is_some_and(|preview| preview.node == node),
                    focused: response.has_focus(),
                },
                &text_context.tokens,
            );
            render_node(ui, first, first_rect, behaviour, presenter, text_context);
            render_node(ui, second, second_rect, behaviour, presenter, text_context);
        }
        DockNode::Tabs { id, tabs, active } => {
            if tabs.is_empty() {
                ui.painter()
                    .rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
                return;
            }
            *active = (*active).min(tabs.len() - 1);
            let tab_visual_height = TAB_VISUAL_HEIGHT * text_context.font_scale;
            let tab_height = text_context
                .tokens
                .geometry
                .minimum_hit_size
                .0
                .max(tab_visual_height);
            let tab_rect = Rect::from_min_max(
                rect.min,
                Pos2::new(rect.right(), (rect.top() + tab_height).min(rect.bottom())),
            );
            let body = Rect::from_min_max(Pos2::new(rect.left(), tab_rect.bottom()), rect.max);
            ui.painter().rect_filled(rect, 0.0, ui.visuals().panel_fill);
            let tab_list_id = egui::Id::new(("polyorama.dock.tab-list", id.0));
            let mut tab_ui = ui.new_child(
                egui::UiBuilder::new()
                    .id(tab_list_id)
                    .max_rect(tab_rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            tab_ui.set_min_size(tab_rect.size());
            tab_ui.ctx().accesskit_node_builder(tab_list_id, |node| {
                use egui::accesskit::{Orientation, Role};
                node.set_role(Role::TabList);
                node.set_label("Workspace panes");
                node.set_author_id(format!("polyorama.dock.tab-list.{}", id.0));
                node.set_orientation(Orientation::Horizontal);
            });
            let strip_padding = text_context.tokens.spacing.unit.0;
            let gap = (text_context.tokens.spacing.unit.0 - 1.0).max(0.0);
            let tab_padding = text_context.tokens.geometry.control_padding_x.0;
            let width_class = PaneWidthClass::from_points(rect.width());
            let maximum_tab_width = match width_class {
                PaneWidthClass::Narrow => 132.0,
                PaneWidthClass::Regular => 180.0,
                PaneWidthClass::Wide => 220.0,
            };
            let intrinsic_spec = TextSpec {
                horizontal_alignment: HorizontalTextAlignment::Centre,
                ..TextSpec::single_line(TextRole::TabLabel, TextOverflow::Expand)
            };
            let desired_widths = tabs
                .iter()
                .map(|pane| {
                    measure_text(
                        tab_ui.painter(),
                        presenter.title(*pane),
                        intrinsic_spec,
                        &text_context.tokens,
                        text_context.font_scale,
                        1.0,
                    )
                    .map(|text| (text.size().x + tab_padding * 2.0).min(maximum_tab_width))
                    .unwrap_or(maximum_tab_width)
                })
                .collect::<Vec<_>>();
            let mut shown_active = *active;
            let mut requested_focus = None;
            if let Some(focused_index) = tabs.iter().position(|pane| {
                tab_ui.memory(|memory| {
                    memory.has_focus(egui::Id::new(("polyorama.dock.tab", pane.0)))
                })
            }) {
                let focused_id = egui::Id::new(("polyorama.dock.tab", tabs[focused_index].0));
                tab_ui.memory_mut(|memory| {
                    memory.set_focus_lock_filter(
                        focused_id,
                        egui::EventFilter {
                            horizontal_arrows: true,
                            ..Default::default()
                        },
                    );
                });
                let keyboard_target = tab_ui.input_mut(|input| {
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) {
                        Some((focused_index + tabs.len() - 1) % tabs.len())
                    } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) {
                        Some((focused_index + 1) % tabs.len())
                    } else if input.consume_key(egui::Modifiers::NONE, egui::Key::Home) {
                        Some(0)
                    } else if input.consume_key(egui::Modifiers::NONE, egui::Key::End) {
                        Some(tabs.len() - 1)
                    } else {
                        None
                    }
                });
                if let Some(target) = keyboard_target {
                    // The focused tab owns these keys. In particular, this clears a
                    // spatial focus move that egui may already have queued when the
                    // tab first gained focus in the preceding pass.
                    tab_ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
                    shown_active = target;
                    let pane = tabs[target];
                    requested_focus = Some(pane);
                    behaviour.pending = Some(DockAction::Activate(pane));
                }
            }
            let minimum_hit = text_context.tokens.geometry.minimum_hit_size.0;
            let allocation = allocate_tab_strip(
                &desired_widths,
                shown_active,
                (tab_rect.width() - strip_padding * 2.0).max(0.0),
                minimum_hit,
                gap,
            );
            let mut x = tab_rect.left() + strip_padding;
            let parent_id = TextComponentId::new(TextComponentKind::DockTabStrip, id.0);
            let mut presentations = Vec::with_capacity(allocation.visible.len());
            for (&index, &width) in allocation.visible.iter().zip(&allocation.widths) {
                let pane = tabs[index];
                let item_rect = Rect::from_min_size(
                    Pos2::new(x, tab_rect.center().y - tab_visual_height * 0.5),
                    egui::vec2(width, tab_visual_height),
                );
                let item_id = egui::Id::new(("polyorama.dock.tab", pane.0));
                let hit_rect = minimum_hit_rect(item_rect, minimum_hit, tab_rect);
                let response = dock_tab_interaction(&mut tab_ui, item_id, hit_rect);
                presenter.record_tab_rect(
                    pane,
                    hit_rect,
                    shown_active == index,
                    response.has_focus(),
                );
                if response.clicked() {
                    shown_active = index;
                    behaviour.pending = Some(DockAction::Activate(pane));
                }
                if response.drag_started() {
                    behaviour.dragging = Some(pane);
                }
                presentations.push((index, pane, item_rect, response));
                x += width + gap;
            }
            let mut overflow_response = None;
            if allocation.overflow {
                let overflow_rect = Rect::from_min_size(
                    Pos2::new(
                        (tab_rect.right() - strip_padding - minimum_hit).max(tab_rect.left()),
                        tab_rect.center().y - minimum_hit * 0.5,
                    ),
                    egui::vec2(
                        minimum_hit.min(tab_rect.width()),
                        minimum_hit.min(tab_rect.height()),
                    ),
                );
                let overflow_id = egui::Id::new(("polyorama.dock.tab-overflow", id.0));
                let response = dock_overflow_trigger(
                    &mut tab_ui,
                    overflow_id,
                    id.0,
                    overflow_rect,
                    &text_context.tokens,
                );
                egui::Popup::menu(&response).show(|ui| {
                    for (index, pane) in tabs.iter().copied().enumerate() {
                        let option =
                            ui.selectable_label(index == shown_active, presenter.title(pane));
                        record_native_text_control(&option, NativeTextControlKind::Selectable);
                        if option.clicked() {
                            behaviour.pending = Some(DockAction::Activate(pane));
                            ui.close();
                        }
                    }
                });
                overflow_response = Some(response);
            }
            if let Some(pane) = requested_focus {
                if let Some((_, _, _, response)) = presentations
                    .iter()
                    .find(|(_, candidate, _, _)| *candidate == pane)
                {
                    response.request_focus();
                } else if let Some(response) = &overflow_response {
                    response.request_focus();
                }
            }
            for (index, pane, item_rect, response) in presentations {
                let observation = paint_dock_tab(
                    &mut tab_ui,
                    &response,
                    presenter.title(pane),
                    DockTabSpec {
                        selected: index == shown_active,
                        visual_rect: item_rect,
                        font_scale: text_context.font_scale,
                        component_id: TextComponentId::new(
                            TextComponentKind::DockTab,
                            u64::from(pane.0),
                        ),
                        parent_id,
                    },
                    &text_context.tokens,
                );
                if let Some(observation) = observation {
                    presenter.record_text_layout(observation);
                }
            }
            ui.painter().hline(
                tab_rect.x_range(),
                tab_rect.bottom(),
                ui.visuals().widgets.noninteractive.bg_stroke,
            );
            if let Some(target) = tabs.get(shown_active).copied() {
                let body_response = ui.interact(
                    body,
                    egui::Id::new(("polyorama.dock.body", target.0)),
                    egui::Sense::click(),
                );
                if body_response.clicked() {
                    behaviour.pending = Some(DockAction::Activate(target));
                }
                let dock_pointer = ui.input(|input| input.pointer.hover_pos());
                if let Some(dragged) = behaviour.dragging
                    && dock_pointer.is_some_and(|pointer| body.contains(pointer))
                {
                    let pointer = dock_pointer.unwrap_or(body.center());
                    let relative = egui::vec2(
                        (pointer.x - body.left()) / body.width().max(1.0),
                        (pointer.y - body.top()) / body.height().max(1.0),
                    );
                    let drop = if relative.x < 0.22 {
                        DockDrop::Left
                    } else if relative.x > 0.78 {
                        DockDrop::Right
                    } else if relative.y < 0.22 {
                        DockDrop::Top
                    } else if relative.y > 0.78 {
                        DockDrop::Bottom
                    } else {
                        DockDrop::Tab
                    };
                    let preview = match drop {
                        DockDrop::Left => {
                            Rect::from_min_max(body.min, Pos2::new(body.center().x, body.bottom()))
                        }
                        DockDrop::Right => {
                            Rect::from_min_max(Pos2::new(body.center().x, body.top()), body.max)
                        }
                        DockDrop::Top => {
                            Rect::from_min_max(body.min, Pos2::new(body.right(), body.center().y))
                        }
                        DockDrop::Bottom => {
                            Rect::from_min_max(Pos2::new(body.left(), body.center().y), body.max)
                        }
                        DockDrop::Tab => body.shrink(12.0),
                    };
                    ui.painter().rect_stroke(
                        preview,
                        4.0,
                        ui.visuals().selection.stroke,
                        egui::StrokeKind::Inside,
                    );
                    if ui.input(|input| input.pointer.any_released()) {
                        behaviour.pending = Some(DockAction::Move {
                            pane: dragged,
                            target,
                            drop,
                        });
                        behaviour.dragging = None;
                    }
                }
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(body)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        ui.set_clip_rect(ui.clip_rect().intersect(body));
                        presenter.pane_ui(ui, target, body);
                    },
                );
            }
        }
    }
}

fn split_rects(rect: Rect, horizontal: bool, fraction: f32) -> (Rect, Rect, Rect) {
    let length = if horizontal {
        rect.width()
    } else {
        rect.height()
    };
    let first_length = (length * fraction - SPLITTER_VISUAL_WIDTH * 0.5).max(40.0);
    if horizontal {
        let cut = rect.left() + first_length;
        (
            Rect::from_min_max(rect.min, Pos2::new(cut, rect.bottom())),
            Rect::from_min_max(
                Pos2::new(cut, rect.top()),
                Pos2::new(cut + SPLITTER_VISUAL_WIDTH, rect.bottom()),
            ),
            Rect::from_min_max(Pos2::new(cut + SPLITTER_VISUAL_WIDTH, rect.top()), rect.max),
        )
    } else {
        let cut = rect.top() + first_length;
        (
            Rect::from_min_max(rect.min, Pos2::new(rect.right(), cut)),
            Rect::from_min_max(
                Pos2::new(rect.left(), cut),
                Pos2::new(rect.right(), cut + SPLITTER_VISUAL_WIDTH),
            ),
            Rect::from_min_max(
                Pos2::new(rect.left(), cut + SPLITTER_VISUAL_WIDTH),
                rect.max,
            ),
        )
    }
}

fn adjusted_split_fraction(before: f32, steps: i32) -> f32 {
    if steps == 0 {
        return before;
    }
    (before + steps as f32 * SPLITTER_KEY_STEP).clamp(0.1, 0.9)
}

#[derive(Clone, Copy, Debug)]
pub struct ViewportAllocation {
    pub pane: PaneId,
    pub logical_rect: Rect,
    pub physical_origin: PhysicalPoint,
    pub physical_size: PhysicalPoint,
    pub scale_factor: f32,
    pub pointer_local: Option<ViewportPoint>,
    pub focused: bool,
}

pub fn allocate_viewport(
    ui: &mut Ui,
    pane: PaneId,
    desired: egui::Vec2,
) -> (ViewportAllocation, egui::Response) {
    ui.push_id(("pane", pane.0, "viewport"), |ui| {
        let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click_and_drag());
        let scale = ui.ctx().pixels_per_point();
        let pointer_local = response.hover_pos().map(|point| {
            ViewportPoint::new(
                (point.x - rect.left()) as f64,
                (point.y - rect.top()) as f64,
            )
        });
        (
            ViewportAllocation {
                pane,
                logical_rect: rect,
                physical_origin: PhysicalPoint::new(
                    (rect.left() * scale) as f64,
                    (rect.top() * scale) as f64,
                ),
                physical_size: PhysicalPoint::new(
                    (rect.width() * scale) as f64,
                    (rect.height() * scale) as f64,
                ),
                scale_factor: scale,
                pointer_local,
                focused: response.has_focus(),
            },
            response,
        )
    })
    .inner
}

pub fn logical(point: Pos2) -> LogicalPoint {
    LogicalPoint::new(point.x as f64, point.y as f64)
}

#[derive(Clone)]
pub struct ScalarPaintCallback {
    pub frame_number: u64,
    request: Arc<RwLock<Option<ImageRenderRequest>>>,
}

impl egui_wgpu::CallbackTrait for ScalarPaintCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(renderer) = resources.get_mut::<ScalarRenderer>() {
            let request = self.request.read().expect("render request lock poisoned");
            if let Some(request) = request.as_ref() {
                renderer.prepare(device, queue, self.frame_number, request);
            }
        }
        Vec::new()
    }

    fn paint(
        &self,
        info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(renderer) = resources.get::<ScalarRenderer>() else {
            return;
        };
        let Some(pane) = self
            .request
            .read()
            .expect("render request lock poisoned")
            .as_ref()
            .map(|request| request.pane)
        else {
            return;
        };
        let viewport = info.viewport_in_pixels();
        let clip = info.clip_rect_in_pixels();
        renderer.paint(
            pane,
            PixelRect {
                x: viewport.left_px.max(0) as u32,
                y: viewport.top_px.max(0) as u32,
                width: viewport.width_px.max(0) as u32,
                height: viewport.height_px.max(0) as u32,
            },
            PixelRect {
                x: clip.left_px.max(0) as u32,
                y: clip.top_px.max(0) as u32,
                width: clip.width_px.max(0) as u32,
                height: clip.height_px.max(0) as u32,
            },
            render_pass,
        );
    }
}

#[derive(Clone)]
pub struct ImagePlanTarget {
    pane: PaneId,
    request: Arc<RwLock<Option<ImageRenderRequest>>>,
}

/// Stage an opaque callback in egui's correct paint list; its request is finalised later.
pub fn stage_render_callback(
    ui: &Ui,
    rect: Rect,
    frame_number: u64,
    request: ImageRenderRequest,
) -> ImagePlanTarget {
    let pane = request.pane;
    let request = Arc::new(RwLock::new(Some(request)));
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
        rect,
        ScalarPaintCallback {
            frame_number,
            request: request.clone(),
        },
    ));
    ImagePlanTarget { pane, request }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderPlanSubmissionError {
    CountMismatch {
        requests: usize,
        targets: usize,
    },
    PaneMismatch {
        index: usize,
        request: PaneId,
        target: PaneId,
    },
    DuplicatePane(PaneId),
}

impl std::fmt::Display for RenderPlanSubmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CountMismatch { requests, targets } => write!(
                formatter,
                "render plan has {requests} requests but {targets} staged targets"
            ),
            Self::PaneMismatch {
                index,
                request,
                target,
            } => write!(
                formatter,
                "render plan request {index} is pane {} but its staged target is pane {}",
                request.0, target.0
            ),
            Self::DuplicatePane(pane) => {
                write!(
                    formatter,
                    "render plan contains pane {} more than once",
                    pane.0
                )
            }
        }
    }
}

impl std::error::Error for RenderPlanSubmissionError {}

fn validate_plan_target_panes(
    requests: &[PaneId],
    targets: &[PaneId],
) -> Result<(), RenderPlanSubmissionError> {
    if requests.len() != targets.len() {
        return Err(RenderPlanSubmissionError::CountMismatch {
            requests: requests.len(),
            targets: targets.len(),
        });
    }
    let mut unique = std::collections::BTreeSet::new();
    for (index, (request, target)) in requests.iter().zip(targets).enumerate() {
        if !unique.insert(*request) {
            return Err(RenderPlanSubmissionError::DuplicatePane(*request));
        }
        if request != target {
            return Err(RenderPlanSubmissionError::PaneMismatch {
                index,
                request: *request,
                target: *target,
            });
        }
    }
    Ok(())
}

/// Publish the complete typed frame plan before callback preparation begins.
/// Validation occurs before any target is changed. On failure, every staged image callback is
/// disabled so a release build cannot silently paint stale or mismatched data.
pub fn submit_render_plan(
    plan: &RenderPlan,
    targets: &[ImagePlanTarget],
) -> Result<(), RenderPlanSubmissionError> {
    let request_panes: Vec<_> = plan.images.iter().map(|request| request.pane).collect();
    let target_panes: Vec<_> = targets.iter().map(|target| target.pane).collect();
    if let Err(error) = validate_plan_target_panes(&request_panes, &target_panes) {
        for target in targets {
            *target
                .request
                .write()
                .expect("render request lock poisoned") = None;
        }
        return Err(error);
    }
    for (request, target) in plan.images.iter().cloned().zip(targets) {
        *target
            .request
            .write()
            .expect("render request lock poisoned") = Some(request);
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct RendererMaintenanceCallback {
    frame_number: u64,
    source_generation: u64,
}

impl egui_wgpu::CallbackTrait for RendererMaintenanceCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(renderer) = resources.get_mut::<ScalarRenderer>() {
            renderer.maintain_frame(device, queue, self.frame_number, self.source_generation);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        _render_pass: &mut wgpu::RenderPass<'static>,
        _resources: &egui_wgpu::CallbackResources,
    ) {
    }
}

/// Stage renderer maintenance before pane presentation so uploads and per-frame metrics progress
/// even when the canonical workspace currently exposes no image callback.
pub fn stage_renderer_maintenance(ui: &Ui, rect: Rect, frame_number: u64, source_generation: u64) {
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
        rect,
        RendererMaintenanceCallback {
            frame_number,
            source_generation,
        },
    ));
}

#[cfg(test)]
mod tests {
    use polyorama_core::{CommandHistory, Document, Session};

    use super::*;

    fn dock_text_context() -> DockTextContext {
        DockTextContext {
            tokens: DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable),
            font_scale: 1.0,
        }
    }

    struct GeometryPresenter {
        tabs: Vec<(PaneId, Rect)>,
        tab_states: Vec<(PaneId, Rect, bool, bool)>,
        bodies: Vec<(PaneId, Rect)>,
        splitters: Vec<(DockNodeId, Rect)>,
        splitter_states: Vec<(DockNodeId, Rect, bool, bool)>,
        text_layouts: Vec<TextLayoutObservation>,
        greedy_pane: Option<PaneId>,
        title: &'static str,
    }

    impl Default for GeometryPresenter {
        fn default() -> Self {
            Self {
                tabs: Vec::new(),
                tab_states: Vec::new(),
                bodies: Vec::new(),
                splitters: Vec::new(),
                splitter_states: Vec::new(),
                text_layouts: Vec::new(),
                greedy_pane: None,
                title: "Pane",
            }
        }
    }

    impl PanePresenter for GeometryPresenter {
        fn title(&self, _pane: PaneId) -> &'static str {
            self.title
        }

        fn pane_ui(&mut self, ui: &mut Ui, pane: PaneId, pane_rect: Rect) {
            self.bodies.push((pane, pane_rect));
            if self.greedy_pane == Some(pane) {
                ui.interact(
                    Rect::EVERYTHING,
                    ui.id().with("greedy-pane-interaction"),
                    egui::Sense::drag(),
                );
            }
        }

        fn record_tab_rect(&mut self, pane: PaneId, rect: Rect, selected: bool, focused: bool) {
            self.tabs.push((pane, rect));
            self.tab_states.push((pane, rect, selected, focused));
        }

        fn record_text_layout(&mut self, observation: TextLayoutObservation) {
            self.text_layouts.push(observation);
        }

        fn record_splitter_rect(
            &mut self,
            node: DockNodeId,
            rect: Rect,
            horizontal: bool,
            focused: bool,
        ) {
            self.splitters.push((node, rect));
            self.splitter_states.push((node, rect, horizontal, focused));
        }
    }

    impl GeometryPresenter {
        fn splitter(&self, node: DockNodeId) -> Rect {
            self.splitters
                .iter()
                .find_map(|(candidate, rect)| (*candidate == node).then_some(*rect))
                .expect("requested splitter is presented")
        }
    }

    fn dock_frame(
        context: &egui::Context,
        workspace: &mut Workspace,
        behaviour: &mut DockBehaviour,
        events: Vec<egui::Event>,
    ) -> (Option<Command>, GeometryPresenter) {
        dock_frame_with_presenter(
            context,
            workspace,
            behaviour,
            events,
            GeometryPresenter::default(),
        )
    }

    fn dock_frame_with_presenter(
        context: &egui::Context,
        workspace: &mut Workspace,
        behaviour: &mut DockBehaviour,
        events: Vec<egui::Event>,
        mut presenter: GeometryPresenter,
    ) -> (Option<Command>, GeometryPresenter) {
        let root = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        let input = egui::RawInput {
            screen_rect: Some(root),
            focused: true,
            events,
            ..Default::default()
        };
        let mut command = None;
        let mut output = context.run_ui(input, |ui| {
            command = ui
                .scope_builder(egui::UiBuilder::new().max_rect(root), |ui| {
                    dock_workspace(
                        ui,
                        workspace,
                        behaviour,
                        &mut presenter,
                        dock_text_context(),
                    )
                })
                .inner;
        });
        output.textures_delta.clear();
        (command, presenter)
    }

    fn dock_frame_with_parent_id(
        context: &egui::Context,
        workspace: &mut Workspace,
        behaviour: &mut DockBehaviour,
        events: Vec<egui::Event>,
        parent_id: &'static str,
    ) -> (Option<Command>, GeometryPresenter) {
        let root = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        let input = egui::RawInput {
            screen_rect: Some(root),
            focused: true,
            events,
            ..Default::default()
        };
        let mut command = None;
        let mut presenter = GeometryPresenter::default();
        let mut output = context.run_ui(input, |ui| {
            command = ui
                .scope_builder(
                    egui::UiBuilder::new()
                        .id(egui::Id::new(parent_id))
                        .max_rect(root),
                    |ui| {
                        dock_workspace(
                            ui,
                            workspace,
                            behaviour,
                            &mut presenter,
                            dock_text_context(),
                        )
                    },
                )
                .inner;
        });
        output.textures_delta.clear();
        (command, presenter)
    }

    fn dock_accesskit_frame(
        context: &egui::Context,
        workspace: &mut Workspace,
        behaviour: &mut DockBehaviour,
        events: Vec<egui::Event>,
    ) -> (Option<Command>, GeometryPresenter, egui::FullOutput) {
        let root = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        let input = egui::RawInput {
            screen_rect: Some(root),
            focused: true,
            events,
            ..Default::default()
        };
        let mut command = None;
        let mut presenter = GeometryPresenter::default();
        let output = context.run_ui(input, |ui| {
            command = ui
                .scope_builder(egui::UiBuilder::new().max_rect(root), |ui| {
                    dock_workspace(
                        ui,
                        workspace,
                        behaviour,
                        &mut presenter,
                        dock_text_context(),
                    )
                })
                .inner;
        });
        (command, presenter, output)
    }

    fn pointer_button(position: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn selected_tab_author_ids(output: &mut egui::FullOutput) -> Vec<String> {
        let update = output
            .platform_output
            .accesskit_update
            .take()
            .expect("AccessKit update");
        output.textures_delta.clear();
        update
            .nodes
            .iter()
            .filter(|(_, node)| {
                node.role() == egui::accesskit::Role::Tab && node.is_selected() == Some(true)
            })
            .map(|(_, node)| node.author_id().expect("tab author ID").to_owned())
            .collect()
    }

    #[test]
    fn render_plan_correspondence_rejects_count_order_and_duplicates() {
        assert_eq!(
            validate_plan_target_panes(&[PaneId(1)], &[]),
            Err(RenderPlanSubmissionError::CountMismatch {
                requests: 1,
                targets: 0,
            })
        );
        assert!(matches!(
            validate_plan_target_panes(&[PaneId(1), PaneId(2)], &[PaneId(2), PaneId(1)]),
            Err(RenderPlanSubmissionError::PaneMismatch { index: 0, .. })
        ));
        assert_eq!(
            validate_plan_target_panes(&[PaneId(1), PaneId(1)], &[PaneId(1), PaneId(1)]),
            Err(RenderPlanSubmissionError::DuplicatePane(PaneId(1)))
        );
        assert!(
            validate_plan_target_panes(&[PaneId(1), PaneId(2)], &[PaneId(1), PaneId(2)]).is_ok()
        );
    }

    #[test]
    fn aborted_dock_gestures_stop_interaction_when_the_pointer_is_released() {
        let mut behaviour = DockBehaviour {
            dragging: Some(PaneId(4)),
            split_preview: Some(SplitPreview {
                node: DockNodeId(2),
                before: 0.5,
                after: 0.6,
                pointer_origin: Pos2::ZERO,
            }),
            interaction_active: true,
            ..Default::default()
        };

        behaviour.finish_frame(false);

        assert!(behaviour.dragging.is_none());
        assert!(behaviour.split_preview.is_none());
        assert!(!behaviour.interaction_active());
    }

    #[test]
    fn active_dock_gesture_keeps_the_recorded_interaction_signal() {
        let mut behaviour = DockBehaviour {
            dragging: Some(PaneId(4)),
            ..Default::default()
        };

        behaviour.finish_frame(true);

        assert_eq!(behaviour.dragging, Some(PaneId(4)));
        assert!(behaviour.interaction_active());
    }

    #[test]
    fn dock_reports_current_semantic_tab_body_and_splitter_geometry() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        let mut output = context.run_ui(Default::default(), |ui| {
            let root = Rect::from_min_size(ui.min_rect().min, egui::vec2(800.0, 600.0));
            let mut workspace = Workspace::analytical_default();
            let mut expected_bodies = Vec::new();
            workspace.root.active_panes(&mut expected_bodies);
            let mut behaviour = DockBehaviour::default();
            let mut presenter = GeometryPresenter::default();

            ui.scope_builder(egui::UiBuilder::new().max_rect(root), |ui| {
                dock_workspace(
                    ui,
                    &mut workspace,
                    &mut behaviour,
                    &mut presenter,
                    dock_text_context(),
                )
            });

            assert_eq!(presenter.tabs.len(), 8);
            assert_eq!(presenter.text_layouts.len(), 8);
            let findings = audit_text_layouts(&presenter.text_layouts);
            assert!(
                findings.is_empty(),
                "{findings:?}: {:?}",
                presenter.text_layouts
            );
            assert_eq!(presenter.bodies.len(), expected_bodies.len());
            assert!(!presenter.splitters.is_empty());
            for (_, rect) in presenter
                .tabs
                .iter()
                .chain(&presenter.bodies)
                .map(|(id, rect)| (*id, *rect))
            {
                assert!(rect.is_positive());
                assert!(root.contains_rect(rect));
            }
            assert!(
                presenter
                    .splitters
                    .iter()
                    .all(|(_, rect)| rect.is_positive() && root.contains_rect(*rect))
            );
        });
        output.textures_delta.clear();
    }

    #[test]
    fn dock_emits_accesskit_tab_and_splitter_metadata() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        context.enable_accesskit();
        let mut workspace = Workspace::analytical_default();
        let mut behaviour = DockBehaviour::default();
        let mut presenter = GeometryPresenter::default();
        let root = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(root),
                ..Default::default()
            },
            |context| {
                egui::CentralPanel::default().show(context, |ui| {
                    let _ = dock_workspace(
                        ui,
                        &mut workspace,
                        &mut behaviour,
                        &mut presenter,
                        dock_text_context(),
                    );
                });
            },
        );
        let update = output
            .platform_output
            .accesskit_update
            .expect("AccessKit is enabled");
        output.textures_delta.clear();
        let (tab_id, tab) = update
            .nodes
            .iter()
            .find(|(_, node)| {
                node.role() == egui::accesskit::Role::Tab && node.is_selected() == Some(true)
            })
            .expect("selected tab node");
        let tab_list = update
            .nodes
            .iter()
            .map(|(_, node)| node)
            .find(|node| {
                node.role() == egui::accesskit::Role::TabList && node.children().contains(tab_id)
            })
            .expect("tab-list node");
        assert_eq!(tab_list.label(), Some("Workspace panes"));
        assert!(tab_list.author_id().is_some());
        assert_eq!(tab.label(), Some("Pane"));
        assert!(tab.supports_action(egui::accesskit::Action::Click));
        assert!(tab.author_id().is_some());
        assert_eq!(tab.is_selected(), Some(true));
        let tab_bounds = tab.bounds().expect("tab semantic bounds");
        assert!(tab_bounds.width() >= 32.0);
        assert!(tab_bounds.height() >= 32.0);
        assert!((presenter.tabs[0].1.height() - tab_bounds.height() as f32).abs() < 0.001);
        let splitter = update
            .nodes
            .iter()
            .map(|(_, node)| node)
            .find(|node| node.role() == egui::accesskit::Role::Splitter)
            .expect("splitter node");
        assert!(splitter.numeric_value().is_some());
        assert_eq!(splitter.min_numeric_value(), Some(0.1));
        assert_eq!(splitter.max_numeric_value(), Some(0.9));
        assert!(splitter.supports_action(egui::accesskit::Action::Increment));
        assert!(splitter.supports_action(egui::accesskit::Action::Decrement));
        let splitter_bounds = splitter.bounds().expect("splitter semantic bounds");
        assert!(splitter_bounds.width() >= 32.0 || splitter_bounds.height() >= 32.0);
        assert!(splitter.orientation().is_some());

        let mut nodes = vec![UiNode::container(
            SemanticUiId::root(),
            None,
            UiRole::Application,
            root.into(),
        )];
        nodes.extend(
            presenter
                .tab_states
                .iter()
                .map(|(pane, rect, selected, focused)| UiNode {
                    id: SemanticUiId::tab(*pane),
                    parent: Some(SemanticUiId::root()),
                    role: UiRole::Tab,
                    name: "Pane".into(),
                    description: None,
                    rect: (*rect).into(),
                    enabled: true,
                    focused: *focused,
                    selected: *selected,
                    checked: None,
                    expanded: None,
                    pane: Some(*pane),
                    domain_reference: Some(DomainReference::Pane(*pane)),
                    actions: Vec::new(),
                    text_selectable: false,
                    disabled_reason: None,
                }),
        );
        nodes.extend(
            presenter
                .splitter_states
                .iter()
                .map(|(node, rect, horizontal, focused)| UiNode {
                    id: SemanticUiId::splitter(*node),
                    parent: Some(SemanticUiId::root()),
                    role: UiRole::Splitter,
                    name: if *horizontal {
                        "Vertical splitter".into()
                    } else {
                        "Horizontal splitter".into()
                    },
                    description: Some("Resize adjacent dock panes".into()),
                    rect: (*rect).into(),
                    enabled: true,
                    focused: *focused,
                    selected: false,
                    checked: None,
                    expanded: None,
                    pane: None,
                    domain_reference: Some(DomainReference::DockNode(*node)),
                    actions: Vec::new(),
                    text_selectable: false,
                    disabled_reason: None,
                }),
        );
        let snapshot = UiSnapshot {
            root: SemanticUiId::root(),
            nodes,
            ..Default::default()
        };
        let findings = audit_accesskit(&snapshot, &update);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn dock_tab_accesskit_identity_survives_a_canonical_pane_move() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        context.enable_accesskit();
        let mut workspace = Workspace::analytical_default();
        let mut behaviour = DockBehaviour::default();
        let (_, _, mut before_output) =
            dock_accesskit_frame(&context, &mut workspace, &mut behaviour, Vec::new());
        let before_update = before_output
            .platform_output
            .accesskit_update
            .take()
            .expect("initial AccessKit update");
        before_output.textures_delta.clear();
        let tab_id = |update: &egui::accesskit::TreeUpdate| {
            update
                .nodes
                .iter()
                .find_map(|(id, node)| {
                    (node.author_id() == Some("polyorama.dock.tab.1")).then_some(*id)
                })
                .expect("primary-view tab node")
        };
        let before_id = tab_id(&before_update);

        assert!(workspace.move_pane(PaneId(1), PaneId(5), DockDrop::Tab));
        let (_, _, mut after_output) =
            dock_accesskit_frame(&context, &mut workspace, &mut behaviour, Vec::new());
        let after_update = after_output
            .platform_output
            .accesskit_update
            .take()
            .expect("moved AccessKit update");
        after_output.textures_delta.clear();
        assert_eq!(before_id, tab_id(&after_update));
    }

    #[test]
    fn focused_tab_enter_and_space_activate_through_response_clicked() {
        for activation_key in [egui::Key::Enter, egui::Key::Space] {
            let context = egui::Context::default();
            crate::install_typography_fonts(&context);
            let mut workspace = Workspace::analytical_default();
            workspace.root = DockNode::Tabs {
                id: DockNodeId(41),
                tabs: vec![PaneId(1), PaneId(2)],
                active: 0,
            };
            workspace.active_pane = PaneId(1);
            let mut behaviour = DockBehaviour::default();
            let (_, initial) = dock_frame(&context, &mut workspace, &mut behaviour, Vec::new());
            let second_tab = initial
                .tabs
                .iter()
                .find_map(|(pane, rect)| (*pane == PaneId(2)).then_some(rect.center()))
                .expect("second tab is visible");
            let _ = dock_frame(
                &context,
                &mut workspace,
                &mut behaviour,
                vec![
                    egui::Event::PointerMoved(second_tab),
                    pointer_button(second_tab, true),
                ],
            );
            let _ = dock_frame(
                &context,
                &mut workspace,
                &mut behaviour,
                vec![pointer_button(second_tab, false)],
            );
            assert_eq!(workspace.active_pane, PaneId(2));

            workspace.activate(PaneId(1));
            let (command, _) = dock_frame(
                &context,
                &mut workspace,
                &mut behaviour,
                vec![key(activation_key)],
            );
            assert!(command.is_none());
            assert_eq!(workspace.active_pane, PaneId(2), "key={activation_key:?}");
        }
    }

    #[test]
    fn accesskit_splitter_adjustments_emit_the_existing_resize_command() {
        for (action, expected_after) in [
            (egui::accesskit::Action::Increment, 0.77),
            (egui::accesskit::Action::Decrement, 0.67),
        ] {
            let context = egui::Context::default();
            crate::install_typography_fonts(&context);
            context.enable_accesskit();
            let mut workspace = Workspace::analytical_default();
            let mut behaviour = DockBehaviour::default();
            let (_, _, mut output) =
                dock_accesskit_frame(&context, &mut workspace, &mut behaviour, Vec::new());
            let update = output
                .platform_output
                .accesskit_update
                .take()
                .expect("AccessKit update");
            output.textures_delta.clear();
            let target_node = update
                .nodes
                .iter()
                .find_map(|(id, node)| {
                    (node.role() == egui::accesskit::Role::Splitter
                        && node.author_id() == Some("polyorama.dock.splitter.1"))
                    .then_some(*id)
                })
                .expect("root splitter node");
            let request = egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
                action,
                target_tree: egui::accesskit::TreeId::ROOT,
                target_node,
                data: None,
            });
            let (command, _, mut output) =
                dock_accesskit_frame(&context, &mut workspace, &mut behaviour, vec![request]);
            output.textures_delta.clear();
            let Command::ResizeSplit {
                node,
                before,
                after,
            } = command.expect("one splitter command")
            else {
                panic!("unexpected command");
            };
            assert_eq!(node, DockNodeId(1));
            assert_eq!(before, 0.72);
            assert!((after - expected_after).abs() < 1.0e-6, "action={action:?}");
        }
    }

    #[test]
    fn focused_splitter_arrow_emits_one_resize_command() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        context.enable_accesskit();
        let mut workspace = Workspace::analytical_default();
        let mut behaviour = DockBehaviour::default();
        let (_, _, mut output) =
            dock_accesskit_frame(&context, &mut workspace, &mut behaviour, Vec::new());
        let update = output
            .platform_output
            .accesskit_update
            .take()
            .expect("AccessKit update");
        output.textures_delta.clear();
        let target_node = update
            .nodes
            .iter()
            .find_map(|(id, node)| {
                (node.author_id() == Some("polyorama.dock.splitter.1")).then_some(*id)
            })
            .expect("root splitter node");
        let focus = egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
            action: egui::accesskit::Action::Focus,
            target_tree: egui::accesskit::TreeId::ROOT,
            target_node,
            data: None,
        });
        let (focus_command, _, mut output) =
            dock_accesskit_frame(&context, &mut workspace, &mut behaviour, vec![focus]);
        output.textures_delta.clear();
        assert!(focus_command.is_none());
        let splitter_id = egui::Id::new(("polyorama.dock.splitter", 1_u64));
        assert_eq!(context.memory(|memory| memory.focused()), Some(splitter_id));
        let (command, _, mut output) = dock_accesskit_frame(
            &context,
            &mut workspace,
            &mut behaviour,
            vec![key(egui::Key::ArrowRight)],
        );
        output.textures_delta.clear();
        let command = command.expect("focused arrow adjustment");
        let Command::ResizeSplit {
            node,
            before,
            after,
        } = &command
        else {
            panic!("unexpected command");
        };
        assert_eq!(*node, DockNodeId(1));
        assert_eq!(*before, 0.72);
        assert!((*after - 0.77).abs() < 1.0e-6);
        assert_eq!(context.memory(|memory| memory.focused()), Some(splitter_id));

        let mut history = CommandHistory::default();
        let mut document = Document::default();
        let mut session = Session::default();
        history.execute(command, &mut document, &mut session, &mut workspace);
        let (second, _, mut output) = dock_accesskit_frame(
            &context,
            &mut workspace,
            &mut behaviour,
            vec![key(egui::Key::ArrowRight)],
        );
        output.textures_delta.clear();
        let Command::ResizeSplit { before, after, .. } = second.expect("second arrow adjustment")
        else {
            panic!("unexpected command");
        };
        assert!((before - 0.77).abs() < 1.0e-6);
        assert!((after - 0.82).abs() < 1.0e-6);
        assert_eq!(context.memory(|memory| memory.focused()), Some(splitter_id));
    }

    #[test]
    fn focused_tab_keys_activate_and_move_focus_across_frames() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        let mut workspace = Workspace::analytical_default();
        workspace.root = DockNode::Tabs {
            id: DockNodeId(41),
            tabs: vec![PaneId(1), PaneId(2), PaneId(3)],
            active: 0,
        };
        let mut behaviour = DockBehaviour::default();
        let (_, initial) = dock_frame(&context, &mut workspace, &mut behaviour, Vec::new());
        let tab = initial.tabs[0].1.center();
        let _ = dock_frame(
            &context,
            &mut workspace,
            &mut behaviour,
            vec![egui::Event::PointerMoved(tab), pointer_button(tab, true)],
        );
        let _ = dock_frame(
            &context,
            &mut workspace,
            &mut behaviour,
            vec![pointer_button(tab, false)],
        );
        for expected in [PaneId(2), PaneId(3), PaneId(1)] {
            let (command, _) = dock_frame(
                &context,
                &mut workspace,
                &mut behaviour,
                vec![key(egui::Key::ArrowRight)],
            );
            assert!(command.is_none());
            assert_eq!(workspace.active_pane, expected);
            assert_eq!(
                context.memory(|memory| memory.focused()),
                Some(egui::Id::new(("polyorama.dock.tab", expected.0)))
            );
        }
    }

    #[test]
    fn tab_activation_frames_keep_selection_body_and_focus_coherent() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        context.enable_accesskit();
        let mut workspace = Workspace::analytical_default();
        workspace.root = DockNode::Tabs {
            id: DockNodeId(41),
            tabs: vec![PaneId(1), PaneId(2)],
            active: 0,
        };
        workspace.active_pane = PaneId(1);
        let mut behaviour = DockBehaviour::default();
        let (_, initial, mut output) =
            dock_accesskit_frame(&context, &mut workspace, &mut behaviour, Vec::new());
        assert_eq!(
            selected_tab_author_ids(&mut output),
            vec!["polyorama.dock.tab.1"]
        );
        let second_tab = initial
            .tabs
            .iter()
            .find_map(|(pane, rect)| (*pane == PaneId(2)).then_some(rect.center()))
            .expect("second tab geometry");
        let (_, _, mut output) = dock_accesskit_frame(
            &context,
            &mut workspace,
            &mut behaviour,
            vec![
                egui::Event::PointerMoved(second_tab),
                pointer_button(second_tab, true),
            ],
        );
        output.textures_delta.clear();
        let (_, released, mut output) = dock_accesskit_frame(
            &context,
            &mut workspace,
            &mut behaviour,
            vec![pointer_button(second_tab, false)],
        );
        assert_eq!(workspace.active_pane, PaneId(2));
        assert_eq!(released.bodies.len(), 1);
        assert_eq!(released.bodies[0].0, PaneId(2));
        assert_eq!(
            selected_tab_author_ids(&mut output),
            vec!["polyorama.dock.tab.2"]
        );

        let (_, arrow, mut output) = dock_accesskit_frame(
            &context,
            &mut workspace,
            &mut behaviour,
            vec![key(egui::Key::ArrowLeft)],
        );
        assert_eq!(workspace.active_pane, PaneId(1));
        assert_eq!(arrow.bodies.len(), 1);
        assert_eq!(arrow.bodies[0].0, PaneId(1));
        assert_eq!(
            selected_tab_author_ids(&mut output),
            vec!["polyorama.dock.tab.1"]
        );
        assert_eq!(
            context.memory(|memory| memory.focused()),
            Some(egui::Id::new(("polyorama.dock.tab", 1_u64)))
        );
    }

    #[test]
    fn roving_focus_reaches_hidden_overflow_tabs_and_wraps() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        context.enable_accesskit();
        let mut workspace = Workspace::analytical_default();
        workspace.root = DockNode::Tabs {
            id: DockNodeId(41),
            tabs: (1_u32..=30).map(PaneId).collect(),
            active: 0,
        };
        workspace.active_pane = PaneId(1);
        let mut behaviour = DockBehaviour::default();
        let (_, initial, mut output) =
            dock_accesskit_frame(&context, &mut workspace, &mut behaviour, Vec::new());
        assert!(initial.tabs.len() < 30);
        let update = output
            .platform_output
            .accesskit_update
            .take()
            .expect("AccessKit update");
        output.textures_delta.clear();
        let first_tab = update
            .nodes
            .iter()
            .find_map(|(id, node)| {
                (node.author_id() == Some("polyorama.dock.tab.1")).then_some(*id)
            })
            .expect("first tab node");
        let focus = egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
            action: egui::accesskit::Action::Focus,
            target_tree: egui::accesskit::TreeId::ROOT,
            target_node: first_tab,
            data: None,
        });
        let (_, _, mut output) =
            dock_accesskit_frame(&context, &mut workspace, &mut behaviour, vec![focus]);
        output.textures_delta.clear();

        let (_, ended, mut output) = dock_accesskit_frame(
            &context,
            &mut workspace,
            &mut behaviour,
            vec![key(egui::Key::End)],
        );
        assert_eq!(workspace.active_pane, PaneId(30));
        assert!(ended.tabs.iter().any(|(pane, _)| *pane == PaneId(30)));
        assert!(ended.bodies.iter().any(|(pane, _)| *pane == PaneId(30)));
        assert_eq!(
            selected_tab_author_ids(&mut output),
            vec!["polyorama.dock.tab.30"]
        );
        assert_eq!(
            context.memory(|memory| memory.focused()),
            Some(egui::Id::new(("polyorama.dock.tab", 30_u32)))
        );

        let (_, wrapped, mut output) = dock_accesskit_frame(
            &context,
            &mut workspace,
            &mut behaviour,
            vec![key(egui::Key::ArrowRight)],
        );
        assert_eq!(workspace.active_pane, PaneId(1));
        assert!(wrapped.tabs.iter().any(|(pane, _)| *pane == PaneId(1)));
        assert!(wrapped.bodies.iter().any(|(pane, _)| *pane == PaneId(1)));
        assert_eq!(
            selected_tab_author_ids(&mut output),
            vec!["polyorama.dock.tab.1"]
        );
        assert_eq!(
            context.memory(|memory| memory.focused()),
            Some(egui::Id::new(("polyorama.dock.tab", 1_u32)))
        );
    }

    #[test]
    fn dock_tabs_are_measured_bounded_and_auditable_across_responsive_cases() {
        let labels = [
            "Results",
            "Results and linked measurements from the active selection",
            "results_without_any_available_line_break_opportunity_0123456789",
        ];
        let widths = [280.0, 500.0, 900.0];
        let densities = [DensityVariant::Compact, DensityVariant::Comfortable];
        let scales = [1.0, 1.25, 1.5];

        for label in labels {
            for width in widths {
                for density in densities {
                    for scale in scales {
                        let context = egui::Context::default();
                        crate::install_typography_fonts(&context);
                        let root = Rect::from_min_size(Pos2::ZERO, egui::vec2(width, 320.0));
                        let input = egui::RawInput {
                            screen_rect: Some(root),
                            focused: true,
                            ..Default::default()
                        };
                        let mut workspace = Workspace::analytical_default();
                        workspace.root = DockNode::Tabs {
                            id: DockNodeId(41),
                            tabs: vec![PaneId(1), PaneId(2), PaneId(3)],
                            active: 0,
                        };
                        workspace.active_pane = PaneId(1);
                        let mut behaviour = DockBehaviour::default();
                        let mut presenter = GeometryPresenter {
                            title: label,
                            ..Default::default()
                        };
                        let mut output = context.run_ui(input, |ui| {
                            let _ = dock_workspace(
                                ui,
                                &mut workspace,
                                &mut behaviour,
                                &mut presenter,
                                DockTextContext {
                                    tokens: DesignTokens::resolve(ThemeVariant::Dark, density),
                                    font_scale: scale,
                                },
                            );
                        });
                        output.textures_delta.clear();

                        assert!((1..=3).contains(&presenter.tabs.len()));
                        assert_eq!(presenter.text_layouts.len(), presenter.tabs.len());
                        assert!(
                            presenter
                                .tabs
                                .iter()
                                .all(|(_, tab)| root.contains_rect(*tab))
                        );
                        assert!(
                            presenter
                                .tabs
                                .windows(2)
                                .all(|pair| pair[0].1.right() <= pair[1].1.left())
                        );
                        assert!(
                            audit_text_layouts(&presenter.text_layouts).is_empty(),
                            "label={label:?} width={width} density={density:?} scale={scale}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn splitter_preview_tracks_total_drag_and_release_commits_the_final_sample() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        let mut workspace = Workspace::analytical_default();
        let mut behaviour = DockBehaviour::default();
        let node = DockNodeId(1);
        let before_fraction = workspace.root.split_fraction(node).unwrap();
        let (_, initial) = dock_frame(&context, &mut workspace, &mut behaviour, Vec::new());
        let initial_rect = initial.splitter(node);
        let origin = initial_rect.center();

        let (pressed, _) = dock_frame(
            &context,
            &mut workspace,
            &mut behaviour,
            vec![
                egui::Event::PointerMoved(origin),
                pointer_button(origin, true),
            ],
        );
        assert!(pressed.is_none());

        let moved_pointer = origin - egui::vec2(80.0, 0.0);
        let (moving, moved) = dock_frame(
            &context,
            &mut workspace,
            &mut behaviour,
            vec![egui::Event::PointerMoved(moved_pointer)],
        );
        assert!(moving.is_none());
        assert!((moved.splitter(node).center().x - (origin.x - 80.0)).abs() < 0.001);

        let (idle, retained) = dock_frame(&context, &mut workspace, &mut behaviour, Vec::new());
        assert!(idle.is_none());
        assert_eq!(retained.splitter(node), moved.splitter(node));

        let released_pointer = origin - egui::vec2(100.0, 0.0);
        let (command, released) = dock_frame(
            &context,
            &mut workspace,
            &mut behaviour,
            vec![
                egui::Event::PointerMoved(released_pointer),
                pointer_button(released_pointer, false),
            ],
        );
        assert!((released.splitter(node).center().x - (origin.x - 100.0)).abs() < 0.001);
        let command = command.expect("split resize is committed on release");
        let Command::ResizeSplit {
            node: changed_node,
            before,
            after,
        } = command.clone()
        else {
            panic!("unexpected splitter command");
        };
        assert_eq!(changed_node, node);
        assert_eq!(before, before_fraction);
        assert!((after - (before_fraction - 100.0 / 800.0)).abs() < f32::EPSILON);

        let mut history = CommandHistory::default();
        let mut document = Document::default();
        let mut session = Session::default();
        history.execute(command, &mut document, &mut session, &mut workspace);
        assert_eq!(workspace.root.split_fraction(node), Some(after));
        assert_eq!(history.undo_len(), 1);
        assert!(history.undo(&mut document, &mut session, &mut workspace));
        assert_eq!(workspace.root.split_fraction(node), Some(before_fraction));
    }

    #[test]
    fn splitter_identity_survives_a_changing_parent_ui_id() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        let mut workspace = Workspace::analytical_default();
        let mut behaviour = DockBehaviour::default();
        let node = DockNodeId(1);
        let (_, initial) = dock_frame_with_parent_id(
            &context,
            &mut workspace,
            &mut behaviour,
            Vec::new(),
            "parent-a",
        );
        let origin = initial.splitter(node).center();

        let _ = dock_frame_with_parent_id(
            &context,
            &mut workspace,
            &mut behaviour,
            vec![
                egui::Event::PointerMoved(origin),
                pointer_button(origin, true),
            ],
            "parent-b",
        );
        let (_, moved) = dock_frame_with_parent_id(
            &context,
            &mut workspace,
            &mut behaviour,
            vec![egui::Event::PointerMoved(origin - egui::vec2(40.0, 0.0))],
            "parent-a",
        );
        assert!((moved.splitter(node).center().x - (origin.x - 40.0)).abs() < 0.001);

        let (command, _) = dock_frame_with_parent_id(
            &context,
            &mut workspace,
            &mut behaviour,
            vec![pointer_button(origin - egui::vec2(40.0, 0.0), false)],
            "parent-b",
        );
        assert!(matches!(
            command,
            Some(Command::ResizeSplit {
                node: DockNodeId(1),
                ..
            })
        ));
    }

    #[test]
    fn splitter_drag_returning_to_origin_emits_no_command() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        let mut workspace = Workspace::analytical_default();
        let mut behaviour = DockBehaviour::default();
        let node = DockNodeId(1);
        let (_, initial) = dock_frame(&context, &mut workspace, &mut behaviour, Vec::new());
        let initial_rect = initial.splitter(node);
        let origin = initial_rect.center();

        let _ = dock_frame(
            &context,
            &mut workspace,
            &mut behaviour,
            vec![
                egui::Event::PointerMoved(origin),
                pointer_button(origin, true),
            ],
        );
        let (_, moved) = dock_frame(
            &context,
            &mut workspace,
            &mut behaviour,
            vec![egui::Event::PointerMoved(origin - egui::vec2(60.0, 0.0))],
        );
        assert_eq!(moved.splitter(node).center().x, origin.x - 60.0);
        let (command, released) = dock_frame(
            &context,
            &mut workspace,
            &mut behaviour,
            vec![
                egui::Event::PointerMoved(origin),
                pointer_button(origin, false),
            ],
        );

        assert!(command.is_none());
        assert_eq!(released.splitter(node), initial_rect);
        assert_eq!(workspace.root.split_fraction(node), Some(0.72));
        assert!(!behaviour.interaction_active());
    }

    #[test]
    fn pane_interactions_are_clipped_before_they_can_steal_an_adjacent_splitter() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        let mut workspace = Workspace::analytical_default();
        workspace.activate(PaneId(6));
        let mut behaviour = DockBehaviour::default();
        let node = DockNodeId(1);
        let greedy = || GeometryPresenter {
            greedy_pane: Some(PaneId(6)),
            ..Default::default()
        };
        let (_, initial) = dock_frame_with_presenter(
            &context,
            &mut workspace,
            &mut behaviour,
            Vec::new(),
            greedy(),
        );
        let origin = initial.splitter(node).center();

        let _ = dock_frame_with_presenter(
            &context,
            &mut workspace,
            &mut behaviour,
            vec![
                egui::Event::PointerMoved(origin),
                pointer_button(origin, true),
            ],
            greedy(),
        );
        let (_, moved) = dock_frame_with_presenter(
            &context,
            &mut workspace,
            &mut behaviour,
            vec![egui::Event::PointerMoved(origin - egui::vec2(40.0, 0.0))],
            greedy(),
        );

        assert!((moved.splitter(node).center().x - (origin.x - 40.0)).abs() < 0.001);
    }
}
