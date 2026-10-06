use polyorama_ui_egui::{ActionKey, ActionScope, ActionShortcut, ActionSpec, ShortcutKey};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize)]
pub enum Action {
    Apply,
    Cancel,
    Undo,
    Redo,
    Save,
    Restore,
    Search,
    ResetFilters,
    Arrange,
    ToggleReviewed,
    FilterCategory,
    FilterReview,
    EditCategory,
}

impl Action {
    pub const ALL: [Self; 13] = [
        Self::Apply,
        Self::Cancel,
        Self::Undo,
        Self::Redo,
        Self::Save,
        Self::Restore,
        Self::Search,
        Self::ResetFilters,
        Self::Arrange,
        Self::ToggleReviewed,
        Self::FilterCategory,
        Self::FilterReview,
        Self::EditCategory,
    ];
}

impl ActionKey for Action {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Apply => "record-desk.apply",
            Self::Cancel => "record-desk.cancel",
            Self::Undo => "record-desk.undo",
            Self::Redo => "record-desk.redo",
            Self::Save => "record-desk.save",
            Self::Restore => "record-desk.restore",
            Self::Search => "record-desk.search",
            Self::ResetFilters => "record-desk.reset-filters",
            Self::Arrange => "record-desk.arrange",
            Self::ToggleReviewed => "record-desk.toggle-reviewed",
            Self::FilterCategory => "record-desk.filter-category",
            Self::FilterReview => "record-desk.filter-review",
            Self::EditCategory => "record-desk.edit-category",
        }
    }

    fn specification(self) -> ActionSpec<Self> {
        let (label, description, shortcut) = match self {
            Self::Apply => (
                "Apply",
                "Validate and commit this draft as one undoable change",
                Some(ActionShortcut::command(ShortcutKey::Enter)),
            ),
            Self::Cancel => (
                "Cancel edits",
                "Discard the draft and keep the committed record",
                Some(ActionShortcut::command_shift(ShortcutKey::Enter)),
            ),
            Self::Undo => (
                "Undo",
                "Reverse the last committed record change",
                Some(ActionShortcut::command(ShortcutKey::Z)),
            ),
            Self::Redo => (
                "Redo",
                "Reapply the last undone record change",
                Some(ActionShortcut::command_shift(ShortcutKey::Z)),
            ),
            Self::Save => (
                "Save",
                "Save committed records and layout locally; drafts are excluded",
                Some(ActionShortcut::command(ShortcutKey::S)),
            ),
            Self::Restore => (
                "Restore",
                "Reload saved records and layout; unavailable with unsaved changes",
                Some(ActionShortcut::command_shift(ShortcutKey::R)),
            ),
            Self::Search => (
                "Search",
                "Focus record search",
                Some(ActionShortcut::command(ShortcutKey::F)),
            ),
            Self::ResetFilters => (
                "Reset filters",
                "Clear search, category and review filters while keeping selection and draft",
                Some(ActionShortcut::command_shift(ShortcutKey::F)),
            ),
            Self::Arrange => (
                "Arrange panes",
                "Switch between stacked and side-by-side panes",
                Some(ActionShortcut::command(ShortcutKey::L)),
            ),
            Self::ToggleReviewed => (
                "Reviewed",
                "Change the draft's reviewed state; Apply commits it",
                None,
            ),
            Self::FilterCategory => ("Category filter", "Limit the list by category", None),
            Self::FilterReview => ("Review filter", "Limit the list by review state", None),
            Self::EditCategory => ("Record category", "Change the draft's category", None),
        };
        ActionSpec {
            id: self,
            label,
            description,
            compact_label: None,
            shortcut,
            scope: ActionScope::Application,
        }
    }
}
