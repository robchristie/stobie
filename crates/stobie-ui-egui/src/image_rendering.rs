//! GPU scalar-image presentation through egui-wgpu paint callbacks, behind
//! the `image-rendering` feature.

use std::sync::{Arc, RwLock};

use egui::{Rect, Ui};
use stobie_core::PaneId;
use stobie_render_wgpu::{ImageRenderRequest, PixelRect, RenderPlan, ScalarRenderer};

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
/// Opaque staged callback target. Collect one per image request, in identical
/// pane order, and publish with [`submit_render_plan`] before callback preparation.
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
/// Structural publication failure. Every staged image target is disabled on
/// failure; repair the complete plan/target pairing before rendering again.
pub enum RenderPlanSubmissionError {
    /// The complete plan and staged-target lists have different lengths.
    CountMismatch { requests: usize, targets: usize },
    /// The request and target at this position name different panes.
    PaneMismatch {
        index: usize,
        request: PaneId,
        target: PaneId,
    },
    /// A pane occurs more than once in the complete plan/target pairing.
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
    use super::*;

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
}
