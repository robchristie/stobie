use std::hash::Hash;

use egui::{Key, KeyboardShortcut, Modifiers, Ui};
use serde::{Deserialize, Serialize};
use stobie_core::PaneId;

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
pub trait IntoActionTarget<A: ActionKey> {
    /// Resolve the target. Panics for a pane-scoped action without a pane.
    fn into_target(self, pane: Option<PaneId>) -> ActionTarget<A>;
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
