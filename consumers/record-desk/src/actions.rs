use serde::Serialize;
use stobie_ui_egui::Action;

#[derive(Action, Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize)]
#[action(prefix = "record-desk")]
pub enum Action {
    #[action(
        label = "Apply",
        description = "Validate and commit this draft as one undoable change",
        shortcut = "Cmd+Enter"
    )]
    Apply,
    #[action(
        label = "Cancel edits",
        description = "Discard the draft and keep the committed record",
        shortcut = "Cmd+Shift+Enter"
    )]
    Cancel,
    #[action(
        label = "Undo",
        description = "Reverse the last committed record change",
        shortcut = "Cmd+Z"
    )]
    Undo,
    #[action(
        label = "Redo",
        description = "Reapply the last undone record change",
        shortcut = "Cmd+Shift+Z"
    )]
    Redo,
    #[action(
        label = "Save",
        description = "Save committed records and layout locally; drafts are excluded",
        shortcut = "Cmd+S"
    )]
    Save,
    #[action(
        label = "Restore",
        description = "Reload saved records and layout; unavailable with unsaved changes",
        shortcut = "Cmd+Shift+R"
    )]
    Restore,
    #[action(
        label = "Search",
        description = "Focus record search",
        shortcut = "Cmd+F"
    )]
    Search,
    #[action(
        label = "Reset filters",
        description = "Clear search, category and review filters while keeping selection and draft",
        shortcut = "Cmd+Shift+F"
    )]
    ResetFilters,
    #[action(
        label = "Arrange panes",
        description = "Switch between stacked and side-by-side panes",
        shortcut = "Cmd+L"
    )]
    Arrange,
    #[action(
        label = "Reviewed",
        description = "Change the draft's reviewed state; Apply commits it"
    )]
    ToggleReviewed,
    #[action(
        label = "Category filter",
        description = "Limit the list by category",
        physical_only = "Chooses a category; actions with arguments are not supported yet"
    )]
    FilterCategory,
    #[action(
        label = "Review filter",
        description = "Limit the list by review state",
        physical_only = "Chooses a review state; actions with arguments are not supported yet"
    )]
    FilterReview,
    #[action(
        label = "Record category",
        description = "Change the draft's category",
        physical_only = "Chooses a category; actions with arguments are not supported yet"
    )]
    EditCategory,
}
