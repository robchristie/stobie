#![cfg(test)]
//! Actions presented once per row: each row's control carries its row's
//! argument through clicks, semantics, discovery and invocation.

use egui::Vec2;
use egui_kittest::{Harness, kittest::Queryable};
use serde::Serialize;

use super::*;
use crate::{
    Action, BoundAction, CapabilityDescription, DomainReference, InspectionErrorCode,
    InspectionOperation, InspectionResult, InspectionSelector, InvocationReceipt, ListRowSpec,
    PresentationScope, ReceiptState, ResolvedTarget,
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
    /// Present Open for this person again in two other presentations, and
    /// record the egui IDs of their responses.
    open_elsewhere: Option<u32>,
    elsewhere: Vec<egui::Id>,
    /// The other presentations name their own domain objects, as two lists
    /// in different places might.
    elsewhere_domains: bool,
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
        if let Some(id) = view.open_elsewhere {
            view.elsewhere = ["first", "second"]
                .map(|scope| {
                    let mut other =
                        frame.presentation(ui, PresentationScope::new(scope), SemanticUiId::root());
                    if view.elsewhere_domains {
                        other = other.with_domain_reference(DomainReference::External {
                            namespace: "list".into(),
                            id: scope.into(),
                        });
                    }
                    frame
                        .action(
                            ui,
                            &mut other,
                            BoundAction::new(RowAct::Open, id),
                            ActionEmphasis::Normal,
                        )
                        .id
                })
                .into();
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

#[test]
fn a_control_whose_argument_its_contract_refuses_keeps_the_actions_own_binding() {
    // Open takes IDs up to 1000; this person's ID is beyond it.
    let mut people = People::default();
    people.people.push(Person {
        id: 5000,
        name: "Out of range",
        role: "Nobody",
        pinned: false,
    });
    let mut harness = harness(people);
    with_inspection(&mut harness);
    let found = discover(&harness, selector(RowAct::Open));
    assert_eq!(
        found
            .iter()
            .map(|capability| capability.argument.clone())
            .collect::<Vec<_>>(),
        [
            Some(serde_json::json!(1)),
            Some(serde_json::json!(2)),
            Some(serde_json::json!(3)),
            Some(serde_json::json!(5000)),
            None
        ],
        "{found:?}"
    );
    // The control is disabled with the contract's reason, carrying its value.
    let refused = &found[3];
    assert!(
        matches!(&refused.availability, Availability::Disabled { reason }
            if reason.contains("The control's argument is invalid")),
        "{refused:?}"
    );
    // The action's own binding still takes any value its contract allows.
    let own = found.last().unwrap();
    assert_eq!(own.availability, Availability::Enabled);
    assert_eq!(
        own.id,
        ActionTarget::application(RowAct::Open).semantic_id().into()
    );
    // A value no row carries reaches the action's own binding.
    host_mut(&mut harness).app_mut().people.push(Person {
        id: 7,
        name: "Barbara",
        role: "Computer scientist",
        pinned: false,
    });
    let opened = invoke(
        &mut harness,
        "open-7",
        selector(RowAct::Open),
        own.target.clone(),
        serde_json::json!({ "value": 7 }),
    );
    assert_eq!(opened.state, ReceiptState::Completed, "{opened:?}");
    assert_eq!(host(&harness).app().opened, Some(7));
    // The refused value selects its own control, which refuses it.
    let refused = invoke(
        &mut harness,
        "open-5000",
        InspectionSelector {
            id: Some(refused.id.clone()),
            ..selector(RowAct::Open)
        },
        refused.target.clone(),
        serde_json::Value::Null,
    );
    assert_eq!(
        refused.error.map(|error| error.code),
        Some(InspectionErrorCode::Unavailable)
    );
}

#[test]
fn hidden_controls_are_scoped_by_their_presentation() {
    let mut harness = harness(People {
        hide_open: Some(2),
        ..People::default()
    });
    host_mut(&mut harness).ui_state.open_elsewhere = Some(2);
    host_mut(&mut harness).request_repaint("test");
    harness.run();
    // Two presentations hiding the same action with the same argument give
    // their empty responses distinct IDs...
    let hidden = host(&harness).ui_state.elsewhere.clone();
    assert_eq!(hidden.len(), 2);
    assert_ne!(hidden[0], hidden[1]);
    // ...the IDs the controls have when shown.
    host_mut(&mut harness).app_mut().hide_open = None;
    host_mut(&mut harness).request_repaint("test");
    harness.run();
    assert_eq!(host(&harness).ui_state.elsewhere, hidden);
    assert_eq!(harness.query_all_by_label("Open").count(), 2);
}

/// Every node in the AccessKit tree the harness holds, as one update.
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

/// Each node presenting `Open` for `id`, with the targets a query gives it.
fn open_controls(harness: &Harness<'_, Probe>, id: u32) -> Vec<(UiNode, Vec<ResolvedTarget>)> {
    let reply = request(
        harness,
        "query-open",
        InspectionOperation::Query {
            selector: InspectionSelector {
                capability: Some(RowAct::Open.stable_id().into()),
                ..Default::default()
            },
            limit: 64,
            cursor: None,
        },
    );
    let Some(InspectionResult::Query { nodes, .. }) = reply.result else {
        panic!("query failed: {:?}", reply.error);
    };
    nodes
        .into_iter()
        .filter(|node| node.node.argument == Some(serde_json::json!(id)))
        .map(|node| (node.node, node.targets))
        .collect()
}

#[test]
fn one_action_and_value_presented_twice_is_bound_through_each_control() {
    for domains in [false, true] {
        let mut harness = harness(People::default());
        with_inspection(&mut harness);
        host_mut(&mut harness).ui_state.open_elsewhere = Some(2);
        host_mut(&mut harness).ui_state.elsewhere_domains = domains;
        host_mut(&mut harness).request_repaint("test");
        harness.run();
        // Grace's row and the two other presentations each present Open 2,
        // and each control is its own binding, named and identified as its
        // node is.
        let controls = open_controls(&harness, 2);
        assert_eq!(controls.len(), 3, "domains {domains}");
        let mut names: Vec<_> = controls.iter().map(|(node, _)| node.name.clone()).collect();
        names.sort();
        assert_eq!(names, ["Open", "Open", "Open Grace"]);
        for (node, targets) in &controls {
            assert_eq!(targets.len(), 1, "{} has its own target", node.id.0);
            assert_eq!(targets[0].id, node.id);
            assert_eq!(targets[0].argument, Some(serde_json::json!(2)));
        }
        let bindings = discover(
            &harness,
            InspectionSelector {
                argument: Some(serde_json::json!(2)),
                ..selector(RowAct::Open)
            },
        );
        assert_eq!(bindings.len(), 3);
        // Invoking a control by its own ID performs Open 2 and reports that
        // control.
        let (second, _) = controls
            .iter()
            .find(|(node, _)| node.name == "Open")
            .unwrap();
        let by_id = invoke(
            &mut harness,
            "by-id",
            InspectionSelector {
                id: Some(second.id.clone()),
                ..Default::default()
            },
            controls
                .iter()
                .find(|(node, _)| node.id == second.id)
                .unwrap()
                .1[0]
                .clone(),
            serde_json::Value::Null,
        );
        assert_eq!(by_id.state, ReceiptState::Completed, "{by_id:?}");
        assert_eq!(
            by_id.target.map(|target| target.id),
            Some(second.id.clone())
        );
        // Invoking the action with the value they all carry performs it
        // once, through whichever control the caller discovered.
        for (index, (node, targets)) in controls.iter().enumerate() {
            let by_value = invoke(
                &mut harness,
                &format!("by-value-{index}"),
                selector(RowAct::Open),
                targets[0].clone(),
                serde_json::json!({ "value": 2 }),
            );
            assert_eq!(by_value.state, ReceiptState::Completed, "{by_value:?}");
            assert_eq!(
                by_value.target.map(|target| target.id),
                Some(node.id.clone())
            );
        }
        let people = host(&harness).app();
        assert_eq!(
            people.performed,
            vec![(RowAct::Open, ActionArgument::Number(2.0)); 4]
        );
        // Controls carrying different values are still told apart.
        let action = discover(&harness, selector(RowAct::Open))
            .into_iter()
            .find(|capability| capability.argument.is_none())
            .unwrap();
        let unsent = invoke(
            &mut harness,
            "no-value",
            selector(RowAct::Open),
            action.target,
            serde_json::Value::Null,
        );
        assert_eq!(
            unsent.error.map(|error| error.code),
            Some(InspectionErrorCode::Ambiguous)
        );
        let mismatches =
            crate::audit_accesskit(host(&harness).ui_snapshot(), &accesskit_update(&harness));
        assert!(mismatches.is_empty(), "{mismatches:?}");
    }
}
