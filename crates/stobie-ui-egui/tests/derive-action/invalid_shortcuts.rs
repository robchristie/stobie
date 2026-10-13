use serde::Serialize;
use stobie_ui_egui::Action;

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "test")]
enum Shortcuts {
    #[action(label = "One", description = "Unknown key", shortcut = "Cmd+Banana")]
    UnknownKey,
    #[action(label = "Two", description = "Tab is reserved", shortcut = "Shift+Tab")]
    TabKey,
    #[action(label = "Three", description = "Modifier only", shortcut = "Cmd+Shift")]
    ModifierOnly,
    #[action(label = "Four", description = "Modifier after key", shortcut = "Z+Cmd")]
    ModifierAfterKey,
    #[action(label = "Five", description = "Repeated modifier", shortcut = "Cmd+Ctrl+Z")]
    RepeatedModifier,
    #[action(label = "Six", description = "Empty part", shortcut = "Cmd++")]
    EmptyPart,
}

fn main() {}
