use serde::Serialize;
use stobie_ui_egui::Action;

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "test")]
enum Shifted {
    #[action(label = "Tool", description = "Shift+1 produces !", shortcut = "Shift+1")]
    Digit,
    #[action(label = "Help", description = "Shift+/ produces ?", shortcut = "Cmd+Shift+/")]
    Punctuation,
    #[action(label = "Zoom", description = "Plain punctuation is fine", shortcut = "Cmd+=")]
    Plain,
}

fn main() {}
