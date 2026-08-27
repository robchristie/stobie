//! Immediate-mode presentation and semantic pane interfaces.

use egui::{Pos2, Rect, Ui};
use workspace_core::{
    Command, DockDrop, DockNode, DockNodeId, LogicalPoint, PaneId, PhysicalPoint, SplitAxis,
    ViewportPoint, Workspace,
};
use workspace_render_wgpu::{ImageRenderRequest, PixelRect, ScalarRenderer};

const TAB_HEIGHT: f32 = 28.0;
const SPLITTER: f32 = 5.0;

#[derive(Default)]
pub struct DockBehaviour {
    pub dragging: Option<PaneId>,
    pending: Option<DockAction>,
    split_preview: Option<SplitPreview>,
}

#[derive(Clone, Copy)]
struct SplitPreview {
    node: DockNodeId,
    before: f32,
    after: f32,
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
    fn pane_ui(&mut self, ui: &mut Ui, pane: PaneId);
}

pub fn dock_workspace(
    ui: &mut Ui,
    workspace: &mut Workspace,
    behaviour: &mut DockBehaviour,
    presenter: &mut impl PanePresenter,
) -> Option<Command> {
    let rect = ui.available_rect_before_wrap();
    render_node(ui, &mut workspace.root, rect, behaviour, presenter);
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
            let (_, hit_rect, _) = split_rects(rect, horizontal, initial_fraction);
            let response = ui.interact(
                hit_rect,
                ui.id().with(("split", node.0)),
                egui::Sense::drag(),
            );
            if response.drag_started() {
                behaviour.split_preview = Some(SplitPreview {
                    node,
                    before: *fraction,
                    after: *fraction,
                });
            }
            if response.dragged() || response.drag_stopped() {
                let delta = if horizontal {
                    response.drag_delta().x
                } else {
                    response.drag_delta().y
                };
                let before = behaviour
                    .split_preview
                    .filter(|preview| preview.node == node)
                    .map_or(*fraction, |preview| preview.before);
                behaviour.split_preview = Some(SplitPreview {
                    node,
                    before,
                    after: ((before * length + delta) / length).clamp(0.1, 0.9),
                });
                ui.ctx().request_repaint();
            }
            let shown_fraction = behaviour
                .split_preview
                .filter(|preview| preview.node == node)
                .map_or(*fraction, |preview| preview.after);
            let (first_rect, split_rect, second_rect) =
                split_rects(rect, horizontal, shown_fraction);
            if response.drag_stopped()
                && let Some(preview) = behaviour
                    .split_preview
                    .take()
                    .filter(|preview| preview.node == node)
            {
                behaviour.pending = Some(DockAction::Resize {
                    node,
                    before: preview.before,
                    after: preview.after,
                });
            }
            ui.painter().rect_filled(
                split_rect,
                0.0,
                if response.hovered() {
                    ui.visuals().selection.bg_fill
                } else {
                    ui.visuals().widgets.noninteractive.bg_fill
                },
            );
            render_node(ui, first, first_rect, behaviour, presenter);
            render_node(ui, second, second_rect, behaviour, presenter);
        }
        DockNode::Tabs { tabs, active, .. } => {
            if tabs.is_empty() {
                ui.painter()
                    .rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
                return;
            }
            *active = (*active).min(tabs.len() - 1);
            let tab_rect = Rect::from_min_max(
                rect.min,
                Pos2::new(rect.right(), (rect.top() + TAB_HEIGHT).min(rect.bottom())),
            );
            let body = Rect::from_min_max(Pos2::new(rect.left(), tab_rect.bottom()), rect.max);
            ui.painter().rect_filled(rect, 0.0, ui.visuals().panel_fill);
            let mut x = tab_rect.left() + 4.0;
            for (index, pane) in tabs.iter().copied().enumerate() {
                let width = (presenter.title(pane).len() as f32 * 7.2 + 22.0).clamp(72.0, 180.0);
                let item_rect = Rect::from_min_size(
                    Pos2::new(x, tab_rect.top() + 3.0),
                    egui::vec2(width, TAB_HEIGHT - 4.0),
                );
                let response = ui.interact(
                    item_rect,
                    ui.id().with(("tab", pane.0)),
                    egui::Sense::click_and_drag(),
                );
                if response.clicked() {
                    behaviour.pending = Some(DockAction::Activate(pane));
                    *active = index;
                }
                if response.drag_started() {
                    behaviour.dragging = Some(pane);
                }
                let fill = if index == *active {
                    ui.visuals().extreme_bg_color
                } else if response.hovered() {
                    ui.visuals().widgets.hovered.bg_fill
                } else {
                    ui.visuals().widgets.inactive.bg_fill
                };
                ui.painter().rect_filled(item_rect, 4.0, fill);
                ui.painter().text(
                    item_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    presenter.title(pane),
                    egui::FontId::proportional(12.5),
                    ui.visuals().text_color(),
                );
                x += width + 3.0;
            }
            ui.painter().hline(
                tab_rect.x_range(),
                tab_rect.bottom(),
                ui.visuals().widgets.noninteractive.bg_stroke,
            );
            if let Some(target) = tabs.get(*active).copied() {
                let body_response = ui.interact(
                    body,
                    ui.id().with(("dock-body", target.0)),
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
                    |ui| presenter.pane_ui(ui, target),
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
    let first_length = (length * fraction - SPLITTER * 0.5).max(40.0);
    if horizontal {
        let cut = rect.left() + first_length;
        (
            Rect::from_min_max(rect.min, Pos2::new(cut, rect.bottom())),
            Rect::from_min_max(
                Pos2::new(cut, rect.top()),
                Pos2::new(cut + SPLITTER, rect.bottom()),
            ),
            Rect::from_min_max(Pos2::new(cut + SPLITTER, rect.top()), rect.max),
        )
    } else {
        let cut = rect.top() + first_length;
        (
            Rect::from_min_max(rect.min, Pos2::new(rect.right(), cut)),
            Rect::from_min_max(
                Pos2::new(rect.left(), cut),
                Pos2::new(rect.right(), cut + SPLITTER),
            ),
            Rect::from_min_max(Pos2::new(rect.left(), cut + SPLITTER), rect.max),
        )
    }
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
    pub request: ImageRenderRequest,
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
            renderer.prepare(device, queue, self.frame_number, &self.request);
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
        let viewport = info.viewport_in_pixels();
        let clip = info.clip_rect_in_pixels();
        renderer.paint(
            self.request.pane,
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

pub fn submit_scalar_callback(ui: &Ui, rect: Rect, frame_number: u64, request: ImageRenderRequest) {
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
        rect,
        ScalarPaintCallback {
            frame_number,
            request,
        },
    ));
}
