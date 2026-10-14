//! `#[derive(Action)]` expansion behaviour and its compile-time diagnostics.

use std::collections::BTreeSet;

use serde::Serialize;
use stobie_ui_egui::{
    Action, ActionArgument, ActionInvocation, ActionKey, ActionScope, ActionSet, ActionShortcut,
    ArgumentContract, ArgumentOption, ChoiceArgument, ShortcutKey,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "record-desk")]
enum Desk {
    #[action(
        label = "Apply",
        description = "Commit the draft",
        shortcut = "Cmd+Enter"
    )]
    Apply,
    #[action(
        label = "Reset filters",
        description = "Clear every filter",
        shortcut = "cmd+shift+f"
    )]
    ResetFilters,
    #[action(
        label = "Fit view",
        compact = "Fit",
        description = "Fit the active view",
        scope = "active_pane",
        shortcut = "Alt+F1",
        id = "fit_view"
    )]
    FitView,
    #[action(
        label = "Category",
        description = "Choose a category",
        scope = "pane",
        physical_only = "Opens a chooser in a dialog"
    )]
    Category,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Colour {
    Red,
    Blue,
}

impl ChoiceArgument for Colour {
    const CHOICES: &'static [(Self, &'static str, &'static str)] =
        &[(Self::Red, "red", "Red"), (Self::Blue, "blue", "Blue")];
}

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "notes")]
enum Typed {
    #[action(
        label = "Rename",
        description = "Rename the note",
        argument = "text",
        max_length = 40
    )]
    Rename,
    #[action(
        label = "Pin",
        description = "Pin or unpin the note",
        argument = "boolean"
    )]
    Pin,
    #[action(
        label = "Zoom",
        description = "Zoom the note",
        argument = "number",
        minimum = -2,
        maximum = 8.5
    )]
    Zoom,
    #[action(
        label = "Copies",
        description = "Set the number of copies",
        argument = "number",
        minimum = 1,
        maximum = 9,
        integer = true
    )]
    Copies,
    #[action(label = "Colour", description = "Colour the note", argument = "choice", choices = Colour)]
    Colour,
    #[action(label = "Clear", description = "Clear the note", shortcut = "Cmd+K")]
    Clear,
}

#[test]
fn expansion_publishes_each_argument_contract() {
    assert_eq!(
        Typed::Rename.argument(),
        ArgumentContract::Text {
            max_length: Some(40)
        }
    );
    assert_eq!(Typed::Pin.argument(), ArgumentContract::Boolean);
    assert_eq!(
        Typed::Zoom.argument(),
        ArgumentContract::Number {
            minimum: -2.0,
            maximum: 8.5,
            integer: false
        }
    );
    assert_eq!(
        Typed::Copies.argument(),
        ArgumentContract::Number {
            minimum: 1.0,
            maximum: 9.0,
            integer: true
        }
    );
    assert_eq!(
        Typed::Colour.argument(),
        ArgumentContract::Choice {
            options: vec![
                ArgumentOption {
                    id: "red".into(),
                    label: "Red".into()
                },
                ArgumentOption {
                    id: "blue".into(),
                    label: "Blue".into()
                },
            ]
        }
    );
    assert_eq!(Typed::Clear.argument(), ArgumentContract::None);
    assert!(Typed::Colour.invocation().semantic());
    assert_eq!(
        ActionArgument::choice(Colour::Blue).as_choice::<Colour>(),
        Ok(Colour::Blue)
    );
}

#[test]
fn expansion_declares_every_action_once_with_its_metadata() {
    assert_eq!(
        Desk::ALL,
        &[
            Desk::Apply,
            Desk::ResetFilters,
            Desk::FitView,
            Desk::Category
        ]
    );
    let ids: Vec<_> = Desk::ALL.iter().map(|action| action.stable_id()).collect();
    assert_eq!(
        ids,
        [
            "record-desk.apply",
            "record-desk.reset-filters",
            "fit_view",
            "record-desk.category"
        ]
    );
    let reset = Desk::ResetFilters.specification();
    assert_eq!(reset.id, Desk::ResetFilters);
    assert_eq!(reset.label, "Reset filters");
    assert_eq!(reset.description, "Clear every filter");
    assert_eq!(reset.compact_label, None);
    assert_eq!(reset.scope, ActionScope::Application);
    assert_eq!(
        reset.shortcut,
        Some(ActionShortcut {
            command: true,
            shift: true,
            alt: false,
            key: ShortcutKey::F,
        })
    );
    let fit = Desk::FitView.specification();
    assert_eq!(fit.compact_label, Some("Fit"));
    assert_eq!(fit.scope, ActionScope::ActivePane);
    assert_eq!(fit.shortcut.unwrap().display(), "Alt+F1");
    assert_eq!(Desk::Category.specification().scope, ActionScope::Pane);
    assert_eq!(Desk::Category.specification().shortcut, None);
}

#[test]
fn actions_are_invocable_unless_they_state_why_not() {
    assert_eq!(Desk::Apply.invocation(), ActionInvocation::Semantic);
    assert_eq!(
        Desk::Category.invocation(),
        ActionInvocation::PhysicalOnly {
            reason: "Opens a chooser in a dialog"
        }
    );
}

// Bind every key through the derive so its spelling table cannot drift from
// `ShortcutKey`: a missing variant fails to compile, a missing spelling fails here.
macro_rules! every_key {
    ($($variant:ident $shortcut:literal),* $(,)?) => {
        #[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[action(prefix = "keys")]
        enum Keys {
            $(#[action(label = "Key", description = "Shortcut coverage", shortcut = $shortcut)]
            $variant,)*
        }
    };
}

every_key! {
    A "A", B "b", C "C", D "D", E "E", F "F", G "G", H "H", I "I", J "J", K "K",
    L "L", M "M", N "N", O "O", P "P", Q "Q", R "R", S "S", T "T", U "U", V "V",
    W "W", X "X", Y "Y", Z "Z", Zero "0", One "1", Two "2", Three "3", Four "4",
    Five "5", Six "6", Seven "7", Eight "8", Nine "9", Enter "Return", Escape "Esc",
    Space "Space", Backspace "Backspace", Delete "Del", Insert "Insert", Home "Home",
    End "End", PageUp "PageUp", PageDown "PageDown", ArrowUp "Up", ArrowDown "ArrowDown",
    ArrowLeft "Left", ArrowRight "ArrowRight", Comma ",", Period "Period", Minus "-",
    Equals "Equals", Slash "/", F1 "F1", F2 "F2", F3 "F3", F4 "F4", F5 "F5", F6 "F6",
    F7 "F7", F8 "F8", F9 "F9", F10 "F10", F11 "F11", F12 "F12",
}

#[test]
fn every_shortcut_key_has_a_derive_spelling() {
    let bound: BTreeSet<_> = Keys::ALL
        .iter()
        .map(|action| action.specification().shortcut.unwrap().key)
        .collect();
    let all: BTreeSet<_> = ShortcutKey::ALL.iter().copied().collect();
    assert_eq!(bound, all);
    assert_eq!(Keys::ALL.len(), ShortcutKey::ALL.len());
}

#[test]
fn invalid_declarations_are_compile_errors_at_their_spans() {
    trybuild::TestCases::new().compile_fail("tests/derive-action/*.rs");
}
