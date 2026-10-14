use serde::Serialize;
use stobie_ui_egui::Action;

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "test")]
enum Arguments {
    #[action(label = "Rename", description = "Rename", argument = "colour")]
    Rename,
    #[action(label = "Zoom", description = "Zoom", argument = "number", minimum = 4)]
    Zoom,
    #[action(label = "Scale", description = "Scale", argument = "number", minimum = 2, maximum = -1.5)]
    Scale,
    #[action(label = "Pick", description = "Pick", argument = "choice")]
    Pick,
    #[action(label = "Note", description = "Note", argument = "boolean", max_length = 4)]
    Note,
    #[action(label = "Limit", description = "Limit", minimum = 1)]
    Limit,
    #[action(label = "Title", description = "Title", argument = "text", shortcut = "Cmd+T")]
    Title,
}

fn main() {}
