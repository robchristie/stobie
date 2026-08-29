use std::borrow::Cow;

use egui::{Key, KeyboardShortcut, Modifiers, Ui};
use polyorama_core::PaneId;
use serde::{Deserialize, Serialize};

/// Stable identity for a user-visible capability.
///
/// These IDs describe and route capabilities into the existing intent and
/// command system. They are not a second mutation architecture.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum ActionId {
    Undo = 0,
    Redo = 1,
    SaveLayout = 2,
    ResetWorkspace = 3,
    FitView = 4,
    LinkViews = 5,
    NavigateTool = 6,
    PolygonTool = 7,
    EditVerticesTool = 8,
    CommitPolygon = 9,
    DeleteAnnotation = 10,
    RecenterPrimary = 11,
    AppearanceSettings = 12,
    CopyDiagnostics = 13,
    DisplaySettings = 14,
}

impl ActionId {
    pub const ALL: [Self; 15] = [
        Self::Undo,
        Self::Redo,
        Self::SaveLayout,
        Self::ResetWorkspace,
        Self::AppearanceSettings,
        Self::DisplaySettings,
        Self::CopyDiagnostics,
        Self::FitView,
        Self::LinkViews,
        Self::NavigateTool,
        Self::PolygonTool,
        Self::EditVerticesTool,
        Self::CommitPolygon,
        Self::DeleteAnnotation,
        Self::RecenterPrimary,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::SaveLayout => "save_layout",
            Self::ResetWorkspace => "reset_workspace",
            Self::AppearanceSettings => "appearance_settings",
            Self::DisplaySettings => "display_settings",
            Self::CopyDiagnostics => "copy_diagnostics",
            Self::FitView => "fit_view",
            Self::LinkViews => "link_views",
            Self::NavigateTool => "navigate_tool",
            Self::PolygonTool => "polygon_tool",
            Self::EditVerticesTool => "edit_vertices_tool",
            Self::CommitPolygon => "commit_polygon",
            Self::DeleteAnnotation => "delete_annotation",
            Self::RecenterPrimary => "recenter_primary",
        }
    }
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
    pub key: ActionKey,
}

impl ActionShortcut {
    pub const fn new(key: ActionKey) -> Self {
        Self {
            command: false,
            shift: false,
            alt: false,
            key,
        }
    }

    pub const fn command(key: ActionKey) -> Self {
        Self {
            command: true,
            ..Self::new(key)
        }
    }

    pub const fn command_shift(key: ActionKey) -> Self {
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
pub enum ActionKey {
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

impl ActionKey {
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
pub struct ActionSpec {
    pub id: ActionId,
    pub label: &'static str,
    pub description: &'static str,
    pub compact_label: Option<&'static str>,
    pub shortcut: Option<ActionShortcut>,
    pub scope: ActionScope,
}

pub const fn action_spec(id: ActionId) -> ActionSpec {
    match id {
        ActionId::Undo => ActionSpec {
            id,
            label: "Undo",
            description: "Undo the most recent change",
            compact_label: None,
            shortcut: Some(ActionShortcut::command(ActionKey::Z)),
            scope: ActionScope::Application,
        },
        ActionId::Redo => ActionSpec {
            id,
            label: "Redo",
            description: "Redo the most recently undone change",
            compact_label: None,
            shortcut: Some(ActionShortcut::command_shift(ActionKey::Z)),
            scope: ActionScope::Application,
        },
        ActionId::SaveLayout => ActionSpec {
            id,
            label: "Save layout",
            description: "Persist the current workspace layout",
            compact_label: Some("Save"),
            shortcut: Some(ActionShortcut::command(ActionKey::S)),
            scope: ActionScope::Application,
        },
        ActionId::ResetWorkspace => ActionSpec {
            id,
            label: "Reset workspace",
            description: "Restore the default workspace, cameras and tools",
            compact_label: Some("Reset"),
            shortcut: None,
            scope: ActionScope::Application,
        },
        ActionId::AppearanceSettings => ActionSpec {
            id,
            label: "Appearance",
            description: "Adjust theme, contrast, density, text scale and motion",
            compact_label: Some("Display"),
            shortcut: None,
            scope: ActionScope::Application,
        },
        ActionId::DisplaySettings => ActionSpec {
            id,
            label: "Display",
            description: "Adjust the image colour map and scalar window",
            compact_label: None,
            shortcut: None,
            scope: ActionScope::Pane,
        },
        ActionId::CopyDiagnostics => ActionSpec {
            id,
            label: "Copy diagnostics",
            description: "Copy the current diagnostic snapshot as JSON",
            compact_label: Some("Copy JSON"),
            shortcut: None,
            scope: ActionScope::Pane,
        },
        ActionId::FitView => ActionSpec {
            id,
            label: "Fit view",
            description: "Fit the complete image into the active viewport",
            compact_label: Some("Fit"),
            shortcut: Some(ActionShortcut::new(ActionKey::F)),
            scope: ActionScope::Pane,
        },
        ActionId::LinkViews => ActionSpec {
            id,
            label: "Link views",
            description: "Link or unlink this camera with camera group A",
            compact_label: Some("Link A"),
            shortcut: Some(ActionShortcut::new(ActionKey::L)),
            scope: ActionScope::Pane,
        },
        ActionId::NavigateTool => ActionSpec {
            id,
            label: "Navigate",
            description: "Pan and zoom the active viewport",
            compact_label: None,
            shortcut: Some(ActionShortcut::new(ActionKey::One)),
            scope: ActionScope::Pane,
        },
        ActionId::PolygonTool => ActionSpec {
            id,
            label: "Polygon",
            description: "Create a polygon annotation",
            compact_label: None,
            shortcut: Some(ActionShortcut::new(ActionKey::Two)),
            scope: ActionScope::Pane,
        },
        ActionId::EditVerticesTool => ActionSpec {
            id,
            label: "Edit vertices",
            description: "Move vertices of the selected polygon",
            compact_label: Some("Edit"),
            shortcut: Some(ActionShortcut::new(ActionKey::Three)),
            scope: ActionScope::Pane,
        },
        ActionId::CommitPolygon => ActionSpec {
            id,
            label: "Commit polygon",
            description: "Commit the current polygon annotation",
            compact_label: Some("Commit"),
            shortcut: Some(ActionShortcut::new(ActionKey::Enter)),
            scope: ActionScope::ActivePane,
        },
        ActionId::DeleteAnnotation => ActionSpec {
            id,
            label: "Delete annotation",
            description: "Delete the selected polygon annotation",
            compact_label: Some("Delete"),
            shortcut: Some(ActionShortcut::new(ActionKey::Delete)),
            scope: ActionScope::ActivePane,
        },
        ActionId::RecenterPrimary => ActionSpec {
            id,
            label: "Recenter primary view",
            description: "Centre the primary viewport on the selected result",
            compact_label: Some("Recenter"),
            shortcut: Some(ActionShortcut::new(ActionKey::R)),
            scope: ActionScope::Pane,
        },
    }
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
pub struct ActionTarget {
    pub action: ActionId,
    pub pane: Option<PaneId>,
}

impl ActionTarget {
    pub const fn application(action: ActionId) -> Self {
        assert!(matches!(
            action_spec(action).scope,
            ActionScope::Application
        ));
        Self { action, pane: None }
    }

    pub const fn pane(action: ActionId, pane: PaneId) -> Self {
        assert!(!matches!(
            action_spec(action).scope,
            ActionScope::Application
        ));
        Self {
            action,
            pane: Some(pane),
        }
    }

    pub fn semantic_id(self) -> String {
        self.pane.map_or_else(
            || format!("action.{}", self.action.as_str()),
            |pane| format!("action.{}.pane.{}", self.action.as_str(), pane.0),
        )
    }
}

pub fn consume_action_shortcut(ui: &mut Ui, id: ActionId, active_pane: bool) -> bool {
    let spec = action_spec(id);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_complete_unique_and_has_stable_identity() {
        let ids: std::collections::BTreeSet<_> = ActionId::ALL.into_iter().collect();
        assert_eq!(ids.len(), ActionId::ALL.len());
        for id in ActionId::ALL {
            let spec = action_spec(id);
            assert_eq!(spec.id, id);
            assert!(!id.as_str().is_empty());
            assert!(!spec.label.is_empty());
            assert!(!spec.description.is_empty());
        }
        assert_eq!(
            ActionTarget::pane(ActionId::FitView, PaneId(3)).semantic_id(),
            "action.fit_view.pane.3"
        );
    }

    #[test]
    fn existing_numeric_action_identity_does_not_shift_when_actions_are_added() {
        assert_eq!(ActionId::Undo as u8, 0);
        assert_eq!(ActionId::ResetWorkspace as u8, 3);
        assert_eq!(ActionId::FitView as u8, 4);
        assert_eq!(ActionId::RecenterPrimary as u8, 11);
        assert_eq!(ActionId::AppearanceSettings as u8, 12);
        assert_eq!(ActionId::CopyDiagnostics as u8, 13);
        assert_eq!(ActionId::DisplaySettings as u8, 14);
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
        for id in ActionId::ALL {
            let spec = action_spec(id);
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
        let _ = ActionTarget::application(ActionId::FitView);
    }

    #[test]
    #[should_panic]
    fn pane_target_rejects_an_application_action() {
        let _ = ActionTarget::pane(ActionId::Undo, PaneId(1));
    }

    #[test]
    fn shortcut_consumption_respects_scope() {
        fn pressed(action: ActionId, active_pane: bool, modifiers: Modifiers) -> bool {
            let context = egui::Context::default();
            let shortcut = action_spec(action)
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

        assert!(pressed(ActionId::Undo, true, Modifiers::COMMAND));
        assert!(!pressed(ActionId::FitView, false, Modifiers::NONE));
        assert!(pressed(ActionId::FitView, true, Modifiers::NONE));
    }
}
