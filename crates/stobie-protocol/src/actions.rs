//! Application-owned action identity and its presentation metadata, which
//! discovery publishes: stable IDs, labels, shortcuts, scope, availability
//! and argument contracts.

use std::borrow::Cow;
use std::hash::Hash;

use serde::{Deserialize, Serialize};

/// Application-owned identity for a user-visible capability.
///
/// Implementations own both the stable external identity and the presentation
/// metadata used by generic egui controls. Action keys route capabilities into
/// the application's intent and command system; they are not a second mutation
/// architecture. Prefer `#[derive(Action)]`, which implements this trait and
/// [`ActionSet`] from declarative attributes and checks them at compile time.
pub trait ActionKey: Copy + Eq + Hash + Ord + Serialize {
    /// Stable external capability identity shared by semantics and automation.
    /// Keep it independent of translated/compact labels and collection positions.
    fn stable_id(self) -> &'static str;
    /// Presentation and routing metadata for this application-owned capability.
    fn specification(self) -> ActionSpec<Self>;
    /// Whether agents may invoke this capability through semantic inspection.
    /// Actions are invocable unless they declare why physical input is required.
    fn invocation(self) -> ActionInvocation {
        ActionInvocation::Semantic
    }
    /// The argument this capability takes, published by discovery and
    /// validated before the application receives it. Most actions take none.
    fn argument(self) -> crate::ArgumentContract {
        crate::ArgumentContract::None
    }
}

/// The complete, closed set of an application's actions, used to discover,
/// dispatch and audit every capability without a hand-maintained list.
pub trait ActionSet: ActionKey + std::fmt::Debug + 'static {
    /// Every action, in declaration order.
    const ALL: &'static [Self];
}

/// How a capability may be exercised by an inspection client.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionInvocation {
    /// The action can run through the same validated route as a click.
    Semantic,
    /// The action needs physical input, for example because it takes a value
    /// argument. The reason records the application's decision.
    PhysicalOnly { reason: &'static str },
}

impl ActionInvocation {
    pub const fn semantic(self) -> bool {
        matches!(self, Self::Semantic)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionScope {
    Application,
    ActivePane,
    Pane,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ActionShortcut {
    pub command: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: ShortcutKey,
}

impl ActionShortcut {
    pub const fn new(key: ShortcutKey) -> Self {
        Self {
            command: false,
            shift: false,
            alt: false,
            key,
        }
    }

    pub const fn command(key: ShortcutKey) -> Self {
        Self {
            command: true,
            ..Self::new(key)
        }
    }

    pub const fn command_shift(key: ShortcutKey) -> Self {
        Self {
            command: true,
            shift: true,
            alt: false,
            key,
        }
    }

    /// The shortcut as an `aria-keyshortcuts` value, which assistive
    /// technology announces. Cmd is Control, or Meta on macOS; in the browser,
    /// where the platform is decided at run time, a command shortcut lists
    /// both forms.
    pub fn aria_keyshortcuts(self) -> String {
        let key = match self.key {
            ShortcutKey::ArrowUp => "ArrowUp",
            ShortcutKey::ArrowDown => "ArrowDown",
            ShortcutKey::ArrowLeft => "ArrowLeft",
            ShortcutKey::ArrowRight => "ArrowRight",
            ShortcutKey::PageUp => "PageUp",
            ShortcutKey::PageDown => "PageDown",
            // Printable keys use their character.
            ShortcutKey::Comma
            | ShortcutKey::Period
            | ShortcutKey::Minus
            | ShortcutKey::Equals
            | ShortcutKey::Slash => self.key.label(),
            key => key.name(),
        };
        let mut chord = String::new();
        if self.shift {
            chord.push_str("Shift+");
        }
        if self.alt {
            chord.push_str("Alt+");
        }
        chord.push_str(key);
        if !self.command {
            chord
        } else if cfg!(target_arch = "wasm32") {
            format!("Control+{chord} Meta+{chord}")
        } else if cfg!(target_os = "macos") {
            format!("Meta+{chord}")
        } else {
            format!("Control+{chord}")
        }
    }

    pub fn display(self) -> String {
        let mut parts = Vec::new();
        if self.command {
            parts.push("Ctrl/Cmd");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.alt {
            parts.push("Alt");
        }
        parts.push(self.key.label());
        parts.join("+")
    }
}

macro_rules! shortcut_keys {
    ($($variant:ident => $key:literal, $label:literal;)*) => {
        /// A key that an application shortcut can bind. Tab is deliberately absent:
        /// it belongs to keyboard focus traversal.
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum ShortcutKey {
            $($variant,)*
        }

        impl ShortcutKey {
            /// Every bindable key, in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];

            /// The key's standard name, which UI toolkits such as egui parse:
            /// `0`, `A`, `ArrowUp`, `PageUp`, `Comma` or `F1`.
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $key,)*
                }
            }

            pub const fn label(self) -> &'static str {
                match self {
                    $(Self::$variant => $label,)*
                }
            }
        }
    };
}

// `#[derive(Action)]` keeps a matching table of shortcut spellings; the derive
// tests bind every key here so the two cannot drift silently. `stobie-ui-egui`
// maps each name to its egui key and tests that every name parses.
shortcut_keys! {
    A => "A", "A";
    B => "B", "B";
    C => "C", "C";
    D => "D", "D";
    E => "E", "E";
    F => "F", "F";
    G => "G", "G";
    H => "H", "H";
    I => "I", "I";
    J => "J", "J";
    K => "K", "K";
    L => "L", "L";
    M => "M", "M";
    N => "N", "N";
    O => "O", "O";
    P => "P", "P";
    Q => "Q", "Q";
    R => "R", "R";
    S => "S", "S";
    T => "T", "T";
    U => "U", "U";
    V => "V", "V";
    W => "W", "W";
    X => "X", "X";
    Y => "Y", "Y";
    Z => "Z", "Z";
    Zero => "0", "0";
    One => "1", "1";
    Two => "2", "2";
    Three => "3", "3";
    Four => "4", "4";
    Five => "5", "5";
    Six => "6", "6";
    Seven => "7", "7";
    Eight => "8", "8";
    Nine => "9", "9";
    Enter => "Enter", "Enter";
    Escape => "Escape", "Escape";
    Space => "Space", "Space";
    Backspace => "Backspace", "Backspace";
    Delete => "Delete", "Delete";
    Insert => "Insert", "Insert";
    Home => "Home", "Home";
    End => "End", "End";
    PageUp => "PageUp", "Page Up";
    PageDown => "PageDown", "Page Down";
    ArrowUp => "ArrowUp", "Up";
    ArrowDown => "ArrowDown", "Down";
    ArrowLeft => "ArrowLeft", "Left";
    ArrowRight => "ArrowRight", "Right";
    Comma => "Comma", ",";
    Period => "Period", ".";
    Minus => "Minus", "-";
    Equals => "Equals", "=";
    Slash => "Slash", "/";
    F1 => "F1", "F1";
    F2 => "F2", "F2";
    F3 => "F3", "F3";
    F4 => "F4", "F4";
    F5 => "F5", "F5";
    F6 => "F6", "F6";
    F7 => "F7", "F7";
    F8 => "F8", "F8";
    F9 => "F9", "F9";
    F10 => "F10", "F10";
    F11 => "F11", "F11";
    F12 => "F12", "F12";
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ActionSpec<A: ActionKey> {
    /// The typed capability described by these fields.
    pub id: A,
    /// Full visible and accessible label.
    pub label: &'static str,
    /// User-facing purpose, also used by observable control semantics.
    pub description: &'static str,
    /// Optional short display label; the full label remains available to semantics.
    pub compact_label: Option<&'static str>,
    /// Optional application-routed shortcut; declaring it alone does not dispatch it.
    pub shortcut: Option<ActionShortcut>,
    /// Application, active-pane or explicit-pane routing scope.
    pub scope: ActionScope,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Availability {
    Enabled,
    Disabled { reason: Cow<'static, str> },
    Hidden,
}

impl Availability {
    pub const fn enabled(&self) -> bool {
        matches!(self, Self::Enabled)
    }

    pub const fn visible(&self) -> bool {
        !matches!(self, Self::Hidden)
    }

    pub fn disabled_reason(&self) -> Option<&str> {
        match self {
            Self::Disabled { reason } => Some(reason),
            Self::Enabled | Self::Hidden => None,
        }
    }
}
