#![cfg(test)]
//! Plain egui widgets observed through `PresentationContext::observed`: one
//! call publishes each with its role, name and state, its AccessKit node
//! agrees, the browser's ARIA layer agrees, and a plain text edit observed
//! this way accepts `stobie set-text`.

use egui::Vec2;
use egui_kittest::Harness;
use serde::Serialize;

use super::*;
use crate::{
    Action, InspectionOperation, InspectionResult, InspectionSelector, PresentationScope,
    ReceiptState,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "plain")]
enum Act {
    #[action(label = "Nothing", description = "Do nothing")]
    Nothing,
}

enum Intent {
    Wrap(bool),
    Title(String),
}

/// Settings presented with plain egui widgets.
#[derive(Default)]
struct Plain {
    wrap: bool,
    title: String,
    /// Present the checkbox disabled.
    locked: bool,
}

const WRAP: &str = "plain.wrap";
const UPLOAD: &str = "plain.upload";
const RESULTS: &str = "plain.results";
const LOGO: &str = "plain.logo";
const TITLE: &str = "plain.title";

impl Application for Plain {
    type Action = Act;
    type Intent = Intent;
    type UiState = Option<egui::Response>;

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("Plain")
    }

    fn availability(&self, _: ActionTarget<Act>) -> Availability {
        Availability::Enabled
    }

    fn report_error(&mut self, message: String) {
        panic!("{message}");
    }

    fn ui(
        &self,
        ui: &mut Ui,
        view: &mut Option<egui::Response>,
        frame: &mut Frame<'_, Act, Intent>,
    ) {
        let mut p = frame.presentation(ui, PresentationScope::new("plain"), SemanticUiId::root());
        let mut wrap = self.wrap;
        let checkbox = p.observed(
            ui,
            SemanticUiId::new(WRAP),
            UiRole::Checkbox,
            "Wrap lines",
            |ui| ui.add_enabled(!self.locked, egui::Checkbox::new(&mut wrap, "Wrap lines")),
        );
        if wrap != self.wrap {
            frame.emit(Intent::Wrap(wrap));
        }
        *view = Some(checkbox);
        p.observed(
            ui,
            SemanticUiId::new(UPLOAD),
            UiRole::Progress,
            "Upload",
            |ui| ui.add(egui::ProgressBar::new(0.4)),
        );
        p.observed(
            ui,
            SemanticUiId::new(RESULTS),
            UiRole::Table,
            "Results",
            |ui| {
                ui.scope(|ui| {
                    egui::Grid::new("results").show(ui, |ui| {
                        ui.label("Name");
                        ui.label("Size");
                        ui.end_row();
                    });
                })
                .response
            },
        );
        p.observed(ui, SemanticUiId::new(LOGO), UiRole::Image, "Logo", |ui| {
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(24.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 4.0, egui::Color32::GRAY);
            response
        });
        let mut title = self.title.clone();
        p.observed(
            ui,
            SemanticUiId::new(TITLE),
            UiRole::TextInput,
            "Title",
            |ui| ui.text_edit_singleline(&mut title),
        );
        if title != self.title {
            frame.emit(Intent::Title(title));
        }
    }

    fn apply(&mut self, intent: Intent, _: &mut Option<egui::Response>) -> Result<bool, String> {
        match intent {
            Intent::Wrap(wrap) => self.wrap = wrap,
            Intent::Title(title) => self.title = title,
        }
        Ok(true)
    }

    fn perform(
        &mut self,
        _: ActionTarget<Act>,
        _: &mut Option<egui::Response>,
    ) -> Result<bool, String> {
        Ok(true)
    }
}

#[derive(Default)]
struct Probe {
    host: Option<AppHost<Plain>>,
}

fn harness(plain: Plain) -> Harness<'static, Probe> {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(480.0, 400.0))
        .build_ui_state(
            |ui, probe: &mut Probe| {
                if let Some(host) = &mut probe.host {
                    host.present(ui);
                }
            },
            Probe::default(),
        );
    crate::install_typography_fonts(&harness.ctx);
    let context = harness.ctx.clone();
    let mut host = AppHost::new(&context, plain);
    host.enable_inspection(Inspection::new("plain-test"));
    harness.state_mut().host = Some(host);
    harness.run();
    harness
}

fn host<'a>(harness: &'a Harness<'_, Probe>) -> &'a AppHost<Plain> {
    harness.state().host.as_ref().unwrap()
}

fn node(harness: &Harness<'_, Probe>, id: &str) -> UiNode {
    host(harness)
        .ui_snapshot()
        .node(&SemanticUiId::new(id))
        .unwrap_or_else(|| panic!("{id} is observed"))
        .clone()
}

fn accesskit_update(harness: &Harness<'_, Probe>) -> egui::accesskit::TreeUpdate {
    fn collect(
        node: egui_kittest::kittest::AccessKitNode<'_>,
        update: &mut egui::accesskit::TreeUpdate,
    ) {
        update.nodes.push((node.locate().0, node.data().clone()));
        for child in node.children() {
            collect(child, update);
        }
    }
    let root = harness.kittest_state().root();
    let mut update = egui::accesskit::TreeUpdate {
        nodes: Vec::new(),
        tree: Some(egui::accesskit::Tree::new(root.locate().0)),
        tree_id: egui::accesskit::TreeId::ROOT,
        focus: root.locate().0,
    };
    collect(root, &mut update);
    update
}

/// The snapshot, AccessKit and the ARIA layer all agree.
fn assert_agrees(harness: &Harness<'_, Probe>) {
    let snapshot = host(harness).ui_snapshot();
    assert!(
        snapshot.semantic_audit.is_empty(),
        "{:?}",
        snapshot.semantic_audit
    );
    assert!(snapshot.text_audit.is_empty(), "{:?}", snapshot.text_audit);
    let update = accesskit_update(harness);
    let mismatches = crate::audit_accesskit(snapshot, &update);
    assert!(mismatches.is_empty(), "{mismatches:?}");
    let disagreements = crate::agreement::semantic_agreement(snapshot, &update);
    assert!(disagreements.is_empty(), "{disagreements:?}");
}

fn click(harness: &mut Harness<'_, Probe>, at: egui::Pos2) {
    harness.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    harness.run();
}

#[test]
fn each_widget_is_observed_with_its_role_name_and_state() {
    let harness = harness(Plain::default());
    assert_agrees(&harness);
    for (id, role, name) in [
        (WRAP, UiRole::Checkbox, "Wrap lines"),
        (UPLOAD, UiRole::Progress, "Upload"),
        (RESULTS, UiRole::Table, "Results"),
        (LOGO, UiRole::Image, "Logo"),
        (TITLE, UiRole::TextInput, "Title"),
    ] {
        let node = node(&harness, id);
        assert_eq!((node.role, node.name.as_str()), (role, name), "{id}");
        assert_eq!(node.parent, Some(SemanticUiId::root()));
        assert!(node.enabled && node.rect.is_positive(), "{node:?}");
    }
    assert_eq!(node(&harness, WRAP).checked, Some(false));
    assert_eq!(node(&harness, UPLOAD).checked, None);
}

#[test]
fn a_checkbox_reports_its_state_and_whether_it_is_enabled() {
    let mut harness = harness(Plain::default());
    let checkbox = host(&harness).ui_state().clone().unwrap();
    click(&mut harness, checkbox.rect.center());
    harness.run();
    assert!(host(&harness).app().wrap);
    assert_eq!(node(&harness, WRAP).checked, Some(true));
    assert_agrees(&harness);

    harness.state_mut().host.as_mut().unwrap().app_mut().locked = true;
    harness.run();
    let wrap = node(&harness, WRAP);
    assert!(!wrap.enabled);
    assert_eq!(wrap.checked, Some(true));
    assert_agrees(&harness);
}

#[test]
fn agents_set_the_text_of_an_observed_text_edit() {
    let mut harness = harness(Plain::default());
    let id = SemanticUiId::new(TITLE);
    let selector = InspectionSelector {
        id: Some(id.clone()),
        ..Default::default()
    };
    let inspection = host(&harness).inspection().unwrap();
    let request = |request_id: &str, operation| {
        inspection.handle(InspectionRequest {
            version: 1,
            request_id: request_id.into(),
            instance: Some(inspection.instance()),
            operation,
        })
    };
    let reply = request(
        "discover",
        InspectionOperation::Discover {
            selector: selector.clone(),
            limit: 8,
            cursor: None,
        },
    );
    let Some(InspectionResult::Discover {
        mut capabilities, ..
    }) = reply.result
    else {
        panic!("discovery: {:?}", reply.error);
    };
    assert_eq!(capabilities.len(), 1, "{capabilities:?}");
    let expected = capabilities.remove(0).target;
    let reply = request(
        "set",
        InspectionOperation::Invoke {
            selector,
            expected,
            arguments: serde_json::json!({ "value": "Quarterly report" }),
        },
    );
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("receipt failed: {:?}", reply.error);
    };
    assert_eq!(receipt.state, ReceiptState::Queued, "{receipt:?}");
    harness.run();
    assert_eq!(host(&harness).app().title, "Quarterly report");
    assert_eq!(
        node(&harness, TITLE).value.as_deref(),
        Some("Quarterly report")
    );
}
