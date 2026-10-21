#![cfg(test)]
//! Segmented choices through the host: one action whose argument is the
//! option, dispatched by clicks, keys and agents alike, with one control per
//! option.

use egui::{Key, Modifiers, Vec2};
use egui_kittest::{Harness, kittest::Queryable};
use serde::Serialize;

use super::*;
use crate::{
    Action, BoundAction, CapabilityDescription, ChoiceArgument, ChoiceOption, InspectionErrorCode,
    InspectionOperation, InspectionResult, InspectionSelector, InvocationReceipt,
    PresentationScope, ReceiptState, ResolvedTarget, SegmentedChoiceSpec,
};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Repeat {
    Daily,
    Weekdays,
    Weekly,
    Hourly,
}

impl ChoiceArgument for Repeat {
    const CHOICES: &'static [(Self, &'static str, &'static str)] = &[
        (Self::Daily, "daily", "Daily"),
        (Self::Weekdays, "weekdays", "Weekdays"),
        (Self::Weekly, "weekly", "Weekly"),
        (Self::Hourly, "hourly", "Hourly"),
    ];
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Day {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

impl ChoiceArgument for Day {
    const CHOICES: &'static [(Self, &'static str, &'static str)] = &[
        (Self::Mon, "mon", "Monday"),
        (Self::Tue, "tue", "Tuesday"),
        (Self::Wed, "wed", "Wednesday"),
        (Self::Thu, "thu", "Thursday"),
        (Self::Fri, "fri", "Friday"),
        (Self::Sat, "sat", "Saturday"),
        (Self::Sun, "sun", "Sunday"),
    ];
}

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "routine")]
enum Act {
    #[action(label = "Save", description = "Save the routine")]
    Save,
    #[action(
        label = "Repeats",
        description = "Choose how often the routine runs",
        argument = "choice",
        choices = Repeat
    )]
    Repeat,
    #[action(
        label = "Runs on",
        description = "Run the routine on this day, or stop",
        argument = "choice",
        choices = Day
    )]
    Day,
    #[action(label = "Done", description = "Close the routine")]
    Done,
    #[action(
        label = "Edit",
        description = "Edit the routine with this ID",
        argument = "number",
        minimum = 1,
        maximum = 100,
        integer = true
    )]
    Edit,
}

struct Routine {
    repeat: Repeat,
    days: [bool; 7],
    /// Every performed action with its argument, in order.
    performed: Vec<(Act, ActionArgument)>,
    errors: Vec<String>,
    /// Sunday's option is hidden.
    hide_sunday: bool,
}

impl Default for Routine {
    fn default() -> Self {
        Self {
            repeat: Repeat::Daily,
            days: [false; 7],
            performed: Vec::new(),
            errors: Vec::new(),
            hide_sunday: false,
        }
    }
}

impl Application for Routine {
    type Action = Act;
    type Intent = ();
    type UiState = ();

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("Routine")
    }

    fn availability(&self, _target: ActionTarget<Act>) -> Availability {
        Availability::Enabled
    }

    fn availability_with(
        &self,
        target: ActionTarget<Act>,
        argument: &ActionArgument,
    ) -> Availability {
        match target.action {
            Act::Repeat if argument.as_choice::<Repeat>() == Ok(Repeat::Hourly) => {
                Availability::Disabled {
                    reason: "Hourly runs are turned off".into(),
                }
            }
            Act::Day if self.hide_sunday && argument.as_choice::<Day>() == Ok(Day::Sun) => {
                Availability::Hidden
            }
            _ => Availability::Enabled,
        }
    }

    fn ui(&self, ui: &mut Ui, _view: &mut (), frame: &mut Frame<'_, Act, ()>) {
        let mut p = frame.presentation(ui, PresentationScope::new("routine"), SemanticUiId::root());
        frame.action(ui, &mut p, Act::Save, ActionEmphasis::Normal);
        let repeats = ChoiceOption::choices::<Repeat>(|repeat| repeat == self.repeat);
        frame.segmented_choice(
            ui,
            &mut p,
            Act::Repeat,
            &SegmentedChoiceSpec::single("Repeats", &repeats),
        );
        let days: Vec<_> = Day::CHOICES
            .iter()
            .enumerate()
            .map(|(index, (day, _, label))| {
                ChoiceOption::choice(*day, self.days[index]).with_short_label(&label[..3])
            })
            .collect();
        frame.segmented_choice(
            ui,
            &mut p,
            Act::Day,
            &SegmentedChoiceSpec::multiple("Runs on", &days),
        );
        // A bound button with its own short label.
        frame.action(
            ui,
            &mut p,
            BoundAction::new(Act::Edit, 7_u32)
                .with_label("Edit")
                .with_subject("Morning news"),
            ActionEmphasis::Normal,
        );
        frame.action(ui, &mut p, Act::Done, ActionEmphasis::Normal);
    }

    fn apply(&mut self, _intent: (), _view: &mut ()) -> Result<bool, String> {
        Ok(false)
    }

    fn perform(&mut self, target: ActionTarget<Act>, _view: &mut ()) -> Result<bool, String> {
        self.performed.push((target.action, ActionArgument::None));
        match target.action {
            Act::Save | Act::Done => Ok(true),
            _ => Err("Choose an option".into()),
        }
    }

    fn perform_with(
        &mut self,
        target: ActionTarget<Act>,
        argument: ActionArgument,
        _view: &mut (),
    ) -> Result<bool, String> {
        self.performed.push((target.action, argument.clone()));
        match target.action {
            Act::Repeat => self.repeat = argument.as_choice()?,
            Act::Day => {
                let day: Day = argument.as_choice()?;
                let index = Day::CHOICES
                    .iter()
                    .position(|(option, ..)| *option == day)
                    .expect("a listed day");
                self.days[index] = !self.days[index];
            }
            Act::Edit => {}
            Act::Save | Act::Done => return Err("No argument expected".into()),
        }
        Ok(true)
    }

    fn report_error(&mut self, message: String) {
        self.errors.push(message);
    }
}

#[derive(Default)]
struct Probe {
    host: Option<AppHost<Routine>>,
}

fn harness_with(routine: Routine, width: f32) -> Harness<'static, Probe> {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(width, 600.0))
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
    harness.state_mut().host = Some(AppHost::new(&context, routine));
    harness.run();
    harness
}

fn harness(routine: Routine) -> Harness<'static, Probe> {
    harness_with(routine, 700.0)
}

fn host<'a>(harness: &'a Harness<'_, Probe>) -> &'a AppHost<Routine> {
    harness.state().host.as_ref().unwrap()
}

fn host_mut<'a>(harness: &'a mut Harness<'_, Probe>) -> &'a mut AppHost<Routine> {
    harness.state_mut().host.as_mut().unwrap()
}

/// The name of the focused node, if any.
fn focused(harness: &Harness<'_, Probe>) -> Option<String> {
    host(harness)
        .ui_snapshot()
        .nodes
        .iter()
        .find(|node| node.focused)
        .map(|node| node.name.clone())
}

fn press(harness: &mut Harness<'_, Probe>, key: Key, modifiers: Modifiers) {
    harness.key_press_modifiers(modifiers, key);
    harness.run();
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

fn with_inspection(harness: &mut Harness<'_, Probe>) {
    host_mut(harness).enable_inspection(Inspection::new("choice-test"));
    harness.run();
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

fn discover(harness: &Harness<'_, Probe>, action: Act) -> Vec<CapabilityDescription> {
    let reply = request(
        harness,
        "discover",
        InspectionOperation::Discover {
            selector: InspectionSelector {
                capability: Some(action.stable_id().into()),
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

fn receipt(reply: InspectionReply) -> InvocationReceipt {
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("receipt failed: {:?}", reply.error);
    };
    receipt
}

fn invoke(
    harness: &mut Harness<'_, Probe>,
    id: &str,
    action: Act,
    expected: ResolvedTarget,
    value: &str,
) -> InvocationReceipt {
    let admitted = receipt(request(
        harness,
        id,
        InspectionOperation::Invoke {
            selector: InspectionSelector {
                capability: Some(action.stable_id().into()),
                argument: Some(serde_json::json!(value)),
                ..Default::default()
            },
            expected,
            arguments: serde_json::json!({ "value": value }),
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

#[test]
fn clicking_an_option_performs_the_action_with_its_argument() {
    let mut harness = harness(Routine::default());
    harness.get_by_label("Weekly").click();
    harness.run();
    assert_eq!(host(&harness).app().repeat, Repeat::Weekly);
    // A multiple choice's options are named by their full labels and
    // toggle on their own.
    harness.get_by_label("Tuesday").click();
    harness.run();
    harness.get_by_label("Friday").click();
    harness.run();
    harness.get_by_label("Tuesday").click();
    harness.run();
    let routine = host(&harness).app();
    assert_eq!(
        routine.performed,
        [
            (Act::Repeat, ActionArgument::Choice("weekly".into())),
            (Act::Day, ActionArgument::Choice("tue".into())),
            (Act::Day, ActionArgument::Choice("fri".into())),
            (Act::Day, ActionArgument::Choice("tue".into())),
        ]
    );
    assert_eq!(
        routine.days,
        [false, false, false, false, true, false, false]
    );
    assert!(routine.errors.is_empty(), "{:?}", routine.errors);
    // The disabled option takes no click.
    harness.get_by_label("Hourly").click();
    harness.run();
    assert_eq!(host(&harness).app().repeat, Repeat::Weekly);
    assert_eq!(host(&harness).app().performed.len(), 4);
}

#[test]
fn options_publish_their_roles_states_and_arguments() {
    let mut routine = Routine {
        repeat: Repeat::Weekdays,
        ..Routine::default()
    };
    routine.days[0] = true;
    let harness = harness(routine);
    let snapshot = host(&harness).ui_snapshot();
    assert!(
        snapshot.semantic_audit.is_empty(),
        "{:?}",
        snapshot.semantic_audit
    );
    let groups: Vec<_> = snapshot.by_role(UiRole::Section).collect();
    assert_eq!(
        groups
            .iter()
            .map(|node| node.name.as_str())
            .collect::<Vec<_>>(),
        ["Repeats", "Runs on"]
    );
    let radios: Vec<_> = snapshot.by_action(Act::Repeat).collect();
    assert!(radios.iter().all(
        |node| node.role == UiRole::RadioButton && node.parent.as_ref() == Some(&groups[0].id)
    ));
    assert_eq!(
        radios
            .iter()
            .map(|node| (node.name.as_str(), node.checked, node.argument.clone()))
            .collect::<Vec<_>>(),
        [
            ("Daily", Some(false), Some(serde_json::json!("daily"))),
            ("Weekdays", Some(true), Some(serde_json::json!("weekdays"))),
            ("Weekly", Some(false), Some(serde_json::json!("weekly"))),
            ("Hourly", Some(false), Some(serde_json::json!("hourly"))),
        ]
    );
    assert_eq!(
        radios[3].disabled_reason.as_deref(),
        Some("Hourly runs are turned off")
    );
    let days: Vec<_> = snapshot.by_action(Act::Day).collect();
    assert_eq!(days.len(), 7);
    assert!(
        days.iter()
            .all(|node| node.role == UiRole::Button && node.parent.as_ref() == Some(&groups[1].id))
    );
    assert_eq!(days[0].checked, Some(true));
    assert_eq!(days[1].name, "Tuesday");
    // The bound button is named by its own label and subject.
    let edit: Vec<_> = snapshot.by_action(Act::Edit).collect();
    assert_eq!(edit[0].name, "Edit Morning news");
    // Identities are stable from pass to pass.
    let ids: Vec<_> = snapshot.nodes.iter().map(|node| node.id.clone()).collect();
    let unique: std::collections::BTreeSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len());
    let mismatches = crate::audit_accesskit(snapshot, &accesskit_update(&harness));
    assert!(mismatches.is_empty(), "{mismatches:?}");
}

#[test]
fn tab_enters_a_single_choice_at_its_chosen_option_and_arrows_choose() {
    let routine = Routine {
        repeat: Repeat::Weekdays,
        ..Routine::default()
    };
    let mut harness = harness(routine);
    harness.get_by_label("Save").focus();
    harness.run();
    assert_eq!(focused(&harness).as_deref(), Some("Save"));
    press(&mut harness, Key::Tab, Modifiers::NONE);
    assert_eq!(focused(&harness).as_deref(), Some("Weekdays"));
    // Arrows move and choose, skipping the disabled option and wrapping.
    press(&mut harness, Key::ArrowRight, Modifiers::NONE);
    assert_eq!(focused(&harness).as_deref(), Some("Weekly"));
    assert_eq!(host(&harness).app().repeat, Repeat::Weekly);
    press(&mut harness, Key::ArrowRight, Modifiers::NONE);
    assert_eq!(focused(&harness).as_deref(), Some("Daily"));
    assert_eq!(host(&harness).app().repeat, Repeat::Daily);
    press(&mut harness, Key::ArrowLeft, Modifiers::NONE);
    assert_eq!(focused(&harness).as_deref(), Some("Weekly"));
    press(&mut harness, Key::ArrowUp, Modifiers::NONE);
    assert_eq!(focused(&harness).as_deref(), Some("Weekdays"));
    press(&mut harness, Key::ArrowDown, Modifiers::NONE);
    assert_eq!(focused(&harness).as_deref(), Some("Weekly"));
    press(&mut harness, Key::Home, Modifiers::NONE);
    assert_eq!(focused(&harness).as_deref(), Some("Daily"));
    press(&mut harness, Key::End, Modifiers::NONE);
    assert_eq!(focused(&harness).as_deref(), Some("Weekly"));
    assert_eq!(host(&harness).app().repeat, Repeat::Weekly);
    // Tab leaves the group for the next one, whose options are each a stop.
    press(&mut harness, Key::Tab, Modifiers::NONE);
    assert_eq!(focused(&harness).as_deref(), Some("Monday"));
    press(&mut harness, Key::Tab, Modifiers::NONE);
    assert_eq!(focused(&harness).as_deref(), Some("Tuesday"));
    // Space toggles the focused option; arrows do not move within it.
    press(&mut harness, Key::Space, Modifiers::NONE);
    assert!(host(&harness).app().days[1]);
    // Shift+Tab returns to the single choice at its chosen option.
    press(&mut harness, Key::Tab, Modifiers::SHIFT);
    press(&mut harness, Key::Tab, Modifiers::SHIFT);
    assert_eq!(focused(&harness).as_deref(), Some("Weekly"));
    press(&mut harness, Key::Tab, Modifiers::SHIFT);
    assert_eq!(focused(&harness).as_deref(), Some("Save"));
    let routine = host(&harness).app();
    assert!(routine.errors.is_empty(), "{:?}", routine.errors);
    assert_eq!(
        routine
            .performed
            .iter()
            .filter(|(action, _)| *action == Act::Repeat)
            .map(|(_, argument)| argument.clone())
            .collect::<Vec<_>>(),
        [
            "weekly", "daily", "weekly", "weekdays", "weekly", "daily", "weekly"
        ]
        .map(|id| ActionArgument::Choice(id.into()))
    );
}

#[test]
fn agents_see_one_control_per_option_and_invoke_each() {
    let routine = Routine {
        hide_sunday: true,
        ..Routine::default()
    };
    let mut harness = harness(routine);
    with_inspection(&mut harness);
    let repeats = discover(&harness, Act::Repeat);
    let bound: Vec<_> = repeats
        .iter()
        .filter(|binding| binding.argument.is_some())
        .map(|binding| {
            (
                binding.argument.clone().unwrap(),
                binding.availability.clone(),
            )
        })
        .collect();
    assert_eq!(
        bound,
        [
            ("daily", Availability::Enabled),
            ("weekdays", Availability::Enabled),
            ("weekly", Availability::Enabled),
            (
                "hourly",
                Availability::Disabled {
                    reason: "Hourly runs are turned off".into()
                }
            ),
        ]
        .map(|(id, availability)| (serde_json::json!(id), availability))
    );
    // The hidden option is no control.
    let days: Vec<_> = discover(&harness, Act::Day)
        .into_iter()
        .filter_map(|binding| binding.argument)
        .collect();
    assert_eq!(
        days,
        ["mon", "tue", "wed", "thu", "fri", "sat"].map(|id| serde_json::json!(id))
    );
    let weekly = repeats
        .iter()
        .find(|binding| binding.argument == Some(serde_json::json!("weekly")))
        .unwrap()
        .target
        .clone();
    let done = invoke(&mut harness, "weekly", Act::Repeat, weekly, "weekly");
    assert_eq!(done.state, ReceiptState::Completed, "{done:?}");
    assert_eq!(host(&harness).app().repeat, Repeat::Weekly);
    let hourly = repeats
        .iter()
        .find(|binding| binding.argument == Some(serde_json::json!("hourly")))
        .unwrap()
        .target
        .clone();
    let refused = invoke(&mut harness, "hourly", Act::Repeat, hourly, "hourly");
    assert_eq!(refused.state, ReceiptState::Rejected);
    assert_eq!(
        refused.error.map(|error| error.code),
        Some(InspectionErrorCode::Unavailable)
    );
    assert_eq!(host(&harness).app().repeat, Repeat::Weekly);
}

#[test]
fn weekday_options_fit_a_phone_width_without_cutting_labels() {
    let harness = harness_with(Routine::default(), 390.0);
    let snapshot = host(&harness).ui_snapshot();
    assert!(snapshot.text_audit.is_empty(), "{:?}", snapshot.text_audit);
    assert!(snapshot.text.iter().all(|text| !text.truncated));
    let days: Vec<_> = snapshot.by_action(Act::Day).collect();
    assert_eq!(days.len(), 7);
    for day in &days {
        let (width, height) = (
            day.rect.max_x - day.rect.min_x,
            day.rect.max_y - day.rect.min_y,
        );
        assert!(day.rect.max_x <= 390.0, "{day:?}");
        assert!(width >= 32.0 && height >= 32.0, "{day:?}");
    }
}
