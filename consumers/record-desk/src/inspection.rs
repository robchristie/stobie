//! Application-owned bindings; the public framework owns the transport contract.
use super::*;
use std::collections::BTreeMap;

impl RecordDeskApp {
    #[cfg(not(target_arch = "wasm32"))]
    fn new_inspection() -> Inspection {
        Inspection::with_build_identity(
            "Record Desk",
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
            Err(error) => self.failure(format!("Automation could not start: {error}")),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn enable_inspection(&mut self, instance: &str) -> Result<(), String> {
        if self.inspection.is_none() {
            let service = Inspection::with_instance(
                "Record Desk",
                BuildIdentity {
                    version: env!("CARGO_PKG_VERSION").into(),
                    source_revision: option_env!("POLYORAMA_BUILD_REVISION").map(str::to_owned),
                },
                instance,
            )
            .map_err(|error| error.to_string())?;
            service.install_completion_hook(&self.inspection_context);
            self.inspection = Some(service);
            self.inspection_context.request_repaint();
        }
        Ok(())
    }

    fn inspection_bindings(&self, snapshot: &UiSnapshot) -> Vec<InspectionBinding<Action>> {
        Action::ALL
            .into_iter()
            .flat_map(|action| {
                let invocable = matches!(
                    action,
                    Action::Apply
                        | Action::Cancel
                        | Action::Undo
                        | Action::Redo
                        | Action::Search
                        | Action::ResetFilters
                        | Action::Arrange
                );
                // Guard assumptions that affect meaning, not focus or repaint counters.
                let meaning = match action {
                    Action::Apply | Action::Cancel => format!(
                        "{}:draft:{}:history:{}",
                        self.desk_epoch,
                        self.desk.draft_revision(),
                        self.desk.history_revision()
                    ),
                    Action::Undo | Action::Redo => format!(
                        "{}:history:{}",
                        self.desk_epoch,
                        self.desk.history_revision()
                    ),
                    Action::ResetFilters => format!(
                        "{}:filters:{}",
                        self.desk_epoch,
                        self.desk.filter_revision()
                    ),
                    Action::Arrange => match &self.workspace.root {
                        polyorama_core::DockNode::Split { axis, .. } => format!("{axis:?}"),
                        _ => "unsplit".into(),
                    },
                    _ => "stable".into(),
                };
                let binding = InspectionBinding::new(
                    ActionTarget::application(action),
                    self.availability(action),
                    invocable,
                    meaning,
                );
                let nodes: Vec<_> = snapshot.by_action(action).collect();
                if nodes.is_empty() {
                    vec![binding]
                } else {
                    nodes
                        .into_iter()
                        .map(|node| {
                            let mut bound = binding.clone().with_id(node.id.clone());
                            if let Some(domain) = &node.domain_reference {
                                bound = bound.with_domain(domain.clone());
                            }
                            bound
                        })
                        .collect()
                }
            })
            .collect()
    }

    pub(super) fn drain_inspection(&mut self, context: &egui::Context) {
        let Some(service) = self.inspection.clone() else {
            return;
        };
        while service.drain(&self.inspection_bindings(&self.snapshot.ui), |target| {
            self.action(target.action)
                .map_err(InspectionError::validation)?;
            context.request_repaint();
            Ok(())
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
        let context = self.inspection_context.clone();
        let response = service.dispatch(
            request,
            &self.inspection_bindings(&self.snapshot.ui),
            |target| {
                self.action(target.action)
                    .map_err(InspectionError::validation)?;
                context.request_repaint();
                Ok(())
            },
        );
        serde_json::to_string(&response).map_err(|e| e.to_string())
    }

    pub(super) fn publish_inspection(&self, context: &egui::Context, snapshot: &UiSnapshot) {
        let Some(service) = &self.inspection else {
            return;
        };
        if context.will_discard() {
            return;
        }
        let facts = BTreeMap::from([
            ("undo_entries".into(), (self.desk.undo_len() as u64).into()),
            ("redo_entries".into(), (self.desk.redo_len() as u64).into()),
            ("draft_dirty".into(), self.desk.is_dirty().into()),
            ("status".into(), self.message.clone().into()),
            ("error".into(), self.error.into()),
            (
                "selected_record".into(),
                self.desk
                    .selected()
                    .map_or(String::new(), |id| id.0.to_string())
                    .into(),
            ),
            (
                "draft_title".into(),
                self.desk
                    .draft()
                    .map_or(String::new(), |r| r.title.clone())
                    .into(),
            ),
            (
                "layout_axis".into(),
                match &self.workspace.root {
                    DockNode::Split { axis, .. } => format!("{axis:?}"),
                    _ => "unsplit".into(),
                }
                .into(),
            ),
        ]);
        let metadata = CollectionMetadata {
            collections: vec![CollectionObservation {
                id: "records".into(),
                total: Some(self.desk.records().len() as u64),
                observed: snapshot.by_role(UiRole::ResultRow).count(),
                offset: None,
                virtualised: false,
                complete: snapshot.by_role(UiRole::ResultRow).count() == self.desk.records().len(),
            }],
            ..Default::default()
        };
        if let Err(error) = service.stage_completed(
            context,
            snapshot.clone(),
            &self.inspection_bindings(snapshot),
            metadata,
            facts,
        ) {
            eprintln!("Automation publication failed: {error:?}");
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    fn present(app: &mut RecordDeskApp, context: &egui::Context) {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1080.0, 760.0),
                )),
                ..Default::default()
            },
            |ui| app.present(ui),
        );
        output.textures_delta.clear();
    }

    fn target(service: &Inspection, action: Action) -> ResolvedTarget {
        let reply = service.handle(InspectionRequest {
            version: 1,
            request_id: "discover".into(),
            instance: Some(service.instance()),
            operation: InspectionOperation::Discover {
                selector: InspectionSelector {
                    capability: Some(action.stable_id().into()),
                    ..Default::default()
                },
                limit: 1,
                cursor: None,
            },
        });
        let Some(InspectionResult::Discover { capabilities, .. }) = reply.result else {
            panic!("discovery");
        };
        capabilities[0].target.clone()
    }

    fn invoke(
        app: &mut RecordDeskApp,
        service: &Inspection,
        action: Action,
        expected: ResolvedTarget,
        id: &str,
    ) -> InvocationReceipt {
        let bindings = app.inspection_bindings(&app.snapshot.ui);
        let reply = service.dispatch(
            InspectionRequest {
                version: 1,
                request_id: id.into(),
                instance: Some(service.instance()),
                operation: InspectionOperation::Invoke {
                    selector: InspectionSelector {
                        capability: Some(action.stable_id().into()),
                        ..Default::default()
                    },
                    expected,
                    arguments: serde_json::Value::Null,
                },
            },
            &bindings,
            |target| {
                app.action(target.action)
                    .map_err(InspectionError::validation)
            },
        );
        let Some(InspectionResult::Receipt { receipt }) = reply.result else {
            panic!("receipt");
        };
        receipt
    }

    #[test]
    fn semantic_guards_ignore_focus_and_layout_but_track_draft_and_history() {
        let directory = tempfile::tempdir().unwrap();
        let context = egui::Context::default();
        let mut app = RecordDeskApp::with_store(
            &context,
            Store::at_path(directory.path().join("records.json")),
        );
        let service = Inspection::new("meaning-test");
        service.install_completion_hook(&context);
        app.inspection = Some(service.clone());
        let mut draft = app.desk.draft().unwrap().clone();
        draft.title = "One valid edit".into();
        app.desk.edit_draft(draft.clone()).unwrap();
        present(&mut app, &context);
        let apply = target(&service, Action::Apply);
        app.action(Action::Search).unwrap();
        polyorama_core::WorkspaceResize {
            node: polyorama_core::DockNodeId(1),
            before: match &app.workspace.root {
                DockNode::Split { fraction, .. } => *fraction,
                _ => unreachable!(),
            },
            after: 0.45,
        }
        .apply(&mut app.workspace)
        .unwrap();
        present(&mut app, &context);
        assert_eq!(
            invoke(&mut app, &service, Action::Apply, apply, "apply").state,
            ReceiptState::Completed
        );
        present(&mut app, &context);
        let undo = target(&service, Action::Undo);
        app.action(Action::Search).unwrap();
        polyorama_core::WorkspaceResize {
            node: polyorama_core::DockNodeId(1),
            before: 0.45,
            after: 0.5,
        }
        .apply(&mut app.workspace)
        .unwrap();
        present(&mut app, &context);
        assert_eq!(
            invoke(&mut app, &service, Action::Undo, undo.clone(), "undo").state,
            ReceiptState::Completed
        );
        present(&mut app, &context);
        assert_eq!(
            invoke(&mut app, &service, Action::Undo, undo, "old-undo")
                .error
                .unwrap()
                .code,
            InspectionErrorCode::StaleTarget
        );
        app.desk.edit_draft(draft.clone()).unwrap();
        present(&mut app, &context);
        let old_apply = target(&service, Action::Apply);
        draft.title = "A different edit".into();
        app.desk.edit_draft(draft).unwrap();
        present(&mut app, &context);
        assert_eq!(
            invoke(&mut app, &service, Action::Apply, old_apply, "old-apply")
                .error
                .unwrap()
                .code,
            InspectionErrorCode::StaleTarget
        );
        assert_eq!(app.desk.undo_len(), 0);
    }

    #[test]
    fn draft_field_capabilities_are_hidden_without_a_selected_record() {
        let directory = tempfile::tempdir().unwrap();
        let context = egui::Context::default();
        let mut app = RecordDeskApp::with_store(
            &context,
            Store::at_path(directory.path().join("records.json")),
        );
        app.desk = Desk::new(Vec::new()).unwrap();
        assert_eq!(
            app.availability(Action::ToggleReviewed),
            Availability::Hidden
        );
        assert_eq!(app.availability(Action::EditCategory), Availability::Hidden);
        assert!(!app.availability(Action::Apply).enabled());
    }

    #[test]
    fn queued_arrange_rechecks_the_current_narrow_window_policy() {
        let directory = tempfile::tempdir().unwrap();
        let context = egui::Context::default();
        let mut app = RecordDeskApp::with_store(
            &context,
            Store::at_path(directory.path().join("records.json")),
        );
        let service = Inspection::new("narrow-policy-test");
        service.install_completion_hook(&context);
        app.inspection = Some(service.clone());
        let present = |app: &mut RecordDeskApp, width| {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 844.0),
                    )),
                    ..Default::default()
                },
                |ui| app.present(ui),
            );
            output.textures_delta.clear();
        };
        present(&mut app, 1080.0);
        let selector = InspectionSelector {
            capability: Some(Action::Arrange.stable_id().into()),
            ..Default::default()
        };
        let reply = service.handle(InspectionRequest {
            version: 1,
            request_id: "discover".into(),
            instance: Some(service.instance()),
            operation: InspectionOperation::Discover {
                selector: selector.clone(),
                limit: 1,
                cursor: None,
            },
        });
        let Some(InspectionResult::Discover { capabilities, .. }) = reply.result else {
            panic!("discovery");
        };
        let message = app.message.clone();
        service.handle(InspectionRequest {
            version: 1,
            request_id: "arrange".into(),
            instance: Some(service.instance()),
            operation: InspectionOperation::Invoke {
                selector,
                expected: capabilities[0].target.clone(),
                arguments: serde_json::Value::Null,
            },
        });
        present(&mut app, 390.0);
        let reply = service.handle(InspectionRequest {
            version: 1,
            request_id: "receipt".into(),
            instance: Some(service.instance()),
            operation: InspectionOperation::Receipt {
                request_id: "arrange".into(),
            },
        });
        let Some(InspectionResult::Receipt { receipt }) = reply.result else {
            panic!("receipt");
        };
        assert_eq!(receipt.state, ReceiptState::Rejected);
        assert!(matches!(
            receipt.error.unwrap().code,
            InspectionErrorCode::StaleTarget | InspectionErrorCode::Unavailable
        ));
        assert_eq!(app.message, message);
        assert_eq!(app.desk.undo_len(), 0);
        assert!(matches!(
            app.workspace.root,
            DockNode::Split {
                axis: SplitAxis::Vertical,
                ..
            }
        ));
    }
}
