use std::hash::Hash;

use egui::{Key, KeyboardShortcut, Modifiers, Ui};
use serde::{Deserialize, Serialize};
use stobie_core::PaneId;

use crate::ActionArgument;

pub use stobie_protocol::actions::*;

/// egui forms of the protocol's shortcut types.
pub trait ShortcutEgui {
    type Egui;
    /// The egui key or keyboard shortcut.
    fn egui(self) -> Self::Egui;
}

impl ShortcutEgui for ShortcutKey {
    type Egui = Key;

    fn egui(self) -> Key {
        Key::from_name(self.name()).expect("every shortcut key name parses as an egui key")
    }
}

impl ShortcutEgui for ActionShortcut {
    type Egui = KeyboardShortcut;

    fn egui(self) -> KeyboardShortcut {
        KeyboardShortcut::new(
            Modifiers {
                command: self.command,
                shift: self.shift,
                alt: self.alt,
                ..Modifiers::NONE
            },
            self.key.egui(),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ActionTarget<A: ActionKey> {
    /// Application-owned capability to route through intents/commands.
    pub action: A,
    /// Stable pane identity for pane scopes; absent for application scope.
    pub pane: Option<PaneId>,
}

/// An action, or an explicit target, resolved against the pane it is presented
/// in. Hosted frames accept either: an application-scoped action is its own
/// target, and a pane-scoped action takes the pane of its pane presentation.
/// A [`BoundAction`] also carries the argument its control acts with.
pub trait IntoActionTarget<A: ActionKey> {
    /// Resolve the target. Panics for a pane-scoped action without a pane.
    fn into_target(self, pane: Option<PaneId>) -> ActionTarget<A>;

    /// Resolve the target together with the argument its control carries,
    /// none unless this is a [`BoundAction`].
    fn into_bound(self, pane: Option<PaneId>) -> BoundAction<A>
    where
        Self: Sized,
    {
        BoundAction::for_target(self.into_target(pane), ActionArgument::None)
    }
}

impl<A: ActionKey> IntoActionTarget<A> for A {
    fn into_target(self, pane: Option<PaneId>) -> ActionTarget<A> {
        match (self.specification().scope, pane) {
            (ActionScope::Application, _) => ActionTarget::application(self),
            (_, Some(pane)) => ActionTarget::pane(self, pane),
            (_, None) => panic!(
                "`{}` is pane-scoped: present it inside Frame::pane_presentation or pass ActionTarget::pane",
                self.stable_id()
            ),
        }
    }
}

impl<A: ActionKey> IntoActionTarget<A> for ActionTarget<A> {
    fn into_target(self, _pane: Option<PaneId>) -> ActionTarget<A> {
        self
    }
}

/// An action together with the argument its control carries, such as the ID
/// of the row it acts on: one action, such as Open, presented once per row.
///
/// Pass it wherever a hosted [`Frame`](crate::Frame) takes a target:
/// [`Frame::action`](crate::Frame::action), `icon_action`, `toggle`,
/// `list_row`, `trigger` and `availability`. A click triggers the action with
/// the argument, through the same route as a semantic invocation, and
/// [`Application::availability_with`](crate::Application::availability_with)
/// decides availability for it. The control's semantic node publishes the
/// argument, so an agent discovers each row's control with the value it
/// carries and invokes it without supplying one. Give the subject, such as the
/// row's title, so that each control has its own accessible name ("Open Ada").
/// Give a control its own label, such as "Tuesday" for one option of a
/// weekday choice, with [`with_label`](Self::with_label), and a shorter one
/// to show, such as "Tue", with [`with_short_label`](Self::with_short_label);
/// [`Frame::segmented_choice`](crate::Frame::segmented_choice) presents a
/// whole set of options of one action this way.
///
/// The argument must satisfy the action's declared contract, which the host
/// checks again before performing it.
///
/// ```
/// use serde::Serialize;
/// use stobie_ui_egui::{
///     Action, ActionArgument, ActionEmphasis, ActionTarget, Application, ApplicationIdentity,
///     Availability, BoundAction, Frame, IconId, ListRowSpec, PresentationScope, SemanticUiId,
///     application_identity,
/// };
///
/// #[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
/// #[action(prefix = "people")]
/// enum PeopleAction {
///     #[action(label = "Open", description = "Open the person with this ID",
///              argument = "number", minimum = 1, maximum = 999, integer = true)]
///     Open,
///     #[action(label = "Remove", description = "Remove the person with this ID",
///              argument = "number", minimum = 1, maximum = 999, integer = true)]
///     Remove,
/// }
///
/// struct Person { id: u32, name: String, role: String }
/// struct People { people: Vec<Person>, opened: Option<u32> }
///
/// impl People {
///     /// The listed person an argument names.
///     fn index(&self, argument: &ActionArgument) -> Option<usize> {
///         let id = argument.as_number()?;
///         self.people.iter().position(|person| f64::from(person.id) == id)
///     }
/// }
///
/// impl Application for People {
///     type Action = PeopleAction;
///     type Intent = ();
///     type UiState = ();
///
///     fn identity(&self) -> ApplicationIdentity {
///         application_identity!("People")
///     }
///
///     fn availability(&self, _: ActionTarget<PeopleAction>) -> Availability {
///         Availability::Enabled
///     }
///
///     // A row's actions are available while its person is listed, so a
///     // click or an agent acting on a removed row is refused.
///     fn availability_with(&self, _: ActionTarget<PeopleAction>, argument: &ActionArgument)
///         -> Availability {
///         match self.index(argument) {
///             Some(_) => Availability::Enabled,
///             None => Availability::Hidden,
///         }
///     }
///
///     fn ui(&self, ui: &mut egui::Ui, _: &mut (), frame: &mut Frame<'_, PeopleAction, ()>) {
///         let mut p = frame.presentation(ui, PresentationScope::new("people"), SemanticUiId::root());
///         for person in &self.people {
///             // The row opens its person; its icon removes them ("Remove Ada").
///             frame.list_row(ui, &mut p, BoundAction::new(PeopleAction::Open, person.id),
///                 ListRowSpec { title: &person.name, detail: Some(&person.role) });
///             frame.icon_action(ui, &mut p,
///                 BoundAction::new(PeopleAction::Remove, person.id).with_subject(&person.name),
///                 IconId::Remove, ActionEmphasis::Quiet);
///         }
///     }
///
///     fn apply(&mut self, _: (), _: &mut ()) -> Result<bool, String> {
///         Ok(false)
///     }
///
///     fn perform(&mut self, _: ActionTarget<PeopleAction>, _: &mut ()) -> Result<bool, String> {
///         Err("Choose a person".into())
///     }
///
///     // Clicks and agents deliver the row's ID, already validated.
///     fn perform_with(&mut self, target: ActionTarget<PeopleAction>, argument: ActionArgument,
///         _: &mut ()) -> Result<bool, String> {
///         let index = self.index(&argument).ok_or("Nobody has this ID")?;
///         match target.action {
///             PeopleAction::Open => self.opened = Some(self.people[index].id),
///             PeopleAction::Remove => drop(self.people.remove(index)),
///         }
///         Ok(true)
///     }
///
///     fn report_error(&mut self, _: String) {}
/// }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct BoundAction<A: ActionKey> {
    action: A,
    pane: Option<PaneId>,
    explicit: bool,
    argument: ActionArgument,
    subject: Option<String>,
    label: Option<String>,
    short_label: Option<String>,
}

impl<A: ActionKey> BoundAction<A> {
    /// `action` carrying `argument`. A pane-scoped action takes the pane of
    /// its presentation, as an unbound action does.
    pub fn new(action: A, argument: impl Into<ActionArgument>) -> Self {
        Self {
            action,
            pane: None,
            explicit: false,
            argument: argument.into(),
            subject: None,
            label: None,
            short_label: None,
        }
    }

    /// An explicit target carrying `argument`, such as a pane-scoped action
    /// presented outside its pane.
    pub fn for_target(target: ActionTarget<A>, argument: impl Into<ActionArgument>) -> Self {
        Self {
            action: target.action,
            pane: target.pane,
            explicit: true,
            argument: argument.into(),
            subject: None,
            label: None,
            short_label: None,
        }
    }

    /// Name what the argument identifies, such as the row's title. Controls
    /// add it to the action's label in their accessible name ("Open Ada"); a
    /// list row is named by its own title instead.
    pub fn with_subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    /// Give the control its own label in place of the action's, such as
    /// "Tuesday" for one option of a weekday choice: it shows the label and is
    /// named by it, followed by the subject when one is named. The action, its
    /// description and the argument stay the same.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Show `short_label`, such as "Tue", in place of the label where space
    /// is tight. The accessible name and the tooltip keep the full label
    /// ("Tuesday"), as an action's own compact label does.
    pub fn with_short_label(mut self, short_label: impl Into<String>) -> Self {
        self.short_label = Some(short_label.into());
        self
    }

    /// The action this control performs.
    pub fn action(&self) -> A {
        self.action
    }

    /// The control's own label, when given in place of the action's.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// The control's own short label, when given.
    pub fn short_label(&self) -> Option<&str> {
        self.short_label.as_deref()
    }

    /// The label the control shows: its short label, its own label or the
    /// action's, in that order.
    pub fn visible_label(&self) -> &str {
        self.short_label
            .as_deref()
            .or(self.label.as_deref())
            .unwrap_or(self.action.specification().label)
    }

    /// The argument the control carries.
    pub fn argument(&self) -> &ActionArgument {
        &self.argument
    }

    /// What the argument identifies, when named.
    pub fn subject(&self) -> Option<&str> {
        self.subject.as_deref()
    }

    /// The resolved target. Panics for a pane-scoped action that was never
    /// resolved against a pane.
    pub fn target(&self) -> ActionTarget<A> {
        if self.explicit {
            ActionTarget {
                action: self.action,
                pane: self.pane,
            }
        } else {
            self.action.into_target(self.pane)
        }
    }

    /// The accessible name of a button presenting this action: its label
    /// (the control's own, or the action's), followed by the subject when one
    /// is named.
    pub fn accessible_name(&self) -> String {
        let label = self
            .label
            .as_deref()
            .unwrap_or(self.action.specification().label);
        match &self.subject {
            Some(subject) => format!("{label} {subject}"),
            None => label.to_owned(),
        }
    }
}

impl<A: ActionKey> IntoActionTarget<A> for BoundAction<A> {
    /// The target alone; the argument is dropped. Frames use
    /// [`IntoActionTarget::into_bound`], which keeps it.
    fn into_target(self, pane: Option<PaneId>) -> ActionTarget<A> {
        self.into_bound(pane).target()
    }

    fn into_bound(mut self, pane: Option<PaneId>) -> BoundAction<A> {
        if !self.explicit {
            let target = self.action.into_target(self.pane.or(pane));
            self.pane = target.pane;
            self.explicit = true;
        }
        self
    }
}

impl<A: ActionKey> ActionTarget<A> {
    pub fn application(action: A) -> Self {
        assert!(
            matches!(action.specification().scope, ActionScope::Application),
            "`{}` is pane-scoped; use ActionTarget::pane",
            action.stable_id()
        );
        Self { action, pane: None }
    }

    pub fn pane(action: A, pane: PaneId) -> Self {
        assert!(
            !matches!(action.specification().scope, ActionScope::Application),
            "`{}` is application-scoped; use ActionTarget::application",
            action.stable_id()
        );
        Self {
            action,
            pane: Some(pane),
        }
    }

    pub fn semantic_id(self) -> String {
        self.pane.map_or_else(
            || format!("action.{}", self.action.stable_id()),
            |pane| format!("action.{}.pane.{}", self.action.stable_id(), pane.0),
        )
    }
}

pub fn consume_action_shortcut<A: ActionKey>(ui: &mut Ui, id: A, active_pane: bool) -> bool {
    let spec = id.specification();
    let Some(shortcut) = spec.shortcut else {
        return false;
    };
    if spec.scope != ActionScope::Application && !active_pane {
        return false;
    }
    // Plain keys yield to text entry. Enter and Space also yield to any other
    // focused control, which they activate; without the host's observation a
    // focused viewport cannot be told apart.
    let activation = matches!(shortcut.key, ShortcutKey::Enter | ShortcutKey::Space);
    let context = ui.ctx();
    if !shortcut.command
        && (context.text_edit_focused() || activation && context.egui_wants_keyboard_input())
    {
        return false;
    }
    ui.input_mut(|input| input.consume_shortcut(&shortcut.egui()))
}

pub(crate) fn stable_action_hash<A: ActionKey>(action: A, pane: Option<PaneId>) -> u64 {
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    const JAVASCRIPT_SAFE_INTEGER_MASK: u64 = (1_u64 << 53) - 1;

    let hash = action
        .stable_id()
        .as_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
        });
    let hash = (hash ^ u64::from(pane.is_some())).wrapping_mul(FNV_PRIME);
    pane.map_or(hash, |pane| {
        pane.0.to_le_bytes().iter().fold(hash, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
        })
    }) & JAVASCRIPT_SAFE_INTEGER_MASK
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_actions::TestAction;

    #[test]
    fn a_bound_actions_own_label_names_and_shows_it() {
        let plain = BoundAction::new(TestAction::Undo, 2_u32).with_subject("Ada");
        assert_eq!(plain.visible_label(), "Undo");
        assert_eq!(plain.accessible_name(), "Undo Ada");
        let labelled = BoundAction::new(TestAction::Undo, "tue").with_label("Tuesday");
        assert_eq!(labelled.label(), Some("Tuesday"));
        assert_eq!(labelled.visible_label(), "Tuesday");
        assert_eq!(labelled.accessible_name(), "Tuesday");
        let short = labelled.with_short_label("Tue");
        assert_eq!(short.short_label(), Some("Tue"));
        assert_eq!(short.visible_label(), "Tue");
        assert_eq!(short.accessible_name(), "Tuesday");
        assert_eq!(
            BoundAction::new(TestAction::Undo, 3_u32)
                .with_label("Edit")
                .with_subject("Ada")
                .accessible_name(),
            "Edit Ada"
        );
        // The action and argument stay the action's own.
        assert_eq!(short.action(), TestAction::Undo);
        assert_eq!(short.argument(), &ActionArgument::from("tue"));
    }

    #[test]
    fn every_shortcut_key_name_is_a_distinct_egui_key() {
        let keys: std::collections::BTreeSet<_> = ShortcutKey::ALL
            .iter()
            .map(|key| {
                let egui = Key::from_name(key.name()).expect("the name parses");
                assert_eq!(egui, key.egui());
                egui
            })
            .collect();
        assert_eq!(keys.len(), ShortcutKey::ALL.len());
        assert_eq!(ShortcutKey::Zero.egui(), Key::Num0);
        assert_eq!(ShortcutKey::PageUp.egui(), Key::PageUp);
        assert_eq!(ShortcutKey::ArrowLeft.egui(), Key::ArrowLeft);
    }

    #[test]
    fn shortcuts_have_aria_key_names() {
        let command = if cfg!(target_os = "macos") {
            "Meta"
        } else {
            "Control"
        };
        assert_eq!(
            ActionShortcut::command(ShortcutKey::Enter).aria_keyshortcuts(),
            format!("{command}+Enter")
        );
        assert_eq!(
            ActionShortcut::command_shift(ShortcutKey::Z).aria_keyshortcuts(),
            format!("{command}+Shift+Z")
        );
        assert_eq!(
            ActionShortcut::command(ShortcutKey::Equals).aria_keyshortcuts(),
            format!("{command}+=")
        );
        assert_eq!(
            ActionShortcut::new(ShortcutKey::ArrowUp).aria_keyshortcuts(),
            "ArrowUp"
        );
        assert_eq!(
            ActionShortcut::new(ShortcutKey::PageDown).aria_keyshortcuts(),
            "PageDown"
        );
        for key in [
            ShortcutKey::Comma,
            ShortcutKey::Period,
            ShortcutKey::Minus,
            ShortcutKey::Slash,
        ] {
            assert_eq!(ActionShortcut::new(key).aria_keyshortcuts(), key.label());
        }
        assert_eq!(
            ActionShortcut::new(ShortcutKey::F5).aria_keyshortcuts(),
            "F5"
        );
    }

    #[test]
    fn registry_is_complete_unique_and_has_stable_identity() {
        let ids: std::collections::BTreeSet<_> = TestAction::ALL.into_iter().collect();
        assert_eq!(ids.len(), TestAction::ALL.len());
        for id in TestAction::ALL {
            let spec = id.specification();
            assert_eq!(spec.id, id);
            assert!(!id.stable_id().is_empty());
            assert!(!spec.label.is_empty());
            assert!(!spec.description.is_empty());
        }
        assert_eq!(
            ActionTarget::pane(TestAction::FitView, PaneId(3)).semantic_id(),
            "action.fit_view.pane.3"
        );
    }

    #[test]
    fn stable_identity_does_not_depend_on_enum_ordinal() {
        assert_eq!(TestAction::Undo.stable_id(), "undo");
        assert_eq!(TestAction::FitView.stable_id(), "fit_view");
        assert_ne!(
            stable_action_hash(TestAction::Undo, None),
            stable_action_hash(TestAction::FitView, None)
        );
        assert_ne!(
            stable_action_hash(TestAction::FitView, None),
            stable_action_hash(TestAction::FitView, Some(PaneId(1)))
        );
        assert!(stable_action_hash(TestAction::FitView, Some(PaneId(u32::MAX))) < (1_u64 << 53));
    }

    #[test]
    fn availability_retains_observable_disabled_reason() {
        let availability = Availability::Disabled {
            reason: "History is empty".into(),
        };
        assert!(!availability.enabled());
        assert!(availability.visible());
        assert_eq!(availability.disabled_reason(), Some("History is empty"));
        assert!(!Availability::Hidden.visible());
    }

    #[test]
    fn shortcuts_are_unique_within_overlapping_scopes() {
        let mut seen = std::collections::BTreeSet::new();
        for id in TestAction::ALL {
            let spec = id.specification();
            if let Some(shortcut) = spec.shortcut {
                assert!(
                    seen.insert((spec.scope, shortcut)),
                    "duplicate shortcut for {id:?}"
                );
            }
        }
    }

    #[test]
    #[should_panic]
    fn application_target_rejects_a_pane_action() {
        let _ = ActionTarget::application(TestAction::FitView);
    }

    #[test]
    #[should_panic]
    fn pane_target_rejects_an_application_action() {
        let _ = ActionTarget::pane(TestAction::Undo, PaneId(1));
    }

    #[test]
    fn shortcut_consumption_respects_scope() {
        fn pressed(action: TestAction, active_pane: bool, modifiers: Modifiers) -> bool {
            let context = egui::Context::default();
            crate::install_typography_fonts(&context);
            let shortcut = action
                .specification()
                .shortcut
                .expect("tested action has a shortcut");
            let mut consumed = false;
            let mut output = context.run_ui(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key: shortcut.key.egui(),
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    }],
                    ..Default::default()
                },
                |ui| {
                    consumed = consume_action_shortcut(ui, action, active_pane);
                },
            );
            output.textures_delta.clear();
            consumed
        }

        assert!(pressed(TestAction::Undo, true, Modifiers::COMMAND));
        assert!(!pressed(TestAction::FitView, false, Modifiers::NONE));
        assert!(pressed(TestAction::FitView, true, Modifiers::NONE));
    }

    #[test]
    fn plain_shortcuts_yield_to_text_entry_and_activation_keys_to_controls() {
        fn pressed_with_focus(action: TestAction, text_entry: bool) -> bool {
            let shortcut = action.specification().shortcut.expect("has a shortcut");
            let context = egui::Context::default();
            crate::install_typography_fonts(&context);
            let focus = egui::Id::new("focus");
            let mut draft = String::new();
            let mut consumed = false;
            for events in [
                Vec::new(),
                Vec::new(),
                vec![egui::Event::Key {
                    key: shortcut.key.egui(),
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }],
            ] {
                let mut output = context.run_ui(
                    egui::RawInput {
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        consumed = consume_action_shortcut(ui, action, true);
                        if text_entry {
                            ui.add(egui::TextEdit::singleline(&mut draft).id(focus));
                        } else {
                            // A focusable non-text widget, such as a button.
                            let rect = ui.allocate_space(egui::vec2(40.0, 40.0)).1;
                            ui.interact(rect, focus, egui::Sense::click());
                        }
                        ui.memory_mut(|memory| memory.request_focus(focus));
                    },
                );
                output.textures_delta.clear();
            }
            assert!(context.egui_wants_keyboard_input());
            consumed
        }

        assert!(pressed_with_focus(TestAction::FitView, false));
        assert!(!pressed_with_focus(TestAction::FitView, true));
        // The focused control activates with Enter, so Enter stays in the input.
        assert!(!pressed_with_focus(TestAction::CommitPolygon, false));
    }
}
