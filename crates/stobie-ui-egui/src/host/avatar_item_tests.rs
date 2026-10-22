#![cfg(test)]
//! A strip of pinned people whose avatars are themselves the controls: each
//! item is one focusable target and one semantic node, named by its caption,
//! carrying its person's ID, and the strip scrolls at phone width.

use egui::Vec2;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use serde::Serialize;

use super::*;
use crate::{
    Action, AvatarItemSpec, AvatarSize, AvatarSpec, BoundAction, CapabilityDescription,
    InspectionOperation, InspectionResult, InspectionSelector, InvocationReceipt,
    PresentationScope, ReceiptState, ResolvedTarget, avatar_item_size,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "pinned")]
enum PinAct {
    #[action(
        label = "Open conversation",
        description = "Open the conversation with this person",
        argument = "number",
        minimum = 1,
        maximum = 1000,
        integer = true
    )]
    Open,
}

const PEOPLE: [(u32, &str); 8] = [
    (1, "Ada Lovelace"),
    (2, "Grace Hopper"),
    (3, "Edsger Dijkstra"),
    (4, "Barbara Liskov"),
    (5, "Alan Turing"),
    (6, "Frances Allen"),
    (7, "Donald Knuth"),
    (8, "Radia Perlman"),
];

/// The person whose conversation is unavailable.
const AWAY: u32 = 4;

#[derive(Default)]
struct Pinned {
    current: Option<u32>,
    opened: Vec<u32>,
}

#[derive(Default)]
struct View {
    tokens: Option<crate::DesignTokens>,
    item: Option<Rect>,
    strip: Option<Rect>,
}

impl Application for Pinned {
    type Action = PinAct;
    type Intent = ();
    type UiState = View;

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("Pinned")
    }

    fn availability(&self, _target: ActionTarget<PinAct>) -> Availability {
        Availability::Enabled
    }

    fn availability_with(
        &self,
        _target: ActionTarget<PinAct>,
        argument: &ActionArgument,
    ) -> Availability {
        match argument {
            ActionArgument::Number(id) if *id as u32 == AWAY => Availability::Disabled {
                reason: "Barbara is away".into(),
            },
            ActionArgument::Number(_) => Availability::Enabled,
            _ => Availability::Hidden,
        }
    }

    fn ui(&self, ui: &mut Ui, view: &mut View, frame: &mut Frame<'_, PinAct, ()>) {
        let mut p = frame.presentation(ui, PresentationScope::new("pinned"), SemanticUiId::root());
        view.tokens = Some(*p.tokens());
        let response = p.avatar_strip(ui, "strip", |ui, p| {
            for (id, name) in PEOPLE {
                let spec = AvatarItemSpec::new(AvatarSpec::person(name, AvatarSize::List), name)
                    .selected(self.current == Some(id));
                let response = frame.avatar_item(ui, p, BoundAction::new(PinAct::Open, id), &spec);
                if id == 1 {
                    view.item = Some(response.rect);
                }
            }
            ui.min_rect()
        });
        view.strip = Some(response);
    }

    fn apply(&mut self, _intent: (), _view: &mut View) -> Result<bool, String> {
        Ok(false)
    }

    fn perform(&mut self, _target: ActionTarget<PinAct>, _view: &mut View) -> Result<bool, String> {
        Err("Open needs a person".into())
    }

    fn perform_with(
        &mut self,
        _target: ActionTarget<PinAct>,
        argument: ActionArgument,
        _view: &mut View,
    ) -> Result<bool, String> {
        let ActionArgument::Number(id) = argument else {
            return Err("Open needs a person".into());
        };
        self.opened.push(id as u32);
        self.current = Some(id as u32);
        Ok(true)
    }

    fn report_error(&mut self, message: String) {
        panic!("{message}");
    }
}

#[derive(Default)]
struct Probe {
    host: Option<AppHost<Pinned>>,
}

fn harness(width: f32) -> Harness<'static, Probe> {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(width, 400.0))
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
    harness.state_mut().host = Some(AppHost::new(&context, Pinned::default()));
    harness.run();
    harness
}

fn host<'a>(harness: &'a Harness<'_, Probe>) -> &'a AppHost<Pinned> {
    harness.state().host.as_ref().unwrap()
}

fn request(
    harness: &Harness<'_, Probe>,
    id: &str,
    operation: InspectionOperation,
) -> InspectionReply {
    let inspection = host(harness).inspection().unwrap();
    inspection.handle(InspectionRequest {
        version: 1,
        request_id: id.into(),
        instance: Some(inspection.instance()),
        operation,
    })
}

fn discover(harness: &Harness<'_, Probe>, argument: u32) -> Vec<CapabilityDescription> {
    let reply = request(
        harness,
        "discover",
        InspectionOperation::Discover {
            selector: InspectionSelector {
                capability: Some(PinAct::Open.stable_id().into()),
                argument: Some(serde_json::json!(argument)),
                ..Default::default()
            },
            limit: 64,
            cursor: None,
        },
    );
    let Some(InspectionResult::Discover { capabilities, .. }) = reply.result else {
        panic!("discovery failed: {:?}", reply.error);
    };
    capabilities
}

fn invoke(
    harness: &mut Harness<'_, Probe>,
    argument: u32,
    expected: ResolvedTarget,
) -> InvocationReceipt {
    let selector = InspectionSelector {
        capability: Some(PinAct::Open.stable_id().into()),
        argument: Some(serde_json::json!(argument)),
        ..Default::default()
    };
    let reply = request(
        harness,
        "open",
        InspectionOperation::Invoke {
            selector,
            expected,
            arguments: serde_json::json!({ "value": argument }),
        },
    );
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("invocation failed: {:?}", reply.error);
    };
    if receipt.state != ReceiptState::Queued {
        return receipt;
    }
    harness.run();
    let reply = request(
        harness,
        "lookup",
        InspectionOperation::Receipt {
            request_id: "open".into(),
        },
    );
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("receipt failed: {:?}", reply.error);
    };
    receipt
}

fn items(harness: &Harness<'_, Probe>) -> Vec<UiNode> {
    host(harness)
        .ui_snapshot()
        .nodes
        .iter()
        .filter(|node| node.role == UiRole::NavigationItem)
        .cloned()
        .collect()
}

#[test]
fn each_avatar_is_one_named_target_carrying_its_persons_id() {
    let mut harness = harness(1200.0);
    let items = items(&harness);
    assert_eq!(items.len(), PEOPLE.len());
    for (node, (id, name)) in items.iter().zip(PEOPLE) {
        assert_eq!(node.name, name);
        assert_eq!(node.argument, Some(serde_json::json!(id)));
        assert_eq!(node.enabled, id != AWAY, "{name}");
    }
    // One AccessKit node, a button named by the caption.
    let grace = harness.get_by_label("Grace Hopper");
    assert_eq!(grace.accesskit_node().role(), egui::accesskit::Role::Button);
    grace.click();
    harness.run();
    assert_eq!(host(&harness).app().opened, [2]);
    // The opened person is the current one.
    let grace = items_named(&harness, "Grace Hopper");
    assert!(grace.selected);
    assert!(
        grace
            .description
            .as_deref()
            .unwrap()
            .ends_with("Current destination")
    );
    assert_eq!(
        harness
            .get_by_label("Grace Hopper")
            .accesskit_node()
            .aria_current(),
        Some(egui::accesskit::AriaCurrent::True)
    );
    // A disabled item keeps its reason and does nothing.
    harness.get_by_label("Barbara Liskov").click();
    harness.run();
    assert_eq!(host(&harness).app().opened, [2]);
    let barbara = items_named(&harness, "Barbara Liskov");
    assert_eq!(barbara.disabled_reason.as_deref(), Some("Barbara is away"));
    let snapshot = host(&harness).ui_snapshot();
    assert!(
        snapshot.semantic_audit.is_empty(),
        "{:?}",
        snapshot.semantic_audit
    );
}

fn items_named(harness: &Harness<'_, Probe>, name: &str) -> UiNode {
    items(harness)
        .into_iter()
        .find(|node| node.name == name)
        .unwrap()
}

#[test]
fn an_item_is_at_least_the_touch_target_and_holds_the_strip_avatar() {
    let harness = harness(1200.0);
    let view = host(&harness).ui_state();
    let item = view.item.unwrap();
    let tokens = host(&harness).ui_state().tokens.unwrap();
    let minimum = tokens.geometry.minimum_hit_size.0;
    assert!(
        item.width() >= minimum && item.height() >= minimum,
        "{item:?}"
    );
    let expected = avatar_item_size(&tokens, 1.0);
    assert!(
        (item.size() - expected).length() < 0.5,
        "{item:?} {expected:?}"
    );
    let side = crate::avatar_size(&tokens, AvatarSize::Strip, 1.0);
    assert!(item.width() >= side && item.height() > side, "{item:?}");
}

#[test]
fn agents_open_a_pinned_person_through_their_avatar() {
    let mut harness = harness(1200.0);
    host_mut(&mut harness).enable_inspection(crate::Inspection::new("pinned-test"));
    harness.run();
    let mut found = discover(&harness, 3);
    assert_eq!(found.len(), 1, "{found:?}");
    let found = found.remove(0);
    // The binding is the item's own control.
    assert_eq!(found.id, items_named(&harness, "Edsger Dijkstra").id);
    let receipt = invoke(&mut harness, 3, found.target);
    assert_eq!(receipt.state, ReceiptState::Completed, "{receipt:?}");
    assert_eq!(host(&harness).app().opened, [3]);
}

fn host_mut<'a>(harness: &'a mut Harness<'_, Probe>) -> &'a mut AppHost<Pinned> {
    harness.state_mut().host.as_mut().unwrap()
}

#[test]
fn at_phone_width_the_strip_scrolls_and_keyboard_focus_brings_an_item_into_view() {
    let mut harness = harness(320.0);
    // The strip is one row as tall as an item, narrower than its items.
    let tokens = host(&harness).ui_state().tokens.unwrap();
    let item = avatar_item_size(&tokens, 1.0);
    let strip = host(&harness).ui_state().strip.unwrap();
    assert!(strip.height() <= item.y + 1.0, "{strip:?}");
    let shown = items(&harness);
    assert!(shown.len() < PEOPLE.len(), "{} shown", shown.len());
    assert!(
        shown
            .iter()
            .all(|node| Rect::from(node.rect).right() <= 320.0)
    );
    // Tab through to the last person; each focused item is in view.
    for (id, name) in PEOPLE {
        if id == AWAY {
            continue;
        }
        harness.key_press(egui::Key::Tab);
        harness.run();
        let node = items_named(&harness, name);
        assert!(node.focused, "{name}");
        let rect = Rect::from(node.rect);
        assert!(
            rect.left() >= 0.0 && rect.right() <= 320.0,
            "{name}: {rect:?}"
        );
    }
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(host(&harness).app().opened, [8]);
}
