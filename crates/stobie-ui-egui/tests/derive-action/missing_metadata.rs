use serde::Serialize;
use stobie_ui_egui::Action;

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "test")]
enum Missing {
    Unannotated,
    #[action(label = "Save")]
    NoDescription,
    #[action(label = " ", description = "Blank labels are rejected")]
    BlankLabel,
}

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
enum NoPrefix {
    #[action(label = "Save", description = "Save every record")]
    Save,
}

fn main() {}
