use std::borrow::Cow;
use std::hash::Hash;

use egui::{Key, KeyboardShortcut, Modifiers, Ui};
use polyorama_core::PaneId;
use serde::{Deserialize, Serialize};

/// Application-owned identity for a user-visible capability.
///
/// Implementations own both the stable external identity and the presentation
/// metadata used by generic egui controls. Action keys route capabilities into
/// the application's intent and command system; they are not a second mutation
/// architecture.
pub trait ActionKey: Copy + Eq + Hash + Ord + Serialize {
    fn stable_id(self) -> &'static str;
    fn specification(self) -> ActionSpec<Self>;
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionScope {
    Application,
    ActivePane,
    Pane,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ActionShortcut {
    pub command: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: ShortcutKey,
}

impl ActionShortcut {
    pub const fn new(key: ShortcutKey) -> Self {
        Self {
            command: false,
            shift: false,
            alt: false,
            key,
        }
    }

    pub const fn command(key: ShortcutKey) -> Self {
        Self {
            command: true,
            ..Self::new(key)
        }
    }

    pub const fn command_shift(key: ShortcutKey) -> Self {
        Self {
            command: true,
            shift: true,
            alt: false,
            key,
        }
    }

    pub fn egui(self) -> KeyboardShortcut {
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

    pub fn display(self) -> String {
        let mut parts = Vec::new();
        if self.command {
            parts.push("Ctrl/Cmd");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.alt {
            parts.push("Alt");
        }
        parts.push(self.key.label());
        parts.join("+")
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutKey {
    Z,
    S,
    F,
    L,
    One,
    Two,
    Three,
    Enter,
    Delete,
    R,
}

impl ShortcutKey {
    pub const fn egui(self) -> Key {
        match self {
            Self::Z => Key::Z,
            Self::S => Key::S,
            Self::F => Key::F,
            Self::L => Key::L,
            Self::One => Key::Num1,
            Self::Two => Key::Num2,
            Self::Three => Key::Num3,
            Self::Enter => Key::Enter,
            Self::Delete => Key::Delete,
            Self::R => Key::R,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Z => "Z",
            Self::S => "S",
            Self::F => "F",
            Self::L => "L",
            Self::One => "1",
            Self::Two => "2",
            Self::Three => "3",
            Self::Enter => "Enter",
            Self::Delete => "Delete",
            Self::R => "R",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ActionSpec<A: ActionKey> {
    pub id: A,
    pub label: &'static str,
    pub description: &'static str,
    pub compact_label: Option<&'static str>,
    pub shortcut: Option<ActionShortcut>,
    pub scope: ActionScope,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Availability {
    Enabled,
    Disabled { reason: Cow<'static, str> },
    Hidden,
}

impl Availability {
    pub const fn enabled(&self) -> bool {
        matches!(self, Self::Enabled)
    }

    pub const fn visible(&self) -> bool {
        !matches!(self, Self::Hidden)
    }

    pub fn disabled_reason(&self) -> Option<&str> {
        match self {
            Self::Disabled { reason } => Some(reason),
            Self::Enabled | Self::Hidden => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ActionTarget<A: ActionKey> {
    pub action: A,
    pub pane: Option<PaneId>,
}

impl<A: ActionKey> ActionTarget<A> {
    pub fn application(action: A) -> Self {
        assert!(matches!(
            action.specification().scope,
            ActionScope::Application
        ));
        Self { action, pane: None }
    }

    pub fn pane(action: A, pane: PaneId) -> Self {
        assert!(!matches!(
            action.specification().scope,
            ActionScope::Application
        ));
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
    if !shortcut.command && ui.ctx().egui_wants_keyboard_input() {
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
}
