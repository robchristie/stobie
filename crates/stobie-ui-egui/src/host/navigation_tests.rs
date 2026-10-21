#![cfg(test)]
//! The navigation recipe in a hosted application: stacked with Back on a
//! phone, split on a tablet and in columns on a desktop; a resize keeps what
//! is open; dynamic destinations mark the current one; and agents read the
//! open item, open another and go back.

use egui::Vec2;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use serde::Serialize;

use super::*;
use crate::{
    Action, BoundAction, DestinationSpec, InspectionOperation, InspectionResult,
    InspectionSelector, NavigationContent, NavigationDetail, NavigationLayout, NavigationPane,
    NavigationSpec, PresentationScope, ReadingTextSpec, ReceiptState,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "people")]
enum PeopleAct {
    #[action(
        label = "Open",
        description = "Open the conversation with this person",
        argument = "text",
        max_length = 8
    )]
    Open,
    #[action(label = "Back", description = "Return to the previous level")]
    Back,
    #[action(label = "Details", description = "Show or hide this person's details")]
    Details,
}

enum Intent {}

struct Person {
    id: &'static str,
    name: &'static str,
}

struct People {
    people: Vec<Person>,
    open: Option<&'static str>,
    details: bool,
    performed: Vec<(PeopleAct, ActionArgument)>,
    errors: Vec<String>,
}

impl Default for People {
    fn default() -> Self {
        Self {
            people: vec![
                Person {
                    id: "p1",
                    name: "Ada",
                },
                Person {
                    id: "p2",
                    name: "Grace",
                },
                Person {
                    id: "p3",
                    name: "Katherine",
                },
            ],
            open: None,
            details: false,
            performed: Vec::new(),
            errors: Vec::new(),
        }
    }
}

impl People {
    fn person(&self, argument: &ActionArgument) -> Option<&Person> {
        let ActionArgument::Text(id) = argument else {
            return None;
        };
        self.people.iter().find(|person| person.id == id.as_str())
    }

    fn open_person(&self) -> Option<&Person> {
        self.open
            .and_then(|id| self.people.iter().find(|person| person.id == id))
    }
}

impl Application for People {
    type Action = PeopleAct;
    type Intent = Intent;
    type UiState = ();

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("People")
    }

    fn availability(&self, target: ActionTarget<PeopleAct>) -> Availability {
        match target.action {
            PeopleAct::Back | PeopleAct::Details if self.open.is_none() => Availability::Hidden,
            _ => Availability::Enabled,
        }
    }

    fn availability_with(
        &self,
        _target: ActionTarget<PeopleAct>,
        argument: &ActionArgument,
    ) -> Availability {
        match self.person(argument) {
            Some(_) => Availability::Enabled,
            None => Availability::Hidden,
        }
    }

    fn ui(&self, ui: &mut Ui, _view: &mut (), frame: &mut Frame<'_, PeopleAct, Intent>) {
        let mut p = frame.presentation(ui, PresentationScope::new("people"), SemanticUiId::root());
        let open = self.open_person();
        let about = open.map(|person| format!("About {}", person.name));
        let spec = NavigationSpec {
            list_title: "People",
            content: open.map(|person| NavigationContent {
                title: person.name,
                reference: Some(DomainReference::External {
                    namespace: "people".into(),
                    id: person.id.into(),
                }),
                detail: about.as_deref().map(|title| NavigationDetail {
                    title,
                    shown: self.details,
                    toggle: PeopleAct::Details,
                    icon: IconId::Info,
                }),
            }),
            back: PeopleAct::Back,
        };
        frame.navigation(
            ui,
            &mut p,
            "navigation",
            &spec,
            |ui, p, frame, pane| match pane {
                NavigationPane::List => {
                    egui::ScrollArea::vertical()
                        .auto_shrink(false)
                        .show(ui, |ui| {
                            for person in &self.people {
                                frame.navigation_item(
                                    ui,
                                    p,
                                    BoundAction::new(PeopleAct::Open, person.id),
                                    DestinationSpec {
                                        icon: IconId::User,
                                        label: person.name,
                                        description: "Open the conversation",
                                        selected: self.open == Some(person.id),
                                        badge: None,
                                    },
                                );
                            }
                        });
                }
                NavigationPane::Content => {
                    let name = open.map_or("", |person| person.name);
                    p.reading_text(
                        ui,
                        "messages",
                        &format!("Messages with {name}"),
                        ReadingTextSpec::default(),
                    );
                }
                NavigationPane::Detail => {
                    p.reading_text(ui, "about", "Joined in 1952", ReadingTextSpec::default());
                }
                NavigationPane::Empty => {
                    p.reading_text(ui, "empty", "Choose a person", ReadingTextSpec::default());
                }
            },
        );
    }

    fn apply(&mut self, intent: Intent, _view: &mut ()) -> Result<bool, String> {
        match intent {}
    }

    fn perform(&mut self, target: ActionTarget<PeopleAct>, _view: &mut ()) -> Result<bool, String> {
        self.performed.push((target.action, ActionArgument::None));
        match target.action {
            PeopleAct::Back if self.details => self.details = false,
            PeopleAct::Back => self.open = None,
            PeopleAct::Details => self.details = !self.details,
            PeopleAct::Open => return Err("Open needs a person".into()),
        }
        Ok(true)
    }

    fn perform_with(
        &mut self,
        target: ActionTarget<PeopleAct>,
        argument: ActionArgument,
        view: &mut (),
    ) -> Result<bool, String> {
        let Some(id) = self.person(&argument).map(|person| person.id) else {
            return self.perform(target, view);
        };
        self.performed.push((target.action, argument));
        self.open = Some(id);
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

const PHONE: f32 = 390.0;
const TABLET: f32 = 820.0;
const DESKTOP: f32 = 1280.0;

fn harness(width: f32) -> Harness<'static, Probe> {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(width, 760.0))
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
    harness.state_mut().host = Some(AppHost::new(&context, People::default()));
    harness.run();
    harness
}

fn host<'a>(harness: &'a Harness<'_, Probe>) -> &'a AppHost<People> {
    harness.state().host.as_ref().unwrap()
}

fn host_mut<'a>(harness: &'a mut Harness<'_, Probe>) -> &'a mut AppHost<People> {
    harness.state_mut().host.as_mut().unwrap()
}

/// The AccessKit tree as an update, for the parity audit.
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

fn assert_clean(harness: &Harness<'_, Probe>) {
    let snapshot = host(harness).ui_snapshot();
    assert!(
        snapshot.semantic_audit.is_empty(),
        "{:?}",
        snapshot.semantic_audit
    );
    assert!(snapshot.text_audit.is_empty(), "{:?}", snapshot.text_audit);
    let mismatches = crate::audit_accesskit(snapshot, &accesskit_update(harness));
    assert!(mismatches.is_empty(), "{mismatches:?}");
    let errors = &host(harness).app().errors;
    assert!(errors.is_empty(), "{errors:?}");
}

/// The panes' regions, by name, from the start of the line.
fn panes(harness: &Harness<'_, Probe>) -> Vec<String> {
    let mut panes: Vec<_> = host(harness)
        .ui_snapshot()
        .by_role(UiRole::Pane)
        .map(|node| (node.rect.min_x, node.name.clone()))
        .collect();
    panes.sort_by(|a, b| a.0.total_cmp(&b.0));
    panes.into_iter().map(|(_, name)| name).collect()
}

/// Which header controls show, by name.
fn controls(harness: &Harness<'_, Probe>) -> Vec<(String, Option<bool>)> {
    let snapshot = host(harness).ui_snapshot();
    [PeopleAct::Back, PeopleAct::Details]
        .into_iter()
        .flat_map(|action| snapshot.by_action(action))
        .map(|node| (node.name.clone(), node.checked))
        .collect()
}

/// Destinations shown, and which is current.
fn destinations(harness: &Harness<'_, Probe>) -> Vec<(String, bool)> {
    host(harness)
        .ui_snapshot()
        .by_action(PeopleAct::Open)
        .map(|node| (node.name.clone(), node.selected))
        .collect()
}

fn run(harness: &mut Harness<'_, Probe>) {
    harness.run();
    assert_clean(harness);
}

#[test]
fn the_layout_follows_the_width_at_each_breakpoint() {
    for (width, layout) in [
        (320.0, NavigationLayout::Stack),
        (719.9, NavigationLayout::Stack),
        (720.0, NavigationLayout::Split),
        (1039.9, NavigationLayout::Split),
        (1040.0, NavigationLayout::Columns),
        (2560.0, NavigationLayout::Columns),
    ] {
        assert_eq!(NavigationLayout::for_width(width), layout, "{width}");
    }
    // The split starts where the wide pane class does.
    assert_eq!(
        crate::PaneWidthClass::from_points(NavigationLayout::SPLIT_FROM),
        crate::PaneWidthClass::Wide
    );
    assert_eq!(
        crate::PaneWidthClass::from_points(NavigationLayout::SPLIT_FROM - 0.1),
        crate::PaneWidthClass::Regular
    );
}

#[test]
fn a_phone_stacks_one_level_at_a_time_with_back() {
    let mut harness = harness(PHONE);
    assert_clean(&harness);
    // The list is the root, with no Back.
    assert_eq!(panes(&harness), ["People"]);
    assert!(controls(&harness).is_empty());
    assert_eq!(
        destinations(&harness),
        [
            ("Ada".into(), false),
            ("Grace".into(), false),
            ("Katherine".into(), false)
        ]
    );
    // Opening one pushes it over the list, with Back and the detail toggle.
    harness.get_by_label("Grace").click();
    run(&mut harness);
    assert_eq!(panes(&harness), ["Grace"]);
    assert!(destinations(&harness).is_empty());
    assert_eq!(
        controls(&harness),
        [("Back".into(), None), ("Details".into(), Some(false))]
    );
    let content = host(&harness)
        .ui_snapshot()
        .by_role(UiRole::Pane)
        .next()
        .unwrap()
        .clone();
    assert!(
        content.rect.max_x - content.rect.min_x > PHONE - 40.0,
        "{content:?}"
    );
    // The detail is a further push.
    harness.get_by_label("Details").click();
    run(&mut harness);
    assert_eq!(panes(&harness), ["About Grace"]);
    assert_eq!(controls(&harness), [("Back".into(), None)]);
    // Back pops one level at a time, by pointer and by keyboard.
    harness.get_by_label("Back").click();
    run(&mut harness);
    assert_eq!(panes(&harness), ["Grace"]);
    harness.get_by_label("Back").focus();
    harness.run();
    harness.key_press(egui::Key::Enter);
    run(&mut harness);
    assert_eq!(panes(&harness), ["People"]);
    let app = host(&harness).app();
    assert_eq!(app.open, None);
    assert_eq!(
        app.performed,
        [
            (PeopleAct::Open, ActionArgument::Text("p2".into())),
            (PeopleAct::Details, ActionArgument::None),
            (PeopleAct::Back, ActionArgument::None),
            (PeopleAct::Back, ActionArgument::None),
        ]
    );
}

/// The control with keyboard focus, by name.
fn focused(harness: &Harness<'_, Probe>) -> Option<String> {
    host(harness)
        .ui_snapshot()
        .nodes
        .iter()
        .find(|node| node.focused)
        .map(|node| node.name.clone())
}

/// Activate the focused control with Enter.
fn enter(harness: &mut Harness<'_, Probe>) {
    harness.key_press(egui::Key::Enter);
    run(harness);
}

#[test]
fn keyboard_focus_follows_the_reader_through_the_levels() {
    // A phone: opening a row covers it, so focus moves to Back, and Back
    // returns focus to the row.
    let mut phone = harness(PHONE);
    phone.get_by_label("Grace").focus();
    phone.run();
    enter(&mut phone);
    assert_eq!(panes(&phone), ["Grace"]);
    assert_eq!(focused(&phone).as_deref(), Some("Back"));
    phone.get_by_label("Details").focus();
    phone.run();
    enter(&mut phone);
    assert_eq!(panes(&phone), ["About Grace"]);
    assert_eq!(focused(&phone).as_deref(), Some("Back"));
    enter(&mut phone);
    assert_eq!(panes(&phone), ["Grace"]);
    assert_eq!(focused(&phone).as_deref(), Some("Details"));
    phone.get_by_label("Back").focus();
    phone.run();
    enter(&mut phone);
    assert_eq!(panes(&phone), ["People"]);
    assert_eq!(focused(&phone).as_deref(), Some("Grace"));

    // A tablet: the row stays in view, so focus stays on it; the detail
    // covers the content's toggle, so focus moves to Back and returns.
    let mut tablet = harness(TABLET);
    tablet.get_by_label("Grace").focus();
    tablet.run();
    enter(&mut tablet);
    assert_eq!(panes(&tablet), ["People", "Grace"]);
    assert_eq!(focused(&tablet).as_deref(), Some("Grace"));
    tablet.get_by_label("Details").focus();
    tablet.run();
    enter(&mut tablet);
    assert_eq!(panes(&tablet), ["People", "About Grace"]);
    assert_eq!(focused(&tablet).as_deref(), Some("Back"));
    enter(&mut tablet);
    assert_eq!(panes(&tablet), ["People", "Grace"]);
    assert_eq!(focused(&tablet).as_deref(), Some("Details"));

    // An agent opening an item takes no focus that nobody had.
    let mut agent = harness(PHONE);
    host_mut(&mut agent).enable_inspection(Inspection::new("navigation-test"));
    agent.run();
    invoke(
        &mut agent,
        InspectionSelector {
            capability: Some(PeopleAct::Open.stable_id().into()),
            argument: Some(serde_json::json!("p1")),
            ..Default::default()
        },
        serde_json::Value::Null,
    );
    assert_eq!(panes(&agent), ["Ada"]);
    assert_eq!(focused(&agent), None);
}

#[test]
fn a_tablet_splits_the_list_beside_the_content() {
    let mut harness = harness(TABLET);
    assert_clean(&harness);
    // Nothing open: the list and the application's prompt, which is no region.
    assert_eq!(panes(&harness), ["People"]);
    assert!(
        host(&harness)
            .ui_snapshot()
            .content_text()
            .any(|text| text == "Choose a person")
    );
    harness.get_by_label("Grace").click();
    run(&mut harness);
    assert_eq!(panes(&harness), ["People", "Grace"]);
    // The open destination is marked current, and there is no Back.
    assert_eq!(
        destinations(&harness),
        [
            ("Ada".into(), false),
            ("Grace".into(), true),
            ("Katherine".into(), false)
        ]
    );
    assert_eq!(
        harness
            .get_by_role_and_label(egui::accesskit::Role::Button, "Grace")
            .accesskit_node()
            .aria_current(),
        Some(egui::accesskit::AriaCurrent::True)
    );
    assert_eq!(controls(&harness), [("Details".into(), Some(false))]);
    let snapshot = host(&harness).ui_snapshot();
    let list = snapshot
        .by_role(UiRole::Pane)
        .find(|node| node.name == "People")
        .unwrap();
    assert_eq!(
        list.rect.max_x - list.rect.min_x,
        NavigationLayout::LIST_WIDTH
    );
    // Another destination replaces the content, keeping the list.
    harness.get_by_label("Katherine").click();
    run(&mut harness);
    assert_eq!(panes(&harness), ["People", "Katherine"]);
    assert_eq!(
        destinations(&harness),
        [
            ("Ada".into(), false),
            ("Grace".into(), false),
            ("Katherine".into(), true)
        ]
    );
    // The detail takes the content's place, with Back to return to it.
    harness.get_by_label("Details").click();
    run(&mut harness);
    assert_eq!(panes(&harness), ["People", "About Katherine"]);
    assert_eq!(controls(&harness), [("Back".into(), None)]);
    harness.get_by_label("Back").click();
    run(&mut harness);
    assert_eq!(panes(&harness), ["People", "Katherine"]);
}

#[test]
fn a_desktop_shows_list_content_and_detail_side_by_side() {
    let mut harness = harness(DESKTOP);
    harness.get_by_label("Ada").click();
    run(&mut harness);
    assert_eq!(panes(&harness), ["People", "Ada"]);
    // The toggle shows the detail beside the content and hides it again;
    // no level covers another, so there is no Back.
    harness.get_by_label("Details").click();
    run(&mut harness);
    assert_eq!(panes(&harness), ["People", "Ada", "About Ada"]);
    assert_eq!(controls(&harness), [("Details".into(), Some(true))]);
    assert_eq!(
        harness.get_by_label("Details").accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    let snapshot = host(&harness).ui_snapshot();
    let width = |name: &str| {
        let rect = snapshot
            .by_role(UiRole::Pane)
            .find(|node| node.name == name)
            .unwrap()
            .rect;
        rect.max_x - rect.min_x
    };
    assert_eq!(width("People"), NavigationLayout::LIST_WIDTH);
    assert_eq!(width("About Ada"), NavigationLayout::DETAIL_WIDTH);
    assert!(width("Ada") >= NavigationLayout::CONTENT_MIN_WIDTH);
    harness.get_by_label("Details").click();
    run(&mut harness);
    assert_eq!(panes(&harness), ["People", "Ada"]);
}

#[test]
fn a_resize_across_the_breakpoints_keeps_what_is_open() {
    let mut harness = harness(DESKTOP);
    harness.get_by_label("Grace").click();
    harness.run();
    harness.get_by_label("Details").click();
    run(&mut harness);
    assert_eq!(panes(&harness), ["People", "Grace", "About Grace"]);
    let resize = |harness: &mut Harness<'_, Probe>, width: f32| {
        harness.set_size(Vec2::new(width, 760.0));
        harness.run();
        harness.run();
        assert_clean(harness);
        panes(harness)
    };
    // Narrower, the detail covers the content, then everything else.
    assert_eq!(resize(&mut harness, TABLET), ["People", "About Grace"]);
    assert_eq!(resize(&mut harness, PHONE), ["About Grace"]);
    // Back on the phone closes the detail, and wider again shows the
    // content open beside the list, with its destination still current.
    harness.get_by_label("Back").click();
    run(&mut harness);
    assert_eq!(panes(&harness), ["Grace"]);
    assert_eq!(resize(&mut harness, TABLET), ["People", "Grace"]);
    assert_eq!(resize(&mut harness, DESKTOP), ["People", "Grace"]);
    assert_eq!(
        destinations(&harness),
        [
            ("Ada".into(), false),
            ("Grace".into(), true),
            ("Katherine".into(), false)
        ]
    );
    let app = host(&harness).app();
    assert_eq!((app.open, app.details), (Some("p2"), false));
}

#[test]
fn the_list_keeps_its_scroll_position_across_levels_and_layouts() {
    let mut harness = harness(TABLET);
    let app = host_mut(&mut harness).app_mut();
    for index in 0..40 {
        app.people.push(Person {
            id: Box::leak(format!("q{index}").into_boxed_str()),
            name: Box::leak(format!("Person {index}").into_boxed_str()),
        });
    }
    host_mut(&mut harness).request_repaint("test");
    run(&mut harness);
    let top = |harness: &Harness<'_, Probe>, name: &str| {
        host(harness)
            .ui_snapshot()
            .by_action(PeopleAct::Open)
            .find(|row| row.name == name)
            .map(|row| row.rect.min_y)
    };
    let before = top(&harness, "Person 20");
    harness.hover_at(egui::pos2(100.0, 300.0));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: Vec2::new(0.0, -600.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    run(&mut harness);
    harness.run();
    let scrolled = top(&harness, "Person 20").expect("scrolled into view");
    assert!(
        before.is_none_or(|before| scrolled < before - 100.0),
        "{before:?} {scrolled}"
    );
    // Scrolled-out rows are not published.
    assert_eq!(top(&harness, "Ada"), None);
    // The phone's list root, and the list again after a push and Back, keep it.
    harness.set_size(Vec2::new(PHONE, 760.0));
    run(&mut harness);
    harness.run();
    assert_eq!(top(&harness, "Person 20"), Some(scrolled));
    harness.get_by_label("Person 20").click();
    run(&mut harness);
    assert_eq!(panes(&harness), ["Person 20"]);
    harness.get_by_label("Back").click();
    run(&mut harness);
    harness.run();
    assert_eq!(top(&harness, "Person 20"), Some(scrolled));
    harness.set_size(Vec2::new(DESKTOP, 760.0));
    run(&mut harness);
    harness.run();
    assert_eq!(top(&harness, "Person 20"), Some(scrolled));
}

fn request(
    harness: &Harness<'_, Probe>,
    id: &str,
    operation: InspectionOperation,
) -> InspectionResult {
    let inspection = host(harness).inspection().unwrap();
    let reply = inspection.handle(InspectionRequest {
        version: 1,
        request_id: id.into(),
        instance: Some(inspection.instance()),
        operation,
    });
    reply
        .result
        .unwrap_or_else(|| panic!("{id} failed: {:?}", reply.error))
}

/// The open item an agent reads: the content region's domain reference.
fn open_item(harness: &Harness<'_, Probe>) -> Option<DomainReference> {
    let InspectionResult::Query { nodes, .. } = request(
        harness,
        "query",
        InspectionOperation::Query {
            selector: InspectionSelector {
                role: Some(UiRole::Pane),
                ..Default::default()
            },
            limit: 8,
            cursor: None,
        },
    ) else {
        panic!("not a query");
    };
    nodes
        .into_iter()
        .find_map(|node| node.node.domain_reference)
}

/// Discover the one capability matching `selector` and invoke it.
fn invoke(
    harness: &mut Harness<'_, Probe>,
    selector: InspectionSelector,
    value: serde_json::Value,
) {
    let InspectionResult::Discover { capabilities, .. } = request(
        harness,
        "discover",
        InspectionOperation::Discover {
            selector,
            limit: 8,
            cursor: None,
        },
    ) else {
        panic!("not a discovery");
    };
    assert_eq!(capabilities.len(), 1, "{capabilities:?}");
    static INVOCATIONS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let id = INVOCATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let InspectionResult::Receipt { receipt } = request(
        harness,
        &format!("invoke-{id}"),
        InspectionOperation::Invoke {
            selector: InspectionSelector {
                id: Some(capabilities[0].id.clone()),
                ..Default::default()
            },
            expected: capabilities[0].target.clone(),
            arguments: value,
        },
    ) else {
        panic!("not a receipt");
    };
    assert_eq!(receipt.state, ReceiptState::Queued, "{receipt:?}");
    harness.run();
    harness.run();
    assert_clean(harness);
}

#[test]
fn agents_read_the_open_item_open_another_and_go_back() {
    for width in [PHONE, TABLET, DESKTOP] {
        let mut harness = harness(width);
        host_mut(&mut harness).enable_inspection(Inspection::new("navigation-test"));
        harness.run();
        assert_eq!(open_item(&harness), None, "{width}");
        // Open a rendered destination by its argument.
        invoke(
            &mut harness,
            InspectionSelector {
                capability: Some(PeopleAct::Open.stable_id().into()),
                argument: Some(serde_json::json!("p1")),
                ..Default::default()
            },
            serde_json::Value::Null,
        );
        let ada = DomainReference::External {
            namespace: "people".into(),
            id: "p1".into(),
        };
        assert_eq!(open_item(&harness), Some(ada.clone()), "{width}");
        // Open another while the phone's list is covered: the action's own
        // binding takes any person's ID.
        let selector = if width == PHONE {
            InspectionSelector {
                capability: Some(PeopleAct::Open.stable_id().into()),
                ..Default::default()
            }
        } else {
            InspectionSelector {
                capability: Some(PeopleAct::Open.stable_id().into()),
                argument: Some(serde_json::json!("p3")),
                ..Default::default()
            }
        };
        let value = if width == PHONE {
            serde_json::json!({ "value": "p3" })
        } else {
            serde_json::Value::Null
        };
        invoke(&mut harness, selector, value);
        let katherine = DomainReference::External {
            namespace: "people".into(),
            id: "p3".into(),
        };
        assert_eq!(open_item(&harness), Some(katherine), "{width}");
        // Back works in every layout, shown or not.
        invoke(
            &mut harness,
            InspectionSelector {
                capability: Some(PeopleAct::Back.stable_id().into()),
                ..Default::default()
            },
            serde_json::Value::Null,
        );
        assert_eq!(open_item(&harness), None, "{width}");
        assert_eq!(host(&harness).app().open, None);
    }
}

#[test]
fn panes_are_regions_whose_controls_belong_to_them() {
    let mut harness = harness(TABLET);
    harness.get_by_label("Ada").click();
    run(&mut harness);
    let snapshot = host(&harness).ui_snapshot();
    let content = snapshot
        .by_role(UiRole::Pane)
        .find(|node| node.name == "Ada")
        .unwrap();
    let toggle = snapshot.by_action(PeopleAct::Details).next().unwrap();
    assert_eq!(toggle.parent.as_ref(), Some(&content.id));
    assert!(
        snapshot
            .by_action(PeopleAct::Open)
            .all(|row| row.parent.as_ref() != Some(&content.id))
    );
    // AccessKit holds the toggle inside the content's region.
    let region = harness.get_by_role_and_label(egui::accesskit::Role::Region, "Ada");
    region.get_by_label("Details");
    assert!(region.query_by_label("Katherine").is_none());
}

/// A pane's header node and text rectangles at a phone's width.
fn header_geometry(harness: &Harness<'_, Probe>) -> (crate::UiRect, crate::UiRect, crate::UiRect) {
    let snapshot = host(harness).ui_snapshot();
    let pane = snapshot.by_role(UiRole::Pane).next().unwrap().rect;
    let back = snapshot.by_action(PeopleAct::Back).next().unwrap().rect;
    let toggle = snapshot.by_action(PeopleAct::Details).next().unwrap().rect;
    (pane, back, toggle)
}

#[test]
fn the_header_places_back_the_title_and_the_toggle_symmetrically() {
    let mut harness = harness(PHONE);
    harness.get_by_label("Ada").click();
    run(&mut harness);
    let (pane, back, toggle) = header_geometry(&harness);
    let heading = harness
        .get_by_role_and_label(egui::accesskit::Role::Heading, "Ada")
        .rect();
    // Back and the toggle sit the same distance in from the pane's edges,
    // and the title takes the room between them, a gap from each.
    let pad = back.min_x - pane.min_x;
    assert!(pad > 0.0, "{pad}");
    assert!((pane.max_x - toggle.max_x - pad).abs() < 0.01);
    let gap = heading.min.x - back.max_x;
    assert!(gap > 0.0, "{gap}");
    assert!((toggle.min_x - heading.max.x - gap).abs() < 0.01);
    // All three are centred on one line.
    let centre = |min: f32, max: f32| (min + max) / 2.0;
    let line = centre(back.min_y, back.max_y);
    assert!((centre(toggle.min_y, toggle.max_y) - line).abs() < 0.01);
    assert!((centre(heading.min.y, heading.max.y) - line).abs() < 0.51);
}

/// The measured text whose words begin with `prefix`.
fn text_rect(harness: &Harness<'_, Probe>, prefix: &str) -> crate::TextRect {
    host(harness)
        .ui_snapshot()
        .text
        .iter()
        .find(|text| {
            matches!(&text.content, Some(crate::TextContent::Excerpt { text, .. })
                if text.starts_with(prefix))
        })
        .unwrap_or_else(|| panic!("no text {prefix:?}"))
        .allocated_rect
}

#[test]
fn content_and_detail_bodies_are_inset_like_their_headers_and_the_list_is_not() {
    for width in [PHONE, DESKTOP] {
        let mut harness = harness(width);
        harness.get_by_label("Ada").click();
        run(&mut harness);
        harness.get_by_label("Details").click();
        run(&mut harness);
        let snapshot = host(&harness).ui_snapshot();
        let pane = |name: &str| {
            snapshot
                .by_role(UiRole::Pane)
                .find(|node| node.name == name)
                .unwrap()
                .rect
        };
        let back_or_toggle = snapshot
            .by_action(PeopleAct::Back)
            .chain(snapshot.by_action(PeopleAct::Details))
            .next()
            .unwrap()
            .rect;
        let header_pane = snapshot
            .by_role(UiRole::Pane)
            .find(|node| node.rect.contains(back_or_toggle, 0.0))
            .unwrap()
            .rect;
        // The header's controls sit this far in from the pane's edges.
        let pad = (back_or_toggle.min_x - header_pane.min_x)
            .min(header_pane.max_x - back_or_toggle.max_x);
        assert!(pad > 0.0, "{width}: {pad}");
        // The detail's body is inset by as much on each side.
        let detail = pane("About Ada");
        let about = text_rect(&harness, "Joined in 1952");
        assert!(
            (about.min_x - detail.min_x - pad).abs() < 0.01,
            "{width}: {about:?}"
        );
        assert!(
            (detail.max_x - about.max_x - pad).abs() < 0.01,
            "{width}: {about:?}"
        );
        if width == DESKTOP {
            // So is the content's, beside it; the list's rows fill its width.
            let content = pane("Ada");
            let messages = text_rect(&harness, "Messages with Ada");
            assert!((messages.min_x - content.min_x - pad).abs() < 0.01);
            assert!((content.max_x - messages.max_x - pad).abs() < 0.01);
            let list = pane("People");
            let row = snapshot.by_action(PeopleAct::Open).next().unwrap().rect;
            assert!((row.min_x - list.min_x).abs() < 0.01, "{row:?}");
        }
    }
}
