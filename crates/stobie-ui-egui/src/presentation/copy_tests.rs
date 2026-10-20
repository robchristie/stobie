//! The copy recipe's faces: AccessKit parity in each state and content
//! variant, a width that holds steady across states, the disabled reason,
//! and nothing at all while hidden.

use egui::Rect;

use super::*;
use crate::{
    DensityVariant, DesignTokens, PresentationScope, SemanticUiId, ThemeVariant, UiSnapshot,
    audit_accesskit, audit_text_layouts,
};

const CODE: &str = "QX7-SECRET-4410";

fn context() -> egui::Context {
    let context = egui::Context::default();
    crate::install_typography_fonts(&context);
    context.enable_accesskit();
    context
}

struct Presented {
    snapshot: UiSnapshot,
    update: egui::accesskit::TreeUpdate,
    rect: Rect,
    state: CopyState,
    published: String,
}

fn present(context: &egui::Context, spec: &CopySpec<'_>) -> Presented {
    let root = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 200.0));
    let mut result = None;
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(root),
            ..Default::default()
        },
        |ui| {
            let tokens = DesignTokens::resolve(ThemeVariant::Light, DensityVariant::Comfortable);
            let mut p = PresentationContext::new(
                ui,
                tokens,
                1.0,
                PresentationScope::new("invite"),
                SemanticUiId::root(),
            );
            let copied = p.copy(ui, "code", spec);
            let published = p.finish(ui);
            assert!(
                audit_text_layouts(&published.text_layouts).is_empty(),
                "{:?}",
                audit_text_layouts(&published.text_layouts)
            );
            let text = format!("{:?}{:?}", published.semantic_nodes, published.text_layouts);
            let mut snapshot = UiSnapshot::default();
            snapshot.nodes.push(crate::UiNode::container(
                SemanticUiId::root(),
                None,
                UiRole::Application,
                root.into(),
            ));
            snapshot.nodes.extend(published.semantic_nodes);
            result = Some((snapshot, copied, text));
        },
    );
    output.textures_delta.clear();
    let (snapshot, copied, published) = result.unwrap();
    Presented {
        snapshot,
        update: output.platform_output.accesskit_update.take().unwrap(),
        rect: copied.response.rect,
        state: copied.state,
        published,
    }
}

fn widget() -> egui::Id {
    PresentationScope::new("invite").instance("code").egui_id()
}

fn status() -> SemanticUiId {
    PresentationScope::new("invite")
        .instance("code")
        .child("status")
        .semantic_id()
}

/// Present `spec` in each state, checking parity, and return the rects.
fn each_state(spec: &CopySpec<'_>) -> Vec<(CopyState, Rect)> {
    let context = context();
    let mut rects = Vec::new();
    for state in [CopyState::Ready, CopyState::Copied, CopyState::Failed] {
        // A first pass records the text; then the state is set as an
        // outcome would set it.
        present(&context, spec);
        clipboard::update(&context, widget(), |control| control.state = state);
        let presented = present(&context, spec);
        assert_eq!(presented.state, state);
        assert!(
            presented.snapshot.audit().is_empty(),
            "{:?}",
            presented.snapshot.audit()
        );
        assert!(
            audit_accesskit(&presented.snapshot, &presented.update).is_empty(),
            "{state:?}: {:?}",
            audit_accesskit(&presented.snapshot, &presented.update)
        );
        assert!(!presented.published.contains(CODE));
        let message = match state {
            CopyState::Copied => spec.confirmation,
            CopyState::Failed => spec.failure,
            _ => "",
        };
        let node = presented.snapshot.node(&status());
        assert_eq!(
            node.map(|node| node.name.as_str()),
            (!message.is_empty()).then_some(message)
        );
        // The live region is in the tree, empty, before any outcome.
        let region = presented
            .update
            .nodes
            .iter()
            .find(|(_, node)| node.author_id() == Some(status().0.as_str()))
            .map(|(_, node)| node)
            .expect("the status region is always present");
        assert_eq!(region.role(), egui::accesskit::Role::Status);
        assert_eq!(region.live(), Some(egui::accesskit::Live::Polite));
        assert_eq!(region.label().unwrap_or_default(), message);
        rects.push((state, presented.rect));
        clipboard::update(&context, widget(), |control| {
            control.state = CopyState::Ready
        });
    }
    rects
}

#[test]
fn each_face_matches_assistive_technology_and_keeps_its_width() {
    for content in [
        CopyContent::Icon,
        CopyContent::Label,
        CopyContent::IconLabel,
    ] {
        for emphasis in [ActionEmphasis::Quiet, ActionEmphasis::Normal] {
            let spec = CopySpec {
                content,
                emphasis,
                description: Some("Copy the invitation code"),
                ..CopySpec::new(CODE, "Copy code", "Copied", "Couldn't copy it")
            };
            let rects = each_state(&spec);
            let (_, ready) = rects[0];
            for (state, rect) in &rects {
                // The control holds the widest of its faces, so nothing
                // beside it moves when it confirms.
                assert_eq!(rect.size(), ready.size(), "{content:?} {state:?}");
            }
        }
    }
}

#[test]
fn a_disabled_control_carries_its_reason_and_copies_nothing() {
    let context = context();
    let spec = CopySpec {
        availability: Availability::Disabled {
            reason: "The code has expired".into(),
        },
        ..CopySpec::new(CODE, "Copy code", "Copied", "Couldn't copy")
    };
    present(&context, &spec);
    clipboard::request(&context, widget());
    let presented = present(&context, &spec);
    let node = presented
        .snapshot
        .node(
            &PresentationScope::new("invite")
                .instance("code")
                .semantic_id(),
        )
        .unwrap();
    assert!(!node.enabled);
    assert_eq!(
        node.disabled_reason.as_deref(),
        Some("The code has expired")
    );
    assert_eq!(presented.state, CopyState::Ready);
    assert_eq!(
        clipboard::control(&context, widget()).state,
        CopyState::Ready
    );
    assert!(audit_accesskit(&presented.snapshot, &presented.update).is_empty());
}

#[test]
fn a_hidden_control_takes_no_space_and_publishes_nothing() {
    let context = context();
    let spec = CopySpec {
        availability: Availability::Hidden,
        ..CopySpec::new(CODE, "Copy code", "Copied", "Couldn't copy")
    };
    let presented = present(&context, &spec);
    assert_eq!(presented.snapshot.nodes.len(), 1);
    assert!(presented.rect.width() <= 0.0);
    assert!(
        !presented
            .update
            .nodes
            .iter()
            .any(|(_, node)| node.author_id().is_some())
    );
}

#[test]
fn a_request_copies_and_new_text_starts_afresh() {
    let context = context();
    let spec = CopySpec::new(CODE, "Copy code", "Copied", "Couldn't copy");
    present(&context, &spec);
    clipboard::request(&context, widget());
    // The request copies; natively it counts as copied at once.
    present(&context, &spec);
    assert_eq!(present(&context, &spec).state, CopyState::Copied);
    // A changed text drops the confirmation and moves the revision on.
    let before = clipboard::control(&context, widget()).revision;
    let other = CopySpec::new("ZK2-9031", "Copy code", "Copied", "Couldn't copy");
    assert_eq!(present(&context, &other).state, CopyState::Ready);
    assert_eq!(clipboard::control(&context, widget()).revision, before + 1);
}
