use serde::Serialize;
use stobie_ui_egui::{Action, ChoiceArgument};

use crate::model::{Category, ReviewFilter};

impl ChoiceArgument for Category {
    const CHOICES: &'static [(Self, &'static str, &'static str)] = &[
        (Self::Research, "research", "Research"),
        (Self::Operations, "operations", "Operations"),
        (Self::Ideas, "ideas", "Ideas"),
    ];
}

impl ChoiceArgument for ReviewFilter {
    const CHOICES: &'static [(Self, &'static str, &'static str)] = &[
        (Self::All, "all", "All records"),
        (Self::Reviewed, "reviewed", "Reviewed"),
        (Self::Unreviewed, "unreviewed", "Unreviewed"),
    ];
}

/// The category filter's choice: every category, or one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CategoryFilter(pub Option<Category>);

impl ChoiceArgument for CategoryFilter {
    const CHOICES: &'static [(Self, &'static str, &'static str)] = &[
        (Self(None), "all", "All categories"),
        (Self(Some(Category::Research)), "research", "Research"),
        (Self(Some(Category::Operations)), "operations", "Operations"),
        (Self(Some(Category::Ideas)), "ideas", "Ideas"),
    ];
}

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
        argument = "choice",
        choices = CategoryFilter
    )]
    FilterCategory,
    #[action(
        label = "Review filter",
        description = "Limit the list by review state",
        argument = "choice",
        choices = ReviewFilter
    )]
    FilterReview,
    #[action(
        label = "Record category",
        description = "Change the draft's category",
        argument = "choice",
        choices = Category
    )]
    EditCategory,
}
