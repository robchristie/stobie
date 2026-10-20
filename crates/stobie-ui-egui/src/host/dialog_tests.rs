#![cfg(test)]
//! Modal dialogs, sheets and confirms through the host: focus moves in, stays
//! in and returns to the opener; Escape and the close control dismiss; and
//! nothing behind a dialog is reachable by a pointer, the keyboard, an agent
//! or assistive technology.

use egui::{Key, Modifiers, Vec2};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use serde::Serialize;

use super::*;
use crate::{
    Action, CapabilityDescription, ConfirmSpec, DialogPlacement, DialogSpec, InspectionErrorCode,
    InspectionOperation, InspectionResult, InspectionSelector, InvocationReceipt,
    NativeTextControlKind, NativeTextEdit, PresentationContext, PresentationScope, ReceiptState,
    ResolvedTarget,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "planner")]
enum Act {
    #[action(label = "Edit routine", description = "Edit the morning routine")]
    Edit,
    #[action(label = "Sign out", description = "Sign out of the planner")]
    SignOut,
    #[action(
        label = "Refresh",
        description = "Refresh the routines",
        shortcut = "Cmd+R"
    )]
    Refresh,
    #[action(label = "Close", description = "Close the editor")]
    Close,
    #[action(label = "Save", description = "Save the routine", shortcut = "Cmd+S")]
    Save,
    #[action(label = "Sign out now", description = "Sign out of the planner now")]
    ConfirmSignOut,
    #[action(label = "Keep signed in", description = "Stay signed in")]
    KeepSignedIn,
}

enum Intent {
    Search(String),
    Name(String),
    Prompt(String),
}

/// A list of routines behind an editor sheet and a sign-out confirm.
#[derive(Default)]
struct Planner {
    search: String,
    name: String,
    prompt: String,
    editing: bool,
    confirming: bool,
    /// The editor is a full-height sheet.
    full: bool,
    /// The editor is a centred dialog at every width.
    centred: bool,
    /// The editor is saving, so it cannot be closed.
    saving: bool,
    /// The editor's form is taller than the screen, so Save scrolls away.
    long: bool,
    refreshed: u32,
    signed_out: bool,
    saved: Vec<(String, String)>,
    performed: Vec<Act>,
}

#[derive(Default)]
struct View {
    placement: Option<DialogPlacement>,
    rect: Option<Rect>,
}

const NAME: &str = "planner.name";
const PROMPT: &str = "planner.prompt";
const SEARCH: &str = "planner.search";

/// A labelled text field presented as Stobie recipes present one.
fn field(
    ui: &mut Ui,
    p: &mut PresentationContext,
    id: &'static str,
    label: &str,
    text: &str,
    native: bool,
) -> Option<String> {
    let mut edited = text.to_owned();
    let (response, ()) = p.native(ui, NativeTextControlKind::TextEdit, |ui| {
        let response = if native {
            NativeTextEdit::multiline(&mut edited, egui::Id::new(id))
                .desired_rows(2)
                .show(ui)
        } else {
            ui.add(egui::TextEdit::singleline(&mut edited).id(egui::Id::new(id)))
        };
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_label(label);
            node.set_author_id(id.to_owned());
        });
        (response, ())
    });
    let mut node = UiNode::container(
        SemanticUiId::new(id),
        Some(p.parent().clone()),
        if native {
            UiRole::MultilineTextInput
        } else {
            UiRole::TextInput
        },
        response.rect.into(),
    );
    node.name = label.into();
    node.focused = response.has_focus();
    p.observe_node(ui, node);
    (edited != text).then_some(edited)
}

impl Application for Planner {
    type Action = Act;
    type Intent = Intent;
    type UiState = View;

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("Planner")
    }

    fn availability(&self, target: ActionTarget<Act>) -> Availability {
        match target.action {
            Act::Close if self.saving => Availability::Disabled {
                reason: "The routine is saving".into(),
            },
            _ => Availability::Enabled,
        }
    }

    fn ui(&self, ui: &mut Ui, view: &mut View, frame: &mut Frame<'_, Act, Intent>) {
        let mut p = frame.presentation(ui, PresentationScope::new("planner"), SemanticUiId::root());
        frame.action(ui, &mut p, Act::Edit, ActionEmphasis::Normal);
        frame.action(ui, &mut p, Act::SignOut, ActionEmphasis::Normal);
        frame.action(ui, &mut p, Act::Refresh, ActionEmphasis::Normal);
        if let Some(search) = field(ui, &mut p, SEARCH, "Search", &self.search, false) {
            frame.emit(Intent::Search(search));
        }
        view.placement = None;
        view.rect = None;
        if self.editing {
            let mut spec = DialogSpec::new("Edit routine", Act::Close);
            if !self.centred {
                spec = spec.sheet();
            }
            if self.full {
                spec = spec.full_height();
            }
            let response = frame.dialog(ui, &mut p, "editor", &spec, |ui, frame, p| {
                if let Some(name) = field(ui, p, NAME, "Name", &self.name, false) {
                    frame.emit(Intent::Name(name));
                }
                if let Some(prompt) = field(ui, p, PROMPT, "Prompt", &self.prompt, true) {
                    frame.emit(Intent::Prompt(prompt));
                }
                if self.long {
                    ui.add_space(2000.0);
                }
                frame.action(ui, p, Act::Save, ActionEmphasis::Primary);
            });
            view.placement = Some(response.placement);
            view.rect = Some(response.rect);
        }
        if self.confirming {
            let response = frame.confirm(
                ui,
                &mut p,
                "sign-out",
                &ConfirmSpec::new(
                    "Sign out?",
                    "You will need to sign in again to see your routines.",
                    Act::ConfirmSignOut,
                    Act::KeepSignedIn,
                ),
            );
            view.placement = Some(DialogPlacement::Centred);
            view.rect = Some(response.rect);
        }
    }

    fn apply(&mut self, intent: Intent, _: &mut View) -> Result<bool, String> {
        match intent {
            Intent::Search(text) => self.search = text,
            Intent::Name(text) => self.name = text,
            Intent::Prompt(text) => self.prompt = text,
        }
        Ok(true)
    }

    fn perform(&mut self, target: ActionTarget<Act>, _: &mut View) -> Result<bool, String> {
        self.performed.push(target.action);
        match target.action {
            Act::Edit => self.editing = true,
            Act::SignOut => self.confirming = true,
            Act::Refresh => self.refreshed += 1,
            Act::Close => self.editing = false,
            Act::Save => {
                self.saved.push((self.name.clone(), self.prompt.clone()));
                self.editing = false;
            }
            Act::ConfirmSignOut => {
                self.signed_out = true;
                self.confirming = false;
            }
            Act::KeepSignedIn => self.confirming = false,
        }
        Ok(true)
    }

    fn report_error(&mut self, message: String) {
        panic!("{message}");
    }
}

#[derive(Default)]
struct Probe {
    host: Option<AppHost<Planner>>,
}

const DESKTOP: Vec2 = Vec2::new(1280.0, 800.0);
const PHONE: Vec2 = Vec2::new(390.0, 844.0);

fn harness_at(planner: Planner, size: Vec2) -> Harness<'static, Probe> {
    let mut harness = Harness::builder().with_size(size).build_ui_state(
        |ui, probe: &mut Probe| {
            if let Some(host) = &mut probe.host {
                host.present(ui);
            }
        },
        Probe::default(),
    );
    crate::install_typography_fonts(&harness.ctx);
    let context = harness.ctx.clone();
    let mut host = AppHost::new(&context, planner);
    host.enable_inspection(Inspection::new("planner-test"));
    harness.state_mut().host = Some(host);
    harness.run();
    harness
}

fn harness() -> Harness<'static, Probe> {
    harness_at(Planner::default(), DESKTOP)
}

fn host<'a>(harness: &'a Harness<'_, Probe>) -> &'a AppHost<Planner> {
    harness.state().host.as_ref().unwrap()
}

fn app<'a>(harness: &'a Harness<'_, Probe>) -> &'a Planner {
    host(harness).app()
}

fn view<'a>(harness: &'a Harness<'_, Probe>) -> &'a View {
    host(harness).ui_state()
}

fn action_node(action: Act) -> SemanticUiId {
    PresentationScope::new("planner")
        .instance(action)
        .semantic_id()
}

/// The label of the control with keyboard focus, as assistive technology
/// names it.
fn focused(harness: &Harness<'_, Probe>) -> Option<String> {
    let widget = harness.ctx.memory(|memory| memory.focused())?;
    let node = widget.accesskit_id();
    harness
        .query_by(|candidate| candidate.locate().0 == node)
        .map(|found| found.accesskit_node().label().unwrap_or_default())
}

fn focus(label: &str) -> Option<String> {
    Some(label.to_owned())
}

fn press(harness: &mut Harness<'_, Probe>, modifiers: Modifiers, key: Key) {
    harness.key_press_modifiers(modifiers, key);
    harness.run();
}

fn open_editor(harness: &mut Harness<'_, Probe>) {
    harness.get_by_label("Edit routine").click();
    harness.run();
    assert!(app(harness).editing);
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

/// Admit an invocation, run passes until it has drained, and read its
/// receipt or the error that refused it.
fn invoke(
    harness: &mut Harness<'_, Probe>,
    id: &str,
    selector: InspectionSelector,
    expected: ResolvedTarget,
    arguments: serde_json::Value,
) -> Result<InvocationReceipt, crate::InspectionError> {
    let reply = request(
        harness,
        id,
        InspectionOperation::Invoke {
            selector,
            expected,
            arguments,
        },
    );
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        return Err(reply.error.expect("a reply without a receipt has an error"));
    };
    if receipt.state == ReceiptState::Rejected {
        return Err(receipt.error.expect("a rejected receipt has an error"));
    }
    if receipt.state != ReceiptState::Queued {
        return Ok(receipt);
    }
    harness.run();
    let reply = request(
        harness,
        &format!("{id}-receipt"),
        InspectionOperation::Receipt {
            request_id: id.into(),
        },
    );
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("receipt failed: {:?}", reply.error);
    };
    match receipt.state {
        ReceiptState::Rejected => Err(receipt.error.expect("a rejected receipt has an error")),
        _ => Ok(receipt),
    }
}

fn by_capability(action: Act) -> InspectionSelector {
    InspectionSelector {
        capability: Some(action.stable_id().into()),
        ..Default::default()
    }
}

fn by_id(id: &str) -> InspectionSelector {
    InspectionSelector {
        id: Some(SemanticUiId::new(id)),
        ..Default::default()
    }
}

/// Invoke `action` as `stobie invoke` does, from what discovery offers.
fn invoke_action(
    harness: &mut Harness<'_, Probe>,
    action: Act,
) -> Result<InvocationReceipt, crate::InspectionError> {
    let found = discover(harness, by_capability(action));
    let target = found
        .first()
        .unwrap_or_else(|| panic!("{action:?} is not offered"))
        .target
        .clone();
    let request = format!(
        "{}-{}",
        action.stable_id(),
        harness.ctx.cumulative_pass_nr()
    );
    invoke(
        harness,
        &request,
        by_capability(action),
        target,
        serde_json::Value::Null,
    )
}

/// Whether the AccessKit node labelled `label`, or an ancestor, is hidden.
fn hidden_from_assistive_technology(harness: &Harness<'_, Probe>, label: &str) -> bool {
    let node = harness.get_by_label(label);
    let mut current = Some(node.accesskit_node());
    while let Some(node) = current {
        if node.is_hidden() {
            return true;
        }
        current = node.parent();
    }
    false
}

fn assert_clean_audit(harness: &Harness<'_, Probe>) {
    let snapshot = host(harness).ui_snapshot();
    assert!(
        snapshot.semantic_audit.is_empty(),
        "{:?}",
        snapshot.semantic_audit
    );
    assert!(snapshot.audit().is_empty(), "{:?}", snapshot.audit());
}

#[test]
fn opening_a_dialog_moves_focus_to_its_first_control() {
    let mut harness = harness();
    open_editor(&mut harness);
    assert_eq!(focused(&harness), focus("Name"));
    let snapshot = host(&harness).ui_snapshot();
    let dialog = snapshot
        .nodes
        .iter()
        .find(|node| node.role == UiRole::Dialog)
        .expect("the dialog is observed");
    assert_eq!(dialog.name, "Edit routine");
    assert!(snapshot.node(&SemanticUiId::new(NAME)).unwrap().focused);
    assert_clean_audit(&harness);
}

#[test]
fn tab_and_shift_tab_stay_inside_the_dialog() {
    let mut harness = harness();
    open_editor(&mut harness);
    let inside = ["Name", "Prompt", "Save", "Close"];
    let mut forward = Vec::new();
    for _ in 0..inside.len() * 2 {
        press(&mut harness, Modifiers::NONE, Key::Tab);
        forward.push(focused(&harness).unwrap());
    }
    for widget in &forward {
        assert!(
            inside.contains(&widget.as_str()),
            "Tab left the dialog: {forward:?}"
        );
    }
    // Every control inside is reached, and Tab wraps.
    for widget in inside {
        assert!(
            forward.iter().any(|seen| seen == widget),
            "{widget} not reached"
        );
    }
    let mut backward = Vec::new();
    for _ in 0..inside.len() * 2 {
        press(&mut harness, Modifiers::SHIFT, Key::Tab);
        backward.push(focused(&harness).unwrap());
    }
    for widget in &backward {
        assert!(
            inside.contains(&widget.as_str()),
            "Shift+Tab left: {backward:?}"
        );
    }
}

#[test]
fn an_arrow_key_cannot_move_focus_behind_the_dialog() {
    let mut harness = harness();
    open_editor(&mut harness);
    // Save, then arrows in every direction: egui's directional focus can
    // pick any widget, and the host brings it back.
    press(&mut harness, Modifiers::NONE, Key::Tab);
    press(&mut harness, Modifiers::NONE, Key::Tab);
    assert_eq!(focused(&harness), focus("Save"));
    let behind = ["Edit routine", "Sign out", "Refresh", "Search"];
    for key in [
        Key::ArrowUp,
        Key::ArrowLeft,
        Key::ArrowDown,
        Key::ArrowRight,
    ] {
        press(&mut harness, Modifiers::NONE, key);
        harness.run();
        let now = focused(&harness);
        assert!(
            now.as_deref()
                .is_none_or(|widget| !behind.contains(&widget)),
            "{key:?} moved focus behind the dialog"
        );
        // Return on whatever has focus never activates what is behind.
        press(&mut harness, Modifiers::NONE, Key::Space);
        assert!(
            !app(&harness)
                .performed
                .iter()
                .skip(1)
                .any(|action| matches!(action, Act::Edit | Act::SignOut | Act::Refresh)),
            "{:?}",
            app(&harness).performed
        );
        if !app(&harness).editing {
            break;
        }
    }
}

#[test]
fn escape_dismisses_and_focus_returns_to_the_opener() {
    let mut harness = harness();
    // Open from the keyboard.
    press(&mut harness, Modifiers::NONE, Key::Tab);
    assert_eq!(focused(&harness), focus("Edit routine"));
    press(&mut harness, Modifiers::NONE, Key::Enter);
    harness.run();
    assert!(app(&harness).editing);
    assert_eq!(focused(&harness), focus("Name"));
    press(&mut harness, Modifiers::NONE, Key::Escape);
    harness.run();
    assert!(!app(&harness).editing);
    assert_eq!(app(&harness).performed, [Act::Edit, Act::Close]);
    assert_eq!(focused(&harness), focus("Edit routine"));
    let snapshot = host(&harness).ui_snapshot();
    assert!(
        snapshot
            .nodes
            .iter()
            .all(|node| node.role != UiRole::Dialog)
    );
    assert!(snapshot.node(&action_node(Act::Edit)).is_some());
}

#[test]
fn the_close_control_dismisses_and_focus_returns_to_the_opener() {
    let mut harness = harness();
    // Open with the pointer.
    open_editor(&mut harness);
    let close = harness.get_by_label("Close");
    assert_eq!(close.accesskit_node().role(), egui::accesskit::Role::Button);
    close.click();
    harness.run();
    assert!(!app(&harness).editing);
    assert_eq!(app(&harness).performed, [Act::Edit, Act::Close]);
    assert_eq!(focused(&harness), focus("Edit routine"));
}

#[test]
fn a_press_on_the_scrim_or_behind_the_dialog_does_nothing() {
    let mut harness = harness();
    open_editor(&mut harness);
    // Refresh's button lies under the scrim: the press reaches neither.
    let refresh = harness.get_by_label("Refresh").rect().center();
    harness.event(egui::Event::PointerMoved(refresh));
    harness.event(egui::Event::PointerButton {
        pos: refresh,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    harness.event(egui::Event::PointerButton {
        pos: refresh,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    harness.run();
    assert_eq!(app(&harness).refreshed, 0);
    assert!(app(&harness).editing, "the scrim does not dismiss a form");
}

#[test]
fn agents_reach_only_what_the_dialog_presents() {
    let mut harness = harness();
    open_editor(&mut harness);
    // Behind the dialog: offered as unavailable, with the reason, and
    // refused.
    for action in [Act::Edit, Act::SignOut, Act::Refresh] {
        let found = discover(&harness, by_capability(action));
        assert_eq!(found.len(), 1, "{action:?}: {found:?}");
        assert_eq!(
            found[0].availability.disabled_reason(),
            Some("The dialog \"Edit routine\" is open"),
            "{action:?}"
        );
        let error = invoke_action(&mut harness, action).unwrap_err();
        assert_eq!(error.code, InspectionErrorCode::Unavailable, "{action:?}");
    }
    assert_eq!(app(&harness).refreshed, 0);
    assert_eq!(app(&harness).performed, [Act::Edit]);
    // The background field is not offered for set-text.
    assert!(discover(&harness, by_id(SEARCH)).is_empty());
    // The dialog's own actions are.
    let close = discover(&harness, by_capability(Act::Close));
    assert!(close[0].availability.enabled(), "{close:?}");
    let receipt = invoke_action(&mut harness, Act::Close).unwrap();
    assert_eq!(receipt.state, ReceiptState::Completed, "{receipt:?}");
    assert!(!app(&harness).editing);
    // Closed, everything is reachable again.
    invoke_action(&mut harness, Act::Refresh).unwrap();
    assert_eq!(app(&harness).refreshed, 1);
}

#[test]
fn a_control_scrolled_out_of_the_dialog_stays_its_own() {
    let planner = Planner {
        long: true,
        ..Planner::default()
    };
    let mut harness = harness_at(planner, PHONE);
    open_editor(&mut harness);
    // Save is below the dialog's view, so the snapshot does not show it.
    let snapshot = host(&harness).ui_snapshot();
    assert!(
        snapshot.by_action(Act::Save).next().is_none(),
        "{:?}",
        snapshot.nodes
    );
    // It is still the dialog's: offered as enabled, not as behind it.
    let save = discover(&harness, by_capability(Act::Save));
    assert_eq!(save.len(), 1, "{save:?}");
    assert!(save[0].availability.enabled(), "{save:?}");
    // What is behind the dialog is still refused.
    let refresh = discover(&harness, by_capability(Act::Refresh));
    assert!(!refresh[0].availability.enabled(), "{refresh:?}");
    // Its shortcut reaches it, and so does an agent.
    press(&mut harness, Modifiers::COMMAND, Key::S);
    assert!(!app(&harness).editing);
    assert_eq!(app(&harness).saved.len(), 1);
    open_editor(&mut harness);
    let receipt = invoke_action(&mut harness, Act::Save).unwrap();
    assert_eq!(receipt.state, ReceiptState::Completed, "{receipt:?}");
    assert!(!app(&harness).editing);
    assert_eq!(app(&harness).saved.len(), 2);
    assert_eq!(app(&harness).refreshed, 0);
    assert_clean_audit(&harness);
}

#[test]
fn a_shortcut_for_an_action_behind_the_dialog_does_nothing() {
    let mut harness = harness();
    press(&mut harness, Modifiers::COMMAND, Key::R);
    assert_eq!(app(&harness).refreshed, 1);
    open_editor(&mut harness);
    press(&mut harness, Modifiers::COMMAND, Key::R);
    assert_eq!(app(&harness).refreshed, 1);
    assert!(app(&harness).editing);
}

#[test]
fn observation_holds_only_the_dialog() {
    let mut harness = harness();
    open_editor(&mut harness);
    let snapshot = host(&harness).ui_snapshot();
    let ids: Vec<&str> = snapshot
        .nodes
        .iter()
        .map(|node| node.id.0.as_str())
        .collect();
    for behind in [
        action_node(Act::Edit),
        action_node(Act::SignOut),
        action_node(Act::Refresh),
        SemanticUiId::new(SEARCH),
    ] {
        assert!(snapshot.node(&behind).is_none(), "{behind:?} in {ids:?}");
    }
    let dialog = snapshot
        .nodes
        .iter()
        .find(|node| node.role == UiRole::Dialog)
        .unwrap();
    for inside in [NAME, PROMPT] {
        let node = snapshot.node(&SemanticUiId::new(inside)).unwrap();
        assert_eq!(node.parent.as_ref(), Some(&dialog.id), "{inside}");
    }
    let names: Vec<&str> = snapshot
        .nodes
        .iter()
        .map(|node| node.name.as_str())
        .collect();
    for name in ["Edit routine", "Name", "Prompt", "Save", "Close"] {
        assert!(names.contains(&name), "{name} missing from {names:?}");
    }
    assert!(!names.contains(&"Refresh"), "{names:?}");
    assert_clean_audit(&harness);
}

#[test]
fn assistive_technology_sees_only_the_modal_dialog() {
    let mut harness = harness();
    assert!(!hidden_from_assistive_technology(&harness, "Refresh"));
    open_editor(&mut harness);
    let dialog = harness.get_by_role(egui::accesskit::Role::Dialog);
    assert!(dialog.accesskit_node().is_modal());
    assert_eq!(
        dialog.accesskit_node().label().as_deref(),
        Some("Edit routine")
    );
    for behind in ["Edit routine", "Sign out", "Refresh", "Search"] {
        // "Edit routine" also names the dialog; the opener is the button.
        let node = harness
            .query_all_by_label(behind)
            .find(|node| node.accesskit_node().role() != egui::accesskit::Role::Dialog)
            .unwrap();
        let mut current = Some(node.accesskit_node());
        let mut hidden = false;
        while let Some(node) = current {
            hidden |= node.is_hidden();
            current = node.parent();
        }
        assert!(hidden, "{behind} is exposed behind the dialog");
    }
    for inside in ["Name", "Prompt", "Save", "Close"] {
        assert!(
            !hidden_from_assistive_technology(&harness, inside),
            "{inside} is hidden"
        );
    }
    // A request aimed behind the dialog is dropped; one inside it works.
    harness.get_by_label("Refresh").click_accesskit();
    harness.run();
    assert_eq!(app(&harness).refreshed, 0);
    harness.get_by_label("Save").click_accesskit();
    harness.run();
    assert_eq!(app(&harness).saved.len(), 1);
    assert!(!app(&harness).editing);
    assert!(!hidden_from_assistive_technology(&harness, "Refresh"));
}

#[test]
fn a_sheet_is_a_bottom_sheet_on_a_phone() {
    let mut harness = harness_at(Planner::default(), PHONE);
    open_editor(&mut harness);
    assert_eq!(view(&harness).placement, Some(DialogPlacement::BottomSheet));
    let rect = view(&harness).rect.unwrap();
    let screen = harness.ctx.content_rect();
    assert!((rect.width() - screen.width()).abs() < 0.5, "{rect:?}");
    assert!((rect.bottom() - screen.bottom()).abs() < 0.5, "{rect:?}");
    assert!(
        rect.top() > screen.center().y,
        "as tall as its content: {rect:?}"
    );
    // Its controls are inside it, and it is observed where it is drawn.
    let save = harness.get_by_label("Save").rect();
    assert!(rect.contains_rect(save), "{save:?} outside {rect:?}");
    let snapshot = host(&harness).ui_snapshot();
    let dialog = snapshot
        .nodes
        .iter()
        .find(|node| node.role == UiRole::Dialog)
        .unwrap();
    let observed = Rect::from(dialog.rect);
    assert!((observed.top() - rect.top()).abs() < 0.5, "{observed:?}");
    assert_clean_audit(&harness);
}

#[test]
fn a_full_height_sheet_covers_a_phone() {
    let mut harness = harness_at(
        Planner {
            full: true,
            ..Planner::default()
        },
        PHONE,
    );
    open_editor(&mut harness);
    assert_eq!(view(&harness).placement, Some(DialogPlacement::FullScreen));
    let rect = view(&harness).rect.unwrap();
    let screen = harness.ctx.content_rect();
    assert!(
        (rect.min - screen.min).length() < 0.5 && (rect.max - screen.max).length() < 0.5,
        "{rect:?} does not cover {screen:?}"
    );
    assert_clean_audit(&harness);
}

#[test]
fn a_sheet_is_a_centred_dialog_on_a_desktop() {
    for full in [false, true] {
        let mut harness = harness_at(
            Planner {
                full,
                ..Planner::default()
            },
            DESKTOP,
        );
        open_editor(&mut harness);
        assert_eq!(view(&harness).placement, Some(DialogPlacement::Centred));
        let rect = view(&harness).rect.unwrap();
        let screen = harness.ctx.content_rect();
        assert!((rect.width() - crate::DIALOG_WIDTH).abs() < 0.5, "{rect:?}");
        assert!((rect.center() - screen.center()).length() < 1.0, "{rect:?}");
    }
}

#[test]
fn a_centred_dialog_fits_a_phone() {
    let mut harness = harness_at(
        Planner {
            centred: true,
            ..Planner::default()
        },
        PHONE,
    );
    open_editor(&mut harness);
    assert_eq!(view(&harness).placement, Some(DialogPlacement::Centred));
    let rect = view(&harness).rect.unwrap();
    let screen = harness.ctx.content_rect();
    assert!(screen.contains_rect(rect), "{rect:?}");
    assert!(rect.width() < screen.width(), "{rect:?}");
    assert!(
        (rect.center().x - screen.center().x).abs() < 0.5,
        "{rect:?}"
    );
}

#[test]
fn text_fields_in_a_sheet_take_typing_and_set_text() {
    let mut harness = harness_at(Planner::default(), PHONE);
    open_editor(&mut harness);
    // Typing into the focused name field.
    harness.event(egui::Event::Text("Morning".into()));
    harness.run();
    assert_eq!(app(&harness).name, "Morning");
    // The native text edit takes focus from Tab and typing.
    press(&mut harness, Modifiers::NONE, Key::Tab);
    assert_eq!(focused(&harness), focus("Prompt"));
    harness.event(egui::Event::Text("Summarise".into()));
    harness.run();
    assert_eq!(app(&harness).prompt, "Summarise");
    // Agents set both fields' text.
    for (field, text) in [(NAME, "Evening"), (PROMPT, "List the news")] {
        let mut found = discover(&harness, by_id(field));
        assert_eq!(found.len(), 1, "{field}: {found:?}");
        let expected = found.remove(0).target;
        let receipt = invoke(
            &mut harness,
            &format!("set-{field}"),
            by_id(field),
            expected,
            serde_json::json!({ "value": text }),
        )
        .unwrap();
        assert_eq!(receipt.state, ReceiptState::Completed, "{receipt:?}");
    }
    assert_eq!(app(&harness).name, "Evening");
    assert_eq!(app(&harness).prompt, "List the news");
    assert!(app(&harness).search.is_empty());
    harness.get_by_label("Save").click();
    harness.run();
    assert_eq!(
        app(&harness).saved,
        [("Evening".to_owned(), "List the news".to_owned())]
    );
}

#[test]
fn a_dialog_that_cannot_be_dismissed_stays_open() {
    let mut harness = harness_at(
        Planner {
            saving: true,
            ..Planner::default()
        },
        DESKTOP,
    );
    open_editor(&mut harness);
    press(&mut harness, Modifiers::NONE, Key::Escape);
    harness.run();
    assert!(app(&harness).editing);
    let close = discover(&harness, by_capability(Act::Close));
    assert_eq!(
        close[0].availability.disabled_reason(),
        Some("The routine is saving")
    );
    harness.get_by_label("Close").click();
    harness.run();
    assert!(app(&harness).editing);
    assert_eq!(app(&harness).performed, [Act::Edit]);
}

fn open_confirm(harness: &mut Harness<'_, Probe>) {
    harness.get_by_label("Sign out").click();
    harness.run();
    assert!(app(harness).confirming);
}

#[test]
fn a_confirm_focuses_cancel_and_return_cancels() {
    let mut harness = harness();
    open_confirm(&mut harness);
    assert_eq!(focused(&harness), focus("Keep signed in"));
    let dialog = harness.get_by_role(egui::accesskit::Role::AlertDialog);
    assert!(dialog.accesskit_node().is_modal());
    assert_eq!(
        dialog.accesskit_node().label().as_deref(),
        Some("Sign out?")
    );
    assert_eq!(
        dialog.accesskit_node().description().as_deref(),
        Some("You will need to sign in again to see your routines.")
    );
    let snapshot = host(&harness).ui_snapshot();
    let observed = snapshot
        .nodes
        .iter()
        .find(|node| node.role == UiRole::AlertDialog)
        .unwrap();
    assert_eq!(observed.name, "Sign out?");
    assert_clean_audit(&harness);
    press(&mut harness, Modifiers::NONE, Key::Enter);
    harness.run();
    assert!(!app(&harness).confirming);
    assert!(!app(&harness).signed_out);
    assert_eq!(app(&harness).performed, [Act::SignOut, Act::KeepSignedIn]);
    assert_eq!(focused(&harness), focus("Sign out"));
}

#[test]
fn a_confirm_confirms_from_the_keyboard_and_the_pointer() {
    let mut harness = harness();
    open_confirm(&mut harness);
    press(&mut harness, Modifiers::NONE, Key::Tab);
    assert_eq!(focused(&harness), focus("Sign out now"));
    // Tab wraps between its two controls.
    press(&mut harness, Modifiers::NONE, Key::Tab);
    assert_eq!(focused(&harness), focus("Keep signed in"));
    press(&mut harness, Modifiers::SHIFT, Key::Tab);
    assert_eq!(focused(&harness), focus("Sign out now"));
    press(&mut harness, Modifiers::NONE, Key::Enter);
    harness.run();
    assert!(app(&harness).signed_out);
    assert!(!app(&harness).confirming);

    let mut harness = self::harness();
    open_confirm(&mut harness);
    harness.get_by_label("Sign out now").click();
    harness.run();
    assert!(app(&harness).signed_out);
    assert_eq!(app(&harness).performed, [Act::SignOut, Act::ConfirmSignOut]);
}

#[test]
fn escape_cancels_a_confirm_and_agents_see_its_two_choices() {
    let mut harness = harness();
    open_confirm(&mut harness);
    let refresh = discover(&harness, by_capability(Act::Refresh));
    assert_eq!(
        refresh[0].availability.disabled_reason(),
        Some("The dialog \"Sign out?\" is open")
    );
    for action in [Act::ConfirmSignOut, Act::KeepSignedIn] {
        let found = discover(&harness, by_capability(action));
        assert!(found[0].availability.enabled(), "{action:?}: {found:?}");
    }
    press(&mut harness, Modifiers::NONE, Key::Escape);
    harness.run();
    assert!(!app(&harness).confirming);
    assert!(!app(&harness).signed_out);
    assert_eq!(focused(&harness), focus("Sign out"));
}

#[test]
fn a_confirm_is_centred_and_fits_a_phone() {
    let mut harness = harness_at(Planner::default(), PHONE);
    open_confirm(&mut harness);
    let rect = view(&harness).rect.unwrap();
    let screen = harness.ctx.content_rect();
    assert!(screen.contains_rect(rect), "{rect:?}");
    assert!((rect.center() - screen.center()).length() < 1.0, "{rect:?}");
    for label in ["Keep signed in", "Sign out now"] {
        let control = harness.get_by_label(label).rect();
        assert!(rect.contains_rect(control), "{label}: {control:?}");
    }
    assert_clean_audit(&harness);
}
