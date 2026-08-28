use super::*;

#[derive(Default)]
pub struct UiBehaviour {
    camera_drags: BTreeMap<PaneId, CameraDragSession>,
    camera_wheels: BTreeMap<CameraGestureKey, CameraGesture>,
    frame_camera_overrides: BTreeMap<PaneId, Camera>,
    pub(super) pointer_image: BTreeMap<PaneId, ImagePoint>,
}

struct CameraGesture {
    source_pane: PaneId,
    before: Vec<CameraState>,
    preview: Vec<CameraState>,
    last_input: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum CameraGestureKey {
    Pane(PaneId),
    Link(LinkGroupId),
}

pub(super) struct ImageFrame {
    pub(super) camera: Camera,
    pub(super) demands: Vec<TileDemand>,
}

pub(super) const WHEEL_GESTURE_IDLE: Duration = Duration::from_millis(140);

pub(super) fn drag_pointer_sample(response: &egui::Response) -> Option<ViewportPoint> {
    (response.dragged() || response.drag_stopped())
        .then(|| response.interact_pointer_pos())
        .flatten()
        .map(|pointer| ViewportPoint::new(pointer.x as f64, pointer.y as f64))
}

pub fn should_cancel_camera_drag(input: &egui::InputState) -> bool {
    !input.focused
        || input
            .events
            .iter()
            .any(|event| matches!(event, egui::Event::WindowFocused(false)))
        || (!input.pointer.primary_down()
            && input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::PointerGone)))
}

impl UiBehaviour {
    #[cfg(test)]
    pub(super) fn wheel_gesture_count(&self) -> usize {
        self.camera_wheels.len()
    }

    pub fn begin_frame(&mut self) {
        self.frame_camera_overrides.clear();
        let mut previews: Vec<_> = self
            .camera_drags
            .values()
            .flat_map(|gesture| gesture.preview().iter().cloned())
            .collect();
        previews.extend(
            self.camera_wheels
                .values()
                .flat_map(|gesture| gesture.preview.iter().cloned()),
        );
        self.expose_preview(&previews);
    }

    pub fn cancel_camera_drags(&mut self) -> bool {
        let cancelled = !self.camera_drags.is_empty();
        self.camera_drags.clear();
        self.frame_camera_overrides.clear();
        cancelled
    }

    fn start_camera_drag(
        &mut self,
        pane: PaneId,
        before: Vec<CameraState>,
        pointer_origin: Option<ViewportPoint>,
    ) {
        let drag = match pointer_origin {
            Some(origin) => CameraDragSession::new_at(pane, before, origin),
            None => CameraDragSession::new(pane, before),
        };
        if let Some(drag) = drag {
            self.camera_drags.insert(pane, drag);
        }
    }

    pub(super) fn camera(&self, pane: PaneId, committed: &[CameraState]) -> Camera {
        self.frame_camera_overrides
            .get(&pane)
            .copied()
            .or_else(|| {
                self.camera_drags
                    .values()
                    .find_map(|gesture| {
                        gesture
                            .preview()
                            .iter()
                            .find(|state| state.pane == pane)
                            .map(|state| state.camera)
                    })
                    .or_else(|| {
                        self.camera_wheels.values().find_map(|gesture| {
                            gesture
                                .preview
                                .iter()
                                .find(|state| state.pane == pane)
                                .map(|state| state.camera)
                        })
                    })
            })
            .or_else(|| {
                committed
                    .iter()
                    .find(|state| state.pane == pane)
                    .map(|state| state.camera)
            })
            .unwrap_or_default()
    }

    fn camera_states(&self, committed: &[CameraState]) -> Vec<CameraState> {
        let mut states = committed.to_vec();
        for state in &mut states {
            if let Some(camera) = self.frame_camera_overrides.get(&state.pane) {
                state.camera = *camera;
            }
        }
        states
    }

    pub(super) fn expose_preview(&mut self, preview: &[CameraState]) {
        self.frame_camera_overrides
            .extend(preview.iter().map(|state| (state.pane, state.camera)));
    }

    fn gesture_key(pane: PaneId, cameras: &[CameraState]) -> CameraGestureKey {
        cameras
            .iter()
            .find(|state| state.pane == pane)
            .and_then(|state| state.link)
            .map_or(CameraGestureKey::Pane(pane), CameraGestureKey::Link)
    }

    pub(super) fn update_wheel_gesture(
        &mut self,
        pane: PaneId,
        committed: &[CameraState],
        after: Camera,
        now: Instant,
    ) {
        let key = Self::gesture_key(pane, committed);
        let current = self.camera_wheels.get(&key).map_or_else(
            || self.camera_states(committed),
            |gesture| gesture.preview.clone(),
        );
        let changes = linked_camera_changes(&current, pane, after);
        let gesture = self
            .camera_wheels
            .entry(key)
            .or_insert_with(|| CameraGesture {
                source_pane: pane,
                before: current.clone(),
                preview: current.clone(),
                last_input: now,
            });
        gesture.preview = current;
        apply_camera_changes(&mut gesture.preview, &changes, true);
        gesture.last_input = now;
        let preview = gesture.preview.clone();
        self.expose_preview(&preview);
    }

    pub(super) fn finish_wheel_key(
        &mut self,
        key: CameraGestureKey,
        outputs: &mut FrameOutput,
    ) -> Option<Vec<CameraState>> {
        let gesture = self.camera_wheels.remove(&key)?;
        let after = gesture
            .preview
            .iter()
            .find(|state| state.pane == gesture.source_pane)
            .map(|state| state.camera)
            .unwrap_or_default();
        self.expose_preview(&gesture.preview);
        push_camera_changes(
            outputs,
            linked_camera_changes(&gesture.before, gesture.source_pane, after),
        );
        Some(gesture.preview)
    }

    pub fn finish_camera_gestures(&mut self, now: Instant, outputs: &mut FrameOutput) {
        let expired: Vec<_> = self
            .camera_wheels
            .iter()
            .filter_map(|(key, gesture)| {
                (now.duration_since(gesture.last_input) >= WHEEL_GESTURE_IDLE).then_some(*key)
            })
            .collect();
        for key in expired {
            self.finish_wheel_key(key, outputs);
        }
        if let Some(remaining) = self
            .camera_wheels
            .values()
            .map(|gesture| WHEEL_GESTURE_IDLE - now.duration_since(gesture.last_input))
            .min()
        {
            outputs.repaint_after = Some(
                outputs
                    .repaint_after
                    .map_or(remaining, |scheduled| scheduled.min(remaining)),
            );
        }
    }
}

impl PaneSurface<'_> {
    pub(super) fn handle_camera(
        &mut self,
        ui: &egui::Ui,
        pane: PaneId,
        rect: egui::Rect,
        response: &egui::Response,
    ) {
        let tool = self
            .active_tools
            .get(&pane)
            .copied()
            .unwrap_or(ActiveTool::Navigate);
        if tool != ActiveTool::Navigate {
            return;
        }
        let now = Instant::now();
        if response.drag_started() {
            let key = UiBehaviour::gesture_key(pane, self.cameras);
            let before = self
                .ui_behaviour
                .finish_wheel_key(key, self.outputs)
                .unwrap_or_else(|| self.ui_behaviour.camera_states(self.cameras));
            let pointer_origin = ui
                .input(|input| input.pointer.press_origin())
                .or_else(|| response.interact_pointer_pos())
                .map(|pointer| ViewportPoint::new(pointer.x as f64, pointer.y as f64));
            self.ui_behaviour
                .start_camera_drag(pane, before, pointer_origin);
        }
        if let Some(pointer) = drag_pointer_sample(response) {
            if let Some(drag) = self.ui_behaviour.camera_drags.get_mut(&pane) {
                let preview = drag.update_pointer(pointer).to_vec();
                self.ui_behaviour.expose_preview(&preview);
                self.outputs.interaction_active = true;
            }
        }
        if response.drag_stopped() {
            if let Some(drag) = self.ui_behaviour.camera_drags.remove(&pane) {
                self.ui_behaviour.expose_preview(drag.preview());
                push_camera_changes(self.outputs, drag.finish());
            }
        }
        if response.hovered() {
            let zoom_delta = ui.input(|input| input.smooth_scroll_delta.y);
            if zoom_delta.abs() > 0.01 {
                let key = UiBehaviour::gesture_key(pane, self.cameras);
                let current = self.ui_behaviour.camera_wheels.get(&key).map_or_else(
                    || self.ui_behaviour.camera_states(self.cameras),
                    |gesture| gesture.preview.clone(),
                );
                let before = current
                    .iter()
                    .find(|state| state.pane == pane)
                    .map(|state| state.camera)
                    .unwrap_or_default();
                let pointer = response.hover_pos().unwrap_or(rect.center());
                let local = ViewportPoint::new(
                    (pointer.x - rect.left()) as f64,
                    (pointer.y - rect.top()) as f64,
                );
                let mut after = before;
                after.zoom_around(
                    (-zoom_delta as f64 * 0.0025).exp(),
                    local,
                    ViewportPoint::new(rect.width() as f64, rect.height() as f64),
                );
                self.ui_behaviour
                    .update_wheel_gesture(pane, self.cameras, after, now);
                self.outputs.interaction_active = true;
                self.outputs.repaint_after = Some(
                    self.outputs
                        .repaint_after
                        .map_or(WHEEL_GESTURE_IDLE, |scheduled| {
                            scheduled.min(WHEEL_GESTURE_IDLE)
                        }),
                );
            }
        }
    }
}

fn push_camera_changes(outputs: &mut FrameOutput, mut changes: Vec<CameraChange>) {
    changes.retain(|change| change.before != change.after);
    if !changes.is_empty() {
        outputs.commands.push(Command::SetCameras { changes });
    }
}

pub(super) fn derive_image_frame(
    behaviour: &UiBehaviour,
    pane: PaneId,
    cameras: &[CameraState],
    viewport: (f64, f64),
    generation: u64,
) -> ImageFrame {
    let camera = behaviour.camera(pane, cameras);
    let demands = image_demands(camera, viewport, generation);
    ImageFrame { camera, demands }
}

pub(super) fn image_demands(
    camera: Camera,
    viewport: (f64, f64),
    generation: u64,
) -> Vec<TileDemand> {
    let mut demands = visible_tile_demands(camera, viewport, SourceId(1), generation, true);
    demands.push(TileDemand {
        key: TileKey {
            source: SourceId(1),
            level: PYRAMID_LEVELS - 1,
            x: 0,
            y: 0,
        },
        priority: DemandPriority::Visible,
        generation,
    });
    demands
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;

    #[test]
    fn drag_returning_to_origin_emits_no_command() {
        let committed = Session::default().cameras;
        let origin = ViewportPoint::new(10.0, 20.0);
        let mut behaviour = UiBehaviour::default();
        behaviour.start_camera_drag(PaneId(1), committed, Some(origin));
        let drag = behaviour.camera_drags.get_mut(&PaneId(1)).unwrap();
        drag.update_pointer(ViewportPoint::new(100.0, 70.0));
        drag.update_pointer(origin);
        let drag = behaviour.camera_drags.remove(&PaneId(1)).unwrap();
        let mut output = FrameOutput::default();

        push_camera_changes(&mut output, drag.finish());

        assert!(output.commands.is_empty());
    }

    #[test]
    fn focus_loss_and_terminal_pointer_gone_cancel_without_committing_preview() {
        fn cancellation_for(input: egui::RawInput) -> bool {
            let observed = Rc::new(RefCell::new(None));
            let output = observed.clone();
            let context = egui::Context::default();
            let mut full_output = context.run_ui(input, |ui| {
                *output.borrow_mut() = Some(ui.input(should_cancel_camera_drag));
            });
            full_output.textures_delta.clear();
            observed.borrow_mut().take().unwrap()
        }

        assert!(cancellation_for(egui::RawInput {
            focused: false,
            events: vec![egui::Event::WindowFocused(false)],
            ..Default::default()
        }));
        assert!(cancellation_for(egui::RawInput {
            focused: true,
            events: vec![egui::Event::PointerGone],
            ..Default::default()
        }));
        assert!(!cancellation_for(egui::RawInput {
            focused: true,
            events: vec![egui::Event::PointerButton {
                pos: egui::pos2(100.0, 70.0),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        }));

        let committed = Session::default().cameras;
        let mut behaviour = UiBehaviour::default();
        behaviour.start_camera_drag(
            PaneId(1),
            committed.clone(),
            Some(ViewportPoint::new(10.0, 20.0)),
        );
        let preview = behaviour
            .camera_drags
            .get_mut(&PaneId(1))
            .unwrap()
            .update_pointer(ViewportPoint::new(100.0, 70.0))
            .to_vec();
        behaviour.expose_preview(&preview);
        assert_ne!(behaviour.camera(PaneId(1), &committed), committed[0].camera);

        assert!(behaviour.cancel_camera_drags());
        assert_eq!(behaviour.camera(PaneId(1), &committed), committed[0].camera);
        assert!(!behaviour.cancel_camera_drags());
    }
}
