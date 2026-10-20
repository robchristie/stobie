#![cfg(test)]
//! Actions presented once per row: each row's control carries its row's
//! argument through clicks, semantics, discovery and invocation.

use egui::Vec2;
use egui_kittest::{Harness, kittest::Queryable};
use serde::Serialize;

use super::*;
use crate::{
    Action, BoundAction, CapabilityDescription, InspectionErrorCode, InspectionOperation,
    InspectionResult, InspectionSelector, InvocationReceipt, ListRowSpec, PresentationScope,
    ReceiptState, ResolvedTarget,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "rows")]
enum RowAct {
    #[action(
        label = "Open",
        description = "Open the person with this ID",
        argument = "number",
        minimum = 1,
        maximum = 1000,
        integer = true
    )]
    Open,
    #[action(
        label = "Remove",
        description = "Remove the person with this ID",
        argument = "number",
        minimum = 1,
        maximum = 1000,
        integer = true
    )]
    Remove,
    #[action(
        label = "Pinned",
        description = "Keep the person with this ID at the top",
        argument = "text",
        max_length = 8
    )]
    Pin,
    #[action(
        label = "Select",
        description = "Select the person with this ID",
        argument = "text",
        max_length = 8
    )]
    Select,
    #[action(label = "Refresh", description = "Reload the people")]
    Refresh,
}

enum Intent {
    Remove(u32),
}

struct Person {
    id: u32,
    name: &'static str,
    role: &'static str,
    pinned: bool,
}

struct People {
    people: Vec<Person>,
    opened: Option<u32>,
    selected: Option<String>,
    refreshed: u64,
    /// Every performed action with its argument, in order.
    performed: Vec<(RowAct, ActionArgument)>,
    errors: Vec<String>,
    /// A person whose Open the application hides.
    hide_open: Option<u32>,
}

impl Default for People {
    fn default() -> Self {
        Self {
            people: vec![
                Person {
                    id: 1,
                    name: "Ada",
                    role: "Mathematician",
                    pinned: false,
                },
                Person {
                    id: 2,
                    name: "Grace",
                    role: "Rear admiral",
                    pinned: false,
                },
                Person {
                    id: 3,
                    name: "Edsger",
                    role: "Computer scientist",
                    pinned: false,
                },
            ],
            opened: None,
            selected: None,
            refreshed: 0,
            performed: Vec::new(),
            errors: Vec::new(),
            hide_open: None,
        }
    }
}

#[derive(Default)]
struct View {
    /// Remove this person and, in the same pass, click its Open.
    remove_then_open: Option<u32>,
}

impl People {
    /// The person an argument names: a number, or text holding one.
    fn person(&self, argument: &ActionArgument) -> Option<&Person> {
        let id = match argument {
            ActionArgument::Number(id) => *id as u32,
            ActionArgument::Text(id) => id.parse().ok()?,
            _ => return None,
        };
        self.people.iter().find(|person| person.id == id)
    }
}

impl Application for People {
    type Action = RowAct;
    type Intent = Intent;
    type UiState = View;

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("People")
    }

    fn availability(&self, _target: ActionTarget<RowAct>) -> Availability {
        Availability::Enabled
    }

    fn availability_with(
        &self,
        target: ActionTarget<RowAct>,
        argument: &ActionArgument,
    ) -> Availability {
        match self.person(argument) {
            None => Availability::Hidden,
            Some(person) if target.action == RowAct::Open && self.hide_open == Some(person.id) => {
                Availability::Hidden
            }
            Some(person) if target.action == RowAct::Remove && person.pinned => {
                Availability::Disabled {
                    reason: format!("Unpin {} first", person.name).into(),
                }
            }
            Some(_) => Availability::Enabled,
        }
    }

    fn ui(&self, ui: &mut Ui, view: &mut View, frame: &mut Frame<'_, RowAct, Intent>) {
        let mut p = frame.presentation(ui, PresentationScope::new("people"), SemanticUiId::root());
        frame.action(ui, &mut p, RowAct::Refresh, ActionEmphasis::Normal);
        for person in &self.people {
            frame.list_row(
                ui,
                &mut p,
                BoundAction::new(RowAct::Select, person.id.to_string()),
                ListRowSpec {
                    title: person.name,
                    detail: Some(person.role),
                },
            );
            ui.horizontal(|ui| {
                frame.action(
                    ui,
                    &mut p,
                    BoundAction::new(RowAct::Open, person.id).with_subject(person.name),
                    ActionEmphasis::Normal,
                );
                frame.icon_action(
                    ui,
                    &mut p,
                    BoundAction::new(RowAct::Remove, person.id).with_subject(person.name),
                    IconId::Remove,
                    ActionEmphasis::Quiet,
                );
                frame.toggle(
                    ui,
                    &mut p,
                    "pinned",
                    BoundAction::new(RowAct::Pin, person.id.to_string()).with_subject(person.name),
                    person.pinned,
                );
            });
        }
        if let Some(id) = view.remove_then_open.take() {
            frame.emit(Intent::Remove(id));
            frame.trigger(BoundAction::new(RowAct::Open, id));
        }
    }

    fn apply(&mut self, intent: Intent, _view: &mut View) -> Result<bool, String> {
        match intent {
            Intent::Remove(id) => {
                self.people.retain(|person| person.id != id);
                Ok(true)
            }
        }
    }

    fn perform(&mut self, target: ActionTarget<RowAct>, _view: &mut View) -> Result<bool, String> {
        match target.action {
            RowAct::Refresh => {
                self.refreshed += 1;
                self.performed.push((RowAct::Refresh, ActionArgument::None));
                Ok(true)
            }
            action => Err(format!("{} needs a person", action.specification().label)),
        }
    }

    fn perform_with(
        &mut self,
        target: ActionTarget<RowAct>,
        argument: ActionArgument,
        view: &mut View,
    ) -> Result<bool, String> {
        let Some(id) = self.person(&argument).map(|person| person.id) else {
            return self.perform(target, view);
        };
        self.performed.push((target.action, argument.clone()));
        match target.action {
            RowAct::Open => self.opened = Some(id),
            RowAct::Remove => self.people.retain(|person| person.id != id),
            RowAct::Pin => {
                let person = self.people.iter_mut().find(|person| person.id == id);
                let person = person.expect("an available person");
                person.pinned = !person.pinned;
            }
            RowAct::Select => self.selected = Some(id.to_string()),
            RowAct::Refresh => return self.perform(target, view),
        }
        Ok(true)
    }

    fn report_error(&mut self, message: String) {
        self.errors.push(message);
    }
}

#[derive(Default)]
struct Probe {
    host: Option<AppHost<People>>,
}

fn harness(people: People) -> Harness<'static, Probe> {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(700.0, 600.0))
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
    harness.state_mut().host = Some(AppHost::new(&context, people));
    harness.run();
    harness
}

fn host<'a>(harness: &'a Harness<'_, Probe>) -> &'a AppHost<People> {
    harness.state().host.as_ref().unwrap()
}

fn host_mut<'a>(harness: &'a mut Harness<'_, Probe>) -> &'a mut AppHost<People> {
    harness.state_mut().host.as_mut().unwrap()
}

fn with_inspection(harness: &mut Harness<'_, Probe>) {
    host_mut(harness).enable_inspection(Inspection::new("rows-test"));
    harness.run();
}

fn request(
    harness: &Harness<'_, Probe>,
    id: &str,
    operation: InspectionOperation,
) -> InspectionReply {
    let host = host(harness);
    let inspection = host.inspection().unwrap();
    inspection.handle(InspectionRequest {
        version: 1,
        request_id: id.into(),
        instance: Some(inspection.instance()),
        operation,
    })
}

fn selector(action: RowAct) -> InspectionSelector {
    InspectionSelector {
        capability: Some(action.stable_id().into()),
        ..Default::default()
    }
}

fn discover(
    harness: &Harness<'_, Probe>,
    selector: InspectionSelector,
) -> Vec<CapabilityDescription> {
    let reply = request(
        harness,
        "discover",
        InspectionOperation::Discover {
            selector,
            limit: 64,
            cursor: None,
        },
    );
    let Some(InspectionResult::Discover { capabilities, .. }) = reply.result else {
        panic!("discovery failed: {:?}", reply.error);
    };
    capabilities
}

/// Admit an invocation, run passes until it has drained, and read its receipt.
fn invoke(
    harness: &mut Harness<'_, Probe>,
    id: &str,
    selector: InspectionSelector,
    expected: ResolvedTarget,
    arguments: serde_json::Value,
) -> InvocationReceipt {
    let admitted = receipt(request(
        harness,
        id,
        InspectionOperation::Invoke {
            selector,
            expected,
            arguments,
        },
    ));
    if admitted.state != ReceiptState::Queued {
        return admitted;
    }
    harness.run();
    receipt(request(
        harness,
        &format!("lookup-{id}"),
        InspectionOperation::Receipt {
            request_id: id.into(),
        },
    ))
}

fn receipt(reply: InspectionReply) -> InvocationReceipt {
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("receipt failed: {:?}", reply.error);
    };
    receipt
}

/// The row control's binding for an action and argument.
fn row_binding(
    harness: &Harness<'_, Probe>,
    action: RowAct,
    argument: serde_json::Value,
) -> CapabilityDescription {
    let mut found: Vec<_> = discover(
        harness,
        InspectionSelector {
            argument: Some(argument.clone()),
            ..selector(action)
        },
    );
    assert_eq!(found.len(), 1, "one {action:?} control carries {argument}");
    found.remove(0)
}

#[test]
fn each_rows_controls_dispatch_their_own_argument() {
    let mut harness = harness(People::default());
    harness.get_by_label("Open Grace").click();
    harness.run();
    assert_eq!(host(&harness).app().opened, Some(2));
    harness.get_by_label("Pinned Edsger").click();
    harness.run();
    // The list row is named by its title and selects its own person.
    harness.get_by_label("Ada").click();
    harness.run();
    harness.get_by_label("Remove Grace").click();
    harness.run();
    let people = host(&harness).app();
    assert_eq!(
        people.performed,
        vec![
            (RowAct::Open, ActionArgument::Number(2.0)),
            (RowAct::Pin, ActionArgument::Text("3".into())),
            (RowAct::Select, ActionArgument::Text("1".into())),
            (RowAct::Remove, ActionArgument::Number(2.0)),
        ]
    );
    assert_eq!(people.selected.as_deref(), Some("1"));
    assert!(
        people
            .people
            .iter()
            .any(|person| person.id == 3 && person.pinned)
    );
    assert!(people.people.iter().all(|person| person.id != 2));
    assert!(people.errors.is_empty(), "{:?}", people.errors);
    // The removed row's controls are gone with it.
    assert!(harness.query_by_label("Open Grace").is_none());
    // Keyboard activation reaches the same row's control.
    harness.get_by_label("Open Edsger").focus();
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(host(&harness).app().opened, Some(3));
}

#[test]
fn rows_publish_each_controls_argument_under_its_own_name() {
    let harness = harness(People::default());
    let snapshot = host(&harness).ui_snapshot();
    assert!(
        snapshot.semantic_audit.is_empty(),
        "{:?}",
        snapshot.semantic_audit
    );
    let carried = |action: RowAct| -> Vec<(String, Option<serde_json::Value>)> {
        snapshot
            .by_action(action)
            .map(|node| (node.name.clone(), node.argument.clone()))
            .collect()
    };
    assert_eq!(
        carried(RowAct::Open),
        [("Open Ada", 1), ("Open Grace", 2), ("Open Edsger", 3)]
            .map(|(name, id)| (name.to_owned(), Some(serde_json::json!(id))))
    );
    assert_eq!(
        carried(RowAct::Remove)
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        ["Remove Ada", "Remove Grace", "Remove Edsger"]
    );
    assert_eq!(
        carried(RowAct::Pin),
        [
            ("Pinned Ada", "1"),
            ("Pinned Grace", "2"),
            ("Pinned Edsger", "3")
        ]
        .map(|(name, id)| (name.to_owned(), Some(serde_json::json!(id))))
    );
    let rows: Vec<_> = snapshot.by_action(RowAct::Select).collect();
    assert_eq!(
        rows.iter().map(|row| row.name.as_str()).collect::<Vec<_>>(),
        ["Ada", "Grace", "Edsger"]
    );
    assert!(rows.iter().all(|row| row.role == UiRole::Button));
    assert_eq!(
        rows[1].description.as_deref(),
        Some("Rear admiral; Select the person with this ID")
    );
    assert_eq!(
        snapshot
            .by_action(RowAct::Pin)
            .map(|node| node.checked)
            .collect::<Vec<_>>(),
        [Some(false); 3]
    );
    // An action without an argument publishes none.
    assert_eq!(
        snapshot
            .by_action(RowAct::Refresh)
            .map(|node| node.argument.clone())
            .collect::<Vec<_>>(),
        [None]
    );
    // Every control has its own stable identity, and the next pass repeats it.
    let ids: Vec<_> = snapshot.nodes.iter().map(|node| node.id.clone()).collect();
    let unique: std::collections::BTreeSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len());
    let mut harness = harness;
    harness.run();
    assert_eq!(
        host(&harness)
            .ui_snapshot()
            .nodes
            .iter()
            .map(|node| node.id.clone())
            .collect::<Vec<_>>(),
        ids
    );
    // AccessKit names each control as the snapshot does.
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
    let mismatches = crate::audit_accesskit(host(&harness).ui_snapshot(), &update);
    assert!(mismatches.is_empty(), "{mismatches:?}");
}

#[test]
fn availability_for_an_argument_disables_one_rows_control() {
    let mut people = People::default();
    people.people[0].pinned = true;
    let mut harness = harness(people);
    with_inspection(&mut harness);
    let snapshot = host(&harness).ui_snapshot();
    let remove: Vec<_> = snapshot.by_action(RowAct::Remove).collect();
    assert_eq!(
        remove
            .iter()
            .map(|node| (node.enabled, node.disabled_reason.as_deref()))
            .collect::<Vec<_>>(),
        [(false, Some("Unpin Ada first")), (true, None), (true, None)]
    );
    assert_eq!(
        row_binding(&harness, RowAct::Remove, serde_json::json!(1)).availability,
        Availability::Disabled {
            reason: "Unpin Ada first".into()
        }
    );
    let ada = row_binding(&harness, RowAct::Remove, serde_json::json!(1)).target;
    let refused = invoke(
        &mut harness,
        "remove-ada",
        selector(RowAct::Remove),
        ada,
        serde_json::json!({ "value": 1 }),
    );
    assert_eq!(refused.state, ReceiptState::Rejected);
    assert_eq!(
        refused.error.map(|error| error.code),
        Some(InspectionErrorCode::Unavailable)
    );
    assert_eq!(host(&harness).app().people.len(), 3);
}

#[test]
fn agents_discover_each_rows_control_and_invoke_it_by_argument() {
    let mut harness = harness(People::default());
    with_inspection(&mut harness);
    let open = discover(&harness, selector(RowAct::Open));
    // One binding per row, carrying its ID, and the action's own binding,
    // which takes any ID and has no control of its own.
    assert_eq!(
        open.iter()
            .map(|capability| (capability.argument.clone(), capability.rendered))
            .collect::<Vec<_>>(),
        vec![
            (Some(serde_json::json!(1)), true),
            (Some(serde_json::json!(2)), true),
            (Some(serde_json::json!(3)), true),
            (None, false),
        ]
    );
    let snapshot = host(&harness).ui_snapshot();
    for capability in &open[..3] {
        let node = snapshot.node(&capability.id).expect("the row's control");
        assert_eq!(node.argument, capability.argument);
        assert_eq!(capability.target.argument, capability.argument);
    }

    // A value chooses the row's control that carries it.
    let grace = row_binding(&harness, RowAct::Open, serde_json::json!(2));
    let opened = invoke(
        &mut harness,
        "open-grace",
        selector(RowAct::Open),
        grace.target.clone(),
        serde_json::json!({ "value": 2 }),
    );
    assert_eq!(opened.state, ReceiptState::Completed, "{opened:?}");
    assert_eq!(
        opened.target.as_ref().map(|target| &target.id),
        Some(&grace.id)
    );
    assert_eq!(host(&harness).app().opened, Some(2));

    // The selector's argument names the row; the control carries its value.
    let edsger = row_binding(&harness, RowAct::Open, serde_json::json!(3));
    let opened = invoke(
        &mut harness,
        "open-edsger",
        InspectionSelector {
            argument: Some(serde_json::json!(3.0)),
            ..selector(RowAct::Open)
        },
        edsger.target,
        serde_json::Value::Null,
    );
    assert_eq!(opened.state, ReceiptState::Completed, "{opened:?}");
    assert_eq!(host(&harness).app().opened, Some(3));

    // A row's control takes its own value only.
    let refused = invoke(
        &mut harness,
        "open-grace-as-ada",
        InspectionSelector {
            id: Some(grace.id.clone()),
            ..selector(RowAct::Open)
        },
        grace.target.clone(),
        serde_json::json!({ "value": 1 }),
    );
    assert_eq!(refused.state, ReceiptState::Rejected);
    assert_eq!(
        refused.error.map(|error| error.code),
        Some(InspectionErrorCode::InvalidArguments)
    );
    // Without a value, the controls and the action's binding are ambiguous.
    let ambiguous = invoke(
        &mut harness,
        "open-anyone",
        selector(RowAct::Open),
        grace.target.clone(),
        serde_json::Value::Null,
    );
    assert_eq!(
        ambiguous.error.map(|error| error.code),
        Some(InspectionErrorCode::Ambiguous)
    );
    // A text argument travels as text, and a list row's control by it too.
    let ada = row_binding(&harness, RowAct::Select, serde_json::json!("1"));
    let selected = invoke(
        &mut harness,
        "select-ada",
        selector(RowAct::Select),
        ada.target,
        serde_json::json!({ "value": "1" }),
    );
    assert_eq!(selected.state, ReceiptState::Completed, "{selected:?}");
    assert_eq!(host(&harness).app().selected.as_deref(), Some("1"));
    assert_eq!(
        host(&harness).app().performed,
        vec![
            (RowAct::Open, ActionArgument::Number(2.0)),
            (RowAct::Open, ActionArgument::Number(3.0)),
            (RowAct::Select, ActionArgument::Text("1".into())),
        ]
    );
}

#[test]
fn a_removed_rows_control_is_stale_and_its_argument_refused() {
    let mut harness = harness(People::default());
    with_inspection(&mut harness);
    let edsger = row_binding(&harness, RowAct::Open, serde_json::json!(3));
    let remove = row_binding(&harness, RowAct::Remove, serde_json::json!(3));
    // Both are admitted against the same observation; the removal runs first.
    for (id, capability) in [("remove", &remove), ("open", &edsger)] {
        let admitted = receipt(request(
            &harness,
            id,
            InspectionOperation::Invoke {
                selector: InspectionSelector {
                    id: Some(capability.id.clone()),
                    ..Default::default()
                },
                expected: capability.target.clone(),
                arguments: serde_json::Value::Null,
            },
        ));
        assert_eq!(admitted.state, ReceiptState::Queued, "{admitted:?}");
    }
    harness.run();
    let lookup = |harness: &Harness<'_, Probe>, id: &str| {
        receipt(request(
            harness,
            &format!("lookup-{id}"),
            InspectionOperation::Receipt {
                request_id: id.into(),
            },
        ))
    };
    assert_eq!(lookup(&harness, "remove").state, ReceiptState::Completed);
    // Its binding, recalculated before it runs, is hidden by the
    // application's availability for the removed person.
    let stale = lookup(&harness, "open");
    assert_eq!(stale.state, ReceiptState::Rejected);
    assert_eq!(
        stale.error.map(|error| (error.code, error.message)),
        Some((
            InspectionErrorCode::Unavailable,
            "capability is hidden".into()
        ))
    );
    // Discovered again, the row is gone; its old target matches nothing.
    assert!(
        discover(
            &harness,
            InspectionSelector {
                argument: Some(serde_json::json!(3)),
                ..selector(RowAct::Open)
            }
        )
        .is_empty()
    );
    let gone = invoke(
        &mut harness,
        "open-again",
        InspectionSelector {
            id: Some(edsger.id.clone()),
            ..Default::default()
        },
        edsger.target.clone(),
        serde_json::Value::Null,
    );
    assert_eq!(
        gone.error.map(|error| error.code),
        Some(InspectionErrorCode::NoMatch)
    );
    // The action's own binding takes the ID, and the application's
    // availability for it refuses the removed person before performing.
    let action = discover(&harness, selector(RowAct::Open))
        .into_iter()
        .find(|capability| capability.argument.is_none())
        .expect("the action's own binding");
    let refused = invoke(
        &mut harness,
        "open-removed",
        selector(RowAct::Open),
        action.target,
        serde_json::json!({ "value": 3 }),
    );
    assert_eq!(refused.state, ReceiptState::Rejected, "{refused:?}");
    assert_eq!(
        refused.error.map(|error| error.message),
        Some("Open is unavailable".into())
    );
    let people = host(&harness).app();
    assert_eq!(people.opened, None);
    assert_eq!(
        people.performed,
        vec![(RowAct::Remove, ActionArgument::Number(3.0))]
    );
}

#[test]
fn a_click_on_a_row_removed_earlier_in_its_pass_is_refused() {
    let mut harness = harness(People::default());
    host_mut(&mut harness).ui_state.remove_then_open = Some(2);
    host_mut(&mut harness).request_repaint("test");
    harness.run();
    let people = host(&harness).app();
    assert_eq!(people.opened, None);
    assert!(people.performed.is_empty(), "{:?}", people.performed);
    assert_eq!(people.errors, ["Open is unavailable"]);
}

#[test]
fn a_hidden_rows_control_presents_nothing() {
    let mut harness = harness(People {
        hide_open: Some(2),
        ..People::default()
    });
    with_inspection(&mut harness);
    assert!(harness.query_by_label("Open Grace").is_none());
    assert!(harness.query_by_label("Open Ada").is_some());
    assert_eq!(
        discover(&harness, selector(RowAct::Open))
            .iter()
            .map(|capability| capability.argument.clone())
            .collect::<Vec<_>>(),
        [Some(serde_json::json!(1)), Some(serde_json::json!(3)), None]
    );
}
