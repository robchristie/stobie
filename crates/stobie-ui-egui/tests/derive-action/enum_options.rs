use serde::Serialize;
use stobie_ui_egui::Action;

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "test.")]
enum TrailingDot {
    #[action(label = "Save", description = "Save")]
    Save,
}

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "test app")]
enum SpacedPrefix {
    #[action(label = "Save", description = "Save")]
    Save,
}

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "test", label = "Not an enum option")]
enum VariantKeyOnEnum {
    #[action(label = "Save", description = "Save")]
    Save,
}

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "test", crate = "stobie_ui_egui", crate = "again")]
enum RepeatedCrate {
    #[action(label = "Save", description = "Save")]
    Save,
}

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "test")]
enum Empty {}

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "test")]
enum Generic<T> {
    #[action(label = "Save", description = "Save")]
    Save,
    #[action(label = "Never", description = "Uses the parameter")]
    Never(std::marker::PhantomData<T>),
}

fn main() {}
