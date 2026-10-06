//! Lab bindings retain domain validation and image commands in the application.
use super::*;
use polyorama_ui_egui::*;

pub(crate) fn inspection_bindings_for_state(
    session: &Session,
    history: &CommandHistory,
    workspace: &Workspace,
) -> Vec<InspectionBinding<LabAction>> {
    LabAction::ALL
        .into_iter()
        .flat_map(|action| {
            let panes: Vec<_> = if action.specification().scope == ActionScope::Application {
                vec![None]
            } else {
                (1..=8).map(|p| Some(PaneId(p))).collect()
            };
            panes.into_iter().map(move |pane| {
                let target = ActionTarget { action, pane };
                let camera = session.cameras.iter().find(|s| Some(s.pane) == pane);
                let context = ActionContext {
                    undo_depth: history.undo_len(),
                    redo_depth: history.redo_len(),
                    active_pane: workspace.active_pane,
                    target_pane: pane,
                    selected_annotation: session.selected_annotation,
                    selected_result: session.selected_result,
                    polygon_vertices: crate::actions::polygon_vertex_count(
                        session.gesture.as_ref(),
                    ),
                };
                let meaningful = match action {
                    LabAction::FitView | LabAction::LinkViews => {
                        serde_json::to_string(&camera).unwrap()
                    }
                    LabAction::ToggleDiagnostics => workspace
                        .closed_optional_panes
                        .contains(&DIAGNOSTICS_PANE)
                        .to_string(),
                    _ => "stable".into(),
                };
                let binding = InspectionBinding::new(
                    target,
                    availability(action, context),
                    matches!(
                        action,
                        LabAction::FitView | LabAction::LinkViews | LabAction::ToggleDiagnostics
                    ),
                    meaningful,
                );
                match pane {
                    Some(pane) => binding.with_domain(DomainReference::Pane(pane)),
                    None => binding,
                }
            })
        })
        .collect()
}

impl AnalyticalWorkspaceApp {
    #[cfg(not(target_arch = "wasm32"))]
    fn new_inspection() -> Inspection {
        Inspection::with_build_identity(
            APPLICATION_NAME,
            BuildIdentity {
                version: env!("CARGO_PKG_VERSION").into(),
                source_revision: option_env!("POLYORAMA_BUILD_REVISION").map(str::to_owned),
            },
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn start_inspection(&mut self, context: &egui::Context) {
        let inspection = Self::new_inspection();
        let wake_context = context.clone();
        match inspection.start_native(move || wake_context.request_repaint()) {
            Ok(Some(host)) => {
                inspection.install_completion_hook(context);
                self.inspection = Some(inspection);
                self._inspection_host = Some(host);
            }
            Ok(None) => {}
            Err(error) => self.status = format!("Automation could not start: {error}"),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn enable_inspection(&mut self, instance: &str) -> Result<(), String> {
        if self.inspection.is_none() {
            let service = Inspection::with_instance(
                APPLICATION_NAME,
                BuildIdentity {
                    version: env!("CARGO_PKG_VERSION").into(),
                    source_revision: option_env!("POLYORAMA_BUILD_REVISION").map(str::to_owned),
                },
                instance,
            )
            .map_err(|error| error.to_string())?;
            service.install_completion_hook(&self.egui_context);
            self.inspection = Some(service);
            let context = self.egui_context.clone();
            self.request_repaint(&context, RepaintReason::Interaction);
        }
        Ok(())
    }

    fn inspection_bindings(&self) -> Vec<InspectionBinding<LabAction>> {
        inspection_bindings_for_state(&self.session, &self.history, &self.workspace)
    }

    fn inspection_action(
        &mut self,
        target: ActionTarget<LabAction>,
        context: &egui::Context,
    ) -> Result<(), InspectionError> {
        let pane = target.pane;
        match target.action {
            LabAction::FitView => {
                let pane =
                    pane.ok_or_else(|| InspectionError::validation("An image pane is required"))?;
                let rect = self
                    .last_ui_snapshot
                    .node(&SemanticUiId::viewport(pane))
                    .map(|n| n.rect)
                    .filter(|r| r.is_positive())
                    .ok_or_else(|| {
                        InspectionError::validation("The image viewport is unavailable")
                    })?;
                self.execute_image_intent(crate::actions::fit_view_intent(pane, rect))
                    .map_err(InspectionError::validation)?;
            }
            LabAction::LinkViews => {
                let pane =
                    pane.ok_or_else(|| InspectionError::validation("An image pane is required"))?;
                let camera = self
                    .session
                    .cameras
                    .iter()
                    .find(|s| s.pane == pane)
                    .ok_or_else(|| InspectionError::validation("The camera is unavailable"))?;
                self.execute_image_intent(ImageIntent::SetCameraLink {
                    pane,
                    link: camera.link.is_none().then_some(LinkGroupId(1)),
                })
                .map_err(InspectionError::validation)?;
            }
            LabAction::ToggleDiagnostics => {
                toggle_diagnostics(&mut self.workspace);
            }
            _ => {
                return Err(InspectionError::validation(
                    "No semantic binding for this action",
                ));
            }
        }
        self.request_repaint(context, RepaintReason::Command);
        Ok(())
    }

    pub(super) fn drain_inspection(&mut self, context: &egui::Context) {
        let Some(service) = self.inspection.clone() else {
            return;
        };
        while service.drain(&self.inspection_bindings(), |target| {
            self.inspection_action(target, context)
        }) {}
    }

    #[cfg(target_arch = "wasm32")]
    pub fn inspection_request(&mut self, json: &str) -> Result<String, String> {
        let service = self.inspection.clone().ok_or("Automation is disabled")?;
        if json.len() > INSPECTION_REQUEST_BYTES {
            return Ok(service.handle_json(json));
        }
        let request: InspectionRequest = match serde_json::from_str(json) {
            Ok(request) => request,
            Err(_) => return Ok(service.handle_json(json)),
        };
        let context = self.egui_context.clone();
        let response = service.dispatch(request, &self.inspection_bindings(), |target| {
            self.inspection_action(target, &context)
        });
        serde_json::to_string(&response).map_err(|e| e.to_string())
    }

    pub(super) fn publish_inspection(&self, context: &egui::Context) {
        let Some(service) = &self.inspection else {
            return;
        };
        if context.will_discard() {
            return;
        }
        let mut facts = BTreeMap::from([
            (
                "undo_entries".into(),
                (self.history.undo_len() as u64).into(),
            ),
            (
                "redo_entries".into(),
                (self.history.redo_len() as u64).into(),
            ),
            (
                "diagnostics_open".into(),
                (!self
                    .workspace
                    .closed_optional_panes
                    .contains(&DIAGNOSTICS_PANE))
                .into(),
            ),
            ("status".into(), self.status.clone().into()),
        ]);
        for camera in &self.session.cameras {
            facts.insert(
                format!("camera.{}.scale", camera.pane.0),
                camera.camera.pixels_per_screen_point.to_string().into(),
            );
            facts.insert(
                format!("camera.{}.centre_x", camera.pane.0),
                camera.camera.centre.x.to_string().into(),
            );
            facts.insert(
                format!("camera.{}.centre_y", camera.pane.0),
                camera.camera.centre.y.to_string().into(),
            );
            facts.insert(
                format!("camera.{}.linked", camera.pane.0),
                camera.link.is_some().into(),
            );
        }
        let metadata = CollectionMetadata {
            collections: vec![
                CollectionObservation {
                    id: "results".into(),
                    total: Some(1_000_000),
                    observed: self.last_ui_snapshot.by_role(UiRole::ResultRow).count(),
                    offset: None,
                    virtualised: true,
                    complete: false,
                },
                CollectionObservation {
                    id: "thumbnails".into(),
                    total: Some(100_000),
                    observed: self.last_ui_snapshot.by_role(UiRole::ThumbnailCell).count(),
                    offset: None,
                    virtualised: true,
                    complete: false,
                },
            ],
            ..Default::default()
        };
        if let Err(error) = service.stage_completed(
            context,
            self.last_ui_snapshot.clone(),
            &self.inspection_bindings(),
            metadata,
            facts,
        ) {
            tracing::error!(?error, "automation publication failed");
        }
    }
}
