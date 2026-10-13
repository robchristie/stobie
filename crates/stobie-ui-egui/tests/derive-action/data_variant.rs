use serde::Serialize;
use stobie_ui_egui::Action;

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "test")]
enum Carries {
    #[action(label = "Open", description = "Open one record")]
    Open(u32),
}

fn main() {}
