use serde::Serialize;
use stobie_ui_egui::Action;

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "test")]
enum Duplicates {
    #[action(label = "Undo", description = "Undo one change", shortcut = "Cmd+Z")]
    Undo,
    #[action(label = "Revert", description = "Same chord as Undo", shortcut = "Ctrl+z")]
    Revert,
    #[action(label = "Reset", description = "Reset the layout", id = "test.undo")]
    Reset,
}

fn main() {}
