use serde::Serialize;
use stobie_ui_egui::Action;

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "test")]
enum Unknown {
    #[action(label = "Save", description = "Save", tooltip = "Not an option")]
    Save,
    #[action(label = "Open", label = "Again", description = "Open")]
    Open,
    #[action(label = "Fit", description = "Fit the view", scope = "window")]
    Fit,
    #[action(label = "Copy", description = "Copy", id = "test copy")]
    Copy,
}

fn main() {}
