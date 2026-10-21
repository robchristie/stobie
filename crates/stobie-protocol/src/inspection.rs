//! The version 1 inspection wire protocol: requests, replies, observations,
//! capability descriptions, argument contracts and receipts. Applications
//! serve it through `stobie-ui-egui`; clients use these types directly.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use stobie_core::PaneId;

use crate::{
    ActionScope, ActionShortcut, Availability, DomainReference, SemanticUiId, UiNode, UiRect,
    UiRole, UiSnapshot,
};

/// The wire protocol version. Unknown versions are rejected before dispatch.
pub const INSPECTION_VERSION: u32 = 1;
pub const INSPECTION_QUERY_LIMIT: usize = 256;
pub const INSPECTION_NODE_LIMIT: usize = 4096;
pub const INSPECTION_TEXT_LIMIT: usize = 512;
pub const INSPECTION_PUBLICATION_BYTES: usize = 2 * 1024 * 1024;
pub const INSPECTION_REQUEST_BYTES: usize = 256 * 1024;
pub const INSPECTION_HISTORY_LIMIT: usize = 256;
pub const INSPECTION_QUEUE_LIMIT: usize = 16;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildIdentity {
    pub version: String,
    pub source_revision: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationIdentity {
    pub name: String,
    pub build: BuildIdentity,
}

/// The GPU backend and adapter the application's renderer is running on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RendererIdentity {
    /// `webgpu` or `webgl2` in a browser; natively, the wgpu backend, such as
    /// `vulkan`, `metal`, `dx12` or `gl`.
    pub backend: String,
    /// The adapter name the backend reports; may be empty.
    pub adapter: String,
    pub selection: RendererSelection,
    /// Why this backend was chosen when it was not the preferred one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererSelection {
    /// The preferred backend: WebGPU in a browser, wgpu's default natively.
    Preferred,
    /// The preferred backend was unavailable or failed to start.
    Fallback,
    /// A test override chose the backend.
    Forced,
}

impl RendererSelection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preferred => "preferred",
            Self::Fallback => "fallback",
            Self::Forced => "forced",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationBoundary {
    /// A completed non-discarded UI pass; not a rendered-frame assertion.
    #[default]
    CompletedUiPass,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionObservation {
    pub id: String,
    pub total: Option<u64>,
    pub observed: usize,
    pub offset: Option<u64>,
    pub virtualised: bool,
    pub complete: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionMetadata {
    pub viewport: String,
    pub boundary: ObservationBoundary,
    pub collections: Vec<CollectionObservation>,
}

impl Default for CollectionMetadata {
    fn default() -> Self {
        Self {
            viewport: "main".into(),
            boundary: ObservationBoundary::CompletedUiPass,
            collections: Vec::new(),
        }
    }
}

/// Deliberately bounded observation facts, rather than serialised application memory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ObservationValue {
    Boolean(bool),
    Integer(i64),
    UnsignedInteger(u64),
    Text(String),
}

impl From<bool> for ObservationValue {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}
impl From<u64> for ObservationValue {
    fn from(value: u64) -> Self {
        Self::UnsignedInteger(value)
    }
}
impl From<usize> for ObservationValue {
    fn from(value: usize) -> Self {
        Self::UnsignedInteger(value as u64)
    }
}
impl From<u32> for ObservationValue {
    fn from(value: u32) -> Self {
        Self::UnsignedInteger(value.into())
    }
}
impl From<i64> for ObservationValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}
impl From<i32> for ObservationValue {
    fn from(value: i32) -> Self {
        Self::Integer(value.into())
    }
}
impl From<String> for ObservationValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}
impl From<&str> for ObservationValue {
    fn from(value: &str) -> Self {
        Self::Text(value.into())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompletedObservation {
    pub instance: String,
    pub application: ApplicationIdentity,
    pub id: u64,
    pub snapshot: UiSnapshot,
    pub collection: CollectionMetadata,
    pub facts: BTreeMap<String, ObservationValue>,
}

/// Exact conjunctive selector. Empty selectors are useful for bounded queries;
/// invocation still requires exactly one semantically invocable binding.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectionSelector {
    pub id: Option<SemanticUiId>,
    pub role: Option<UiRole>,
    pub name: Option<String>,
    pub capability: Option<String>,
    pub pane: Option<PaneId>,
    pub domain: Option<DomainReference>,
    /// The argument a control carries, such as the ID of the row it acts on.
    /// It matches only controls bound to an equal value; numbers compare by
    /// value, so `7` matches `7.0`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argument: Option<serde_json::Value>,
}

impl InspectionSelector {
    /// Reject a selector whose text exceeds the 4096-byte bound.
    pub fn validate(&self) -> Result<(), InspectionError> {
        let text_bytes = self.id.as_ref().map_or(0, |id| id.0.len())
            + self.name.as_ref().map_or(0, String::len)
            + self.capability.as_ref().map_or(0, String::len);
        let domain_bytes = self.domain.as_ref().map_or(0, |domain| {
            serde_json::to_vec(domain).expect("domain serialises").len()
        });
        let argument_bytes = self.argument.as_ref().map_or(0, |argument| {
            serde_json::to_vec(argument)
                .expect("argument serialises")
                .len()
        });
        if text_bytes + domain_bytes + argument_bytes > 4096 {
            Err(InspectionError::new(
                InspectionErrorCode::InvalidRequest,
                "selector exceeds 4096 bytes",
            ))
        } else {
            Ok(())
        }
    }

    /// Whether `node` has every property this selector names; a capability
    /// matches a node that presents that action.
    pub fn matches_node(&self, node: &UiNode) -> bool {
        self.id.as_ref().is_none_or(|id| id == &node.id)
            && self.role.is_none_or(|role| role == node.role)
            && self.name.as_ref().is_none_or(|name| name == &node.name)
            && self
                .capability
                .as_ref()
                .is_none_or(|capability| node.actions.iter().any(|action| &action.0 == capability))
            && self.pane.is_none_or(|pane| Some(pane) == node.pane)
            && self
                .domain
                .as_ref()
                .is_none_or(|domain| Some(domain) == node.domain_reference.as_ref())
            && self.argument.as_ref().is_none_or(|argument| {
                node.argument
                    .as_ref()
                    .is_some_and(|bound| same_argument(argument, bound))
            })
    }
}

/// Whether two argument values are equal, comparing numbers by value so that
/// `7`, `7.0` and `7e0` agree.
pub fn same_argument(left: &serde_json::Value, right: &serde_json::Value) -> bool {
    match (left, right) {
        (serde_json::Value::Number(left), serde_json::Value::Number(right)) => {
            left.as_f64() == right.as_f64()
        }
        _ => left == right,
    }
}

/// App-owned meaning distinguishes a logical target from later reuse of its ID.
/// Observation identity records the source, but presentation-frame equality is
/// not required: geometry, focus or repaint alone do not change target meaning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedTarget {
    pub id: SemanticUiId,
    pub capability: String,
    pub pane: Option<PaneId>,
    pub domain: Option<DomainReference>,
    pub meaning: String,
    pub observation: u64,
    /// The argument the target's control carries, such as its row's ID. A
    /// target whose control now carries another argument, or none, is stale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argument: Option<serde_json::Value>,
}

/// The framework capability that replaces a text field's text as typing would.
pub const SET_TEXT_CAPABILITY: &str = "stobie.set-text";

/// The framework capability that opens a link's URL as activating it would.
pub const OPEN_LINK_CAPABILITY: &str = "stobie.open-link";

/// The framework capability that copies a copy control's text to the
/// clipboard as pressing it would. The text is never published: an agent
/// observes the control's confirmation or failure instead.
pub const COPY_TEXT_CAPABILITY: &str = "stobie.copy-text";

/// The value a capability takes, published by discovery. An action takes at
/// most one argument, sent as `"arguments": {"value": …}`. `None` serialises
/// as `"none"`; the others as objects tagged by `kind`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "ContractWire", into = "ContractWire")]
pub enum ArgumentContract {
    None,
    /// A string of at most `max_length` characters, when set.
    Text {
        max_length: Option<u32>,
    },
    Boolean,
    /// A finite number within `minimum..=maximum`, whole when `integer`.
    Number {
        minimum: f64,
        maximum: f64,
        integer: bool,
    },
    /// One of the published option IDs.
    Choice {
        options: Vec<ArgumentOption>,
    },
}

/// One option of an [`ArgumentContract::Choice`]: a stable ID for agents and
/// the label a user sees.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArgumentOption {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum ContractWire {
    Unit(UnitContract),
    Typed(TypedContract),
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum UnitContract {
    None,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum TypedContract {
    Text {
        max_length: Option<u32>,
    },
    Boolean,
    Number {
        minimum: f64,
        maximum: f64,
        integer: bool,
    },
    Choice {
        options: Vec<ArgumentOption>,
    },
}

impl From<ContractWire> for ArgumentContract {
    fn from(wire: ContractWire) -> Self {
        match wire {
            ContractWire::Unit(UnitContract::None) => Self::None,
            ContractWire::Typed(TypedContract::Text { max_length }) => Self::Text { max_length },
            ContractWire::Typed(TypedContract::Boolean) => Self::Boolean,
            ContractWire::Typed(TypedContract::Number {
                minimum,
                maximum,
                integer,
            }) => Self::Number {
                minimum,
                maximum,
                integer,
            },
            ContractWire::Typed(TypedContract::Choice { options }) => Self::Choice { options },
        }
    }
}

impl From<ArgumentContract> for ContractWire {
    fn from(contract: ArgumentContract) -> Self {
        match contract {
            ArgumentContract::None => Self::Unit(UnitContract::None),
            ArgumentContract::Text { max_length } => {
                Self::Typed(TypedContract::Text { max_length })
            }
            ArgumentContract::Boolean => Self::Typed(TypedContract::Boolean),
            ArgumentContract::Number {
                minimum,
                maximum,
                integer,
            } => Self::Typed(TypedContract::Number {
                minimum,
                maximum,
                integer,
            }),
            ArgumentContract::Choice { options } => Self::Typed(TypedContract::Choice { options }),
        }
    }
}

/// The largest whole number an [`ActionArgument`] carries exactly, 2^53 − 1;
/// its negation is the smallest. Arguments travel as JSON numbers, which are
/// `f64`.
pub const MAX_EXACT_INTEGER: i64 = (1 << 53) - 1;

/// A whole number outside ±(2^53 − 1) ([`MAX_EXACT_INTEGER`]), which an
/// [`ActionArgument`] cannot carry exactly; it holds the value as written.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InexactInteger(pub String);

impl std::fmt::Display for InexactInteger {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} cannot be represented exactly: an integer argument must lie within \
             ±{MAX_EXACT_INTEGER}",
            self.0
        )
    }
}

impl std::error::Error for InexactInteger {}

/// A validated argument, delivered to the application with its action.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum ActionArgument {
    /// The action takes no argument.
    #[default]
    None,
    Text(String),
    Boolean(bool),
    Number(f64),
    /// The chosen option's ID.
    Choice(String),
}

impl ActionArgument {
    /// A whole-number argument, such as a row's ID. Numbers travel as `f64`,
    /// so an ID must lie within ±(2^53 − 1) ([`MAX_EXACT_INTEGER`]) to stay
    /// exact.
    ///
    /// # Panics
    ///
    /// Panics, in release builds too, when `value` lies outside that range,
    /// rather than round it to another ID. Use [`Self::try_integer`] for a
    /// value that might.
    pub fn integer(value: i64) -> Self {
        Self::try_integer(value).unwrap_or_else(|error| panic!("{error}"))
    }

    /// A whole-number argument, or [`InexactInteger`] when `value` lies
    /// outside ±(2^53 − 1) ([`MAX_EXACT_INTEGER`]), which a number cannot
    /// carry exactly. Takes any integer type, such as `u64` or `usize`.
    pub fn try_integer<T>(value: T) -> Result<Self, InexactInteger>
    where
        T: TryInto<i64> + std::fmt::Display + Copy,
    {
        match value.try_into() {
            Ok(exact) if exact.unsigned_abs() <= MAX_EXACT_INTEGER.unsigned_abs() => {
                Ok(Self::Number(exact as f64))
            }
            _ => Err(InexactInteger(value.to_string())),
        }
    }

    /// The argument as the JSON value an invocation sends in
    /// `{"value": …}`, or `None` for no argument. A whole number within
    /// ±(2^53 − 1) is an integer.
    pub fn to_value(&self) -> Option<serde_json::Value> {
        match self {
            Self::None => None,
            Self::Text(text) | Self::Choice(text) => Some(serde_json::Value::String(text.clone())),
            Self::Boolean(value) => Some(serde_json::Value::Bool(*value)),
            Self::Number(value) => Some(
                if value.fract() == 0.0 && value.abs() < 9_007_199_254_740_992.0 {
                    serde_json::Value::from(*value as i64)
                } else {
                    serde_json::Number::from_f64(*value)
                        .map_or(serde_json::Value::Null, serde_json::Value::Number)
                },
            ),
        }
    }

    /// A choice argument for `value`, as a control presenting the choice
    /// would trigger it.
    pub fn choice<C: ChoiceArgument>(value: C) -> Self {
        Self::Choice(value.choice_id().to_owned())
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            Self::Boolean(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            _ => None,
        }
    }

    /// The chosen option as the application's choice type.
    pub fn as_choice<C: ChoiceArgument>(&self) -> Result<C, String> {
        match self {
            Self::Choice(id) => {
                C::from_choice_id(id).ok_or_else(|| format!("unknown option {id:?}"))
            }
            _ => Err("expected a choice".into()),
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::None => "no argument",
            Self::Text(_) => "text",
            Self::Boolean(_) => "a boolean",
            Self::Number(_) => "a number",
            Self::Choice(_) => "a choice",
        }
    }
}

impl From<i32> for ActionArgument {
    fn from(value: i32) -> Self {
        Self::Number(value.into())
    }
}

impl From<u32> for ActionArgument {
    fn from(value: u32) -> Self {
        Self::Number(value.into())
    }
}

/// See [`ActionArgument::integer`]: the value must lie within ±(2^53 − 1),
/// and one outside it panics. Use [`ActionArgument::try_integer`] for a
/// value that might lie outside it.
impl From<i64> for ActionArgument {
    fn from(value: i64) -> Self {
        Self::integer(value)
    }
}

/// See [`ActionArgument::integer`]: the value must be below 2^53, and a
/// larger one panics. Use [`ActionArgument::try_integer`] for a value that
/// might be larger.
impl From<u64> for ActionArgument {
    fn from(value: u64) -> Self {
        Self::try_integer(value).unwrap_or_else(|error| panic!("{error}"))
    }
}

impl From<bool> for ActionArgument {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

impl From<&str> for ActionArgument {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<String> for ActionArgument {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

/// A closed set of options an action can take, such as an application's
/// category enum. Option IDs are stable external identities; labels are what
/// a user sees.
pub trait ChoiceArgument: Copy + PartialEq + 'static {
    /// Every option in presentation order: value, stable ID and label.
    const CHOICES: &'static [(Self, &'static str, &'static str)];

    fn choice_id(self) -> &'static str {
        Self::CHOICES
            .iter()
            .find(|(value, ..)| *value == self)
            .map(|(_, id, _)| *id)
            .expect("every value is listed in CHOICES")
    }

    fn from_choice_id(id: &str) -> Option<Self> {
        Self::CHOICES
            .iter()
            .find(|(_, option, _)| *option == id)
            .map(|(value, ..)| *value)
    }

    /// The published options. Option IDs must be distinct.
    fn options() -> Vec<ArgumentOption> {
        debug_assert!(
            Self::CHOICES
                .iter()
                .enumerate()
                .all(|(index, (_, id, _))| Self::CHOICES[..index]
                    .iter()
                    .all(|(_, other, _)| other != id)),
            "choice option IDs must be distinct"
        );
        Self::CHOICES
            .iter()
            .map(|(_, id, label)| ArgumentOption {
                id: (*id).to_owned(),
                label: (*label).to_owned(),
            })
            .collect()
    }
}

impl ArgumentContract {
    /// Validate a wire `arguments` value: missing, `null` or `{}` for no
    /// argument, otherwise exactly `{"value": …}` of the contract's kind.
    pub fn parse(&self, arguments: &serde_json::Value) -> Result<ActionArgument, InspectionError> {
        let empty =
            arguments.is_null() || arguments.as_object().is_some_and(serde_json::Map::is_empty);
        if *self == Self::None {
            return if empty {
                Ok(ActionArgument::None)
            } else {
                Err(invalid_arguments("this capability accepts no arguments"))
            };
        }
        let expected = self.expected();
        let value = arguments
            .as_object()
            .filter(|object| object.len() == 1)
            .and_then(|object| object.get("value"))
            .ok_or_else(|| {
                invalid_arguments(format!(
                    "this capability requires arguments {{\"value\": …}} with {expected}"
                ))
            })?;
        let argument = match (self, value) {
            (Self::Text { .. }, serde_json::Value::String(text)) => {
                ActionArgument::Text(text.clone())
            }
            (Self::Boolean, serde_json::Value::Bool(value)) => ActionArgument::Boolean(*value),
            (Self::Number { .. }, serde_json::Value::Number(number)) => {
                ActionArgument::Number(number.as_f64().unwrap_or(f64::NAN))
            }
            (Self::Choice { .. }, serde_json::Value::String(id)) => {
                ActionArgument::Choice(id.clone())
            }
            _ => return Err(invalid_arguments(format!("value must be {expected}"))),
        };
        self.validate(&argument)?;
        Ok(argument)
    }

    /// Check a typed argument against the contract, as for a physical trigger.
    pub fn validate(&self, argument: &ActionArgument) -> Result<(), InspectionError> {
        let expected = self.expected();
        match (self, argument) {
            (Self::None, ActionArgument::None) | (Self::Boolean, ActionArgument::Boolean(_)) => {
                Ok(())
            }
            (Self::Text { max_length }, ActionArgument::Text(text)) => {
                // Longer than the limit when a character exists past it.
                match max_length.filter(|limit| text.chars().nth(*limit as usize).is_some()) {
                    Some(limit) => Err(invalid_arguments(format!(
                        "text exceeds {limit} characters"
                    ))),
                    None => Ok(()),
                }
            }
            (
                Self::Number {
                    minimum,
                    maximum,
                    integer,
                },
                ActionArgument::Number(value),
            ) => {
                let in_range = value.is_finite() && value >= minimum && value <= maximum;
                if in_range && (!*integer || value.fract() == 0.0) {
                    Ok(())
                } else {
                    Err(invalid_arguments(format!(
                        "value must be {expected}, not {value}"
                    )))
                }
            }
            (Self::Choice { options }, ActionArgument::Choice(id)) => {
                if options.iter().any(|option| &option.id == id) {
                    Ok(())
                } else {
                    Err(invalid_arguments(format!(
                        "unknown option {id:?}; value must be {expected}"
                    )))
                }
            }
            (Self::None, argument) => Err(invalid_arguments(format!(
                "this capability accepts no arguments, not {}",
                argument.kind()
            ))),
            (_, argument) => Err(invalid_arguments(format!(
                "value must be {expected}, not {}",
                argument.kind()
            ))),
        }
    }

    /// What the contract accepts, for error messages.
    fn expected(&self) -> String {
        match self {
            Self::None => "no argument".into(),
            Self::Text {
                max_length: Some(limit),
            } => format!("a string of at most {limit} characters"),
            Self::Text { max_length: None } => "a string".into(),
            Self::Boolean => "true or false".into(),
            Self::Number {
                minimum,
                maximum,
                integer,
            } => format!(
                "{} from {minimum} to {maximum}",
                if *integer { "an integer" } else { "a number" }
            ),
            Self::Choice { options } => {
                let ids: Vec<String> = options
                    .iter()
                    .take(16)
                    .map(|option| format!("{:?}", option.id))
                    .collect();
                let more = if options.len() > ids.len() {
                    ", …"
                } else {
                    ""
                };
                format!("one of {}{more}", ids.join(", "))
            }
        }
    }
}

fn invalid_arguments(message: impl Into<String>) -> InspectionError {
    InspectionError::new(InspectionErrorCode::InvalidArguments, message)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CapabilityDescription {
    pub id: SemanticUiId,
    pub capability: String,
    pub pane: Option<PaneId>,
    pub domain: Option<DomainReference>,
    pub label: String,
    pub description: String,
    pub scope: ActionScope,
    pub shortcut: Option<ActionShortcut>,
    pub availability: Availability,
    /// Whether this binding's individual control is in the retained UI pass.
    pub rendered: bool,
    pub control_enabled: Option<bool>,
    pub semantic_invocable: bool,
    pub arguments: ArgumentContract,
    /// The argument this binding's control carries, such as the ID of the row
    /// it acts on. Invoking it sends no value, or this one; absent for a
    /// binding that takes any value its contract allows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argument: Option<serde_json::Value>,
    pub target: ResolvedTarget,
}

impl CapabilityDescription {
    /// Validate a wire `arguments` value for this binding. A binding whose
    /// control carries an argument takes no value, which means that argument,
    /// or the same value again; any other value is invalid.
    pub fn parse_arguments(
        &self,
        arguments: &serde_json::Value,
    ) -> Result<ActionArgument, InspectionError> {
        let Some(bound) = &self.argument else {
            return self.arguments.parse(arguments);
        };
        let empty =
            arguments.is_null() || arguments.as_object().is_some_and(serde_json::Map::is_empty);
        let carried = self
            .arguments
            .parse(&serde_json::json!({ "value": bound }))?;
        if empty {
            return Ok(carried);
        }
        let given = self.arguments.parse(arguments)?;
        if given == carried {
            Ok(given)
        } else {
            Err(invalid_arguments(format!(
                "this control acts on {bound}; send no value, or that value"
            )))
        }
    }

    /// Whether this binding can take the value in wire `arguments`: any
    /// binding when no value is sent, and otherwise an unbound binding or one
    /// whose control carries an equal value.
    pub fn accepts_value(&self, arguments: &serde_json::Value) -> bool {
        match (sent_value(arguments), &self.argument) {
            (Some(value), Some(bound)) => same_argument(value, bound),
            _ => true,
        }
    }
}

/// The value in wire `arguments` of the form `{"value": …}`, if any.
fn sent_value(arguments: &serde_json::Value) -> Option<&serde_json::Value> {
    arguments
        .as_object()
        .filter(|object| object.len() == 1)
        .and_then(|object| object.get("value"))
}

/// Choose the one binding an invocation addresses among those its selector
/// matches. A value sent with the invocation also selects: a binding whose
/// control carries another value cannot take it, and one whose control
/// carries that value, such as the row with that ID, is preferred over the
/// action's own binding that takes any value. A selector that matches one
/// binding alone keeps it, so that its own contract refuses another value
/// ([`CapabilityDescription::parse_arguments`]); among several, a value that
/// none can take matches nothing.
pub fn select_invocation_target<'a, T>(
    candidates: &'a [T],
    describe: impl Fn(&T) -> &CapabilityDescription,
    arguments: &serde_json::Value,
) -> Result<&'a T, InspectionError> {
    let accepting: Vec<&T> = candidates
        .iter()
        .filter(|candidate| describe(candidate).accepts_value(arguments))
        .collect();
    let carrying: Vec<&T> = accepting
        .iter()
        .copied()
        .filter(|candidate| describe(candidate).argument.is_some())
        .collect();
    let chosen = if sent_value(arguments).is_some() && carrying.len() == 1 {
        &carrying
    } else {
        &accepting
    };
    match chosen.as_slice() {
        [one] => Ok(one),
        [] if candidates.len() == 1 => Ok(&candidates[0]),
        [] if candidates.is_empty() => Err(InspectionError::new(
            InspectionErrorCode::NoMatch,
            "selector resolves no capability binding",
        )),
        [] => Err(InspectionError::new(
            InspectionErrorCode::NoMatch,
            "no matching control carries this value",
        )),
        _ => Err(InspectionError::new(
            InspectionErrorCode::Ambiguous,
            "selector resolves multiple capability bindings",
        )),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectionCursor {
    pub observation: u64,
    pub offset: usize,
}

fn default_limit() -> usize {
    64
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum InspectionOperation {
    Hello,
    Observe,
    Query {
        #[serde(default)]
        selector: InspectionSelector,
        #[serde(default = "default_limit")]
        limit: usize,
        #[serde(default)]
        cursor: Option<InspectionCursor>,
    },
    Discover {
        #[serde(default)]
        selector: InspectionSelector,
        #[serde(default = "default_limit")]
        limit: usize,
        #[serde(default)]
        cursor: Option<InspectionCursor>,
    },
    Invoke {
        selector: InspectionSelector,
        expected: ResolvedTarget,
        #[serde(default)]
        arguments: serde_json::Value,
    },
    Receipt {
        request_id: String,
    },
    Cancel {
        request_id: String,
    },
}

/// Exhaustive wire operation names advertised during version negotiation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectionOperationKind {
    Hello,
    Observe,
    Query,
    Discover,
    Invoke,
    Receipt,
    Cancel,
}

impl InspectionOperation {
    pub fn kind(&self) -> InspectionOperationKind {
        match self {
            Self::Hello => InspectionOperationKind::Hello,
            Self::Observe => InspectionOperationKind::Observe,
            Self::Query { .. } => InspectionOperationKind::Query,
            Self::Discover { .. } => InspectionOperationKind::Discover,
            Self::Invoke { .. } => InspectionOperationKind::Invoke,
            Self::Receipt { .. } => InspectionOperationKind::Receipt,
            Self::Cancel { .. } => InspectionOperationKind::Cancel,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct InspectionRequest {
    pub version: u32,
    pub request_id: String,
    pub instance: Option<String>,
    pub operation: InspectionOperation,
}

impl<'de> Deserialize<'de> for InspectionRequest {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Envelope {
            version: u32,
            request_id: String,
            #[serde(default)]
            instance: Option<String>,
            operation: serde_json::Value,
        }
        let envelope = Envelope::deserialize(deserializer)?;
        let object = envelope
            .operation
            .as_object()
            .ok_or_else(|| serde::de::Error::custom("operation must be an object"))?;
        let op = object
            .get("op")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| serde::de::Error::custom("operation requires a string op"))?;
        let allowed: &[&str] = match op {
            "hello" | "observe" => &["op"],
            "query" | "discover" => &["op", "selector", "limit", "cursor"],
            "invoke" => &["op", "selector", "expected", "arguments"],
            "receipt" | "cancel" => &["op", "request_id"],
            _ => return Err(serde::de::Error::custom("unknown operation")),
        };
        if object.keys().any(|key| !allowed.contains(&key.as_str())) {
            return Err(serde::de::Error::custom("unknown operation field"));
        }
        let operation =
            serde_json::from_value(envelope.operation).map_err(serde::de::Error::custom)?;
        Ok(Self {
            version: envelope.version,
            request_id: envelope.request_id,
            instance: envelope.instance,
            operation,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectionErrorCode {
    UnsupportedVersion,
    UnsupportedOperation,
    InvalidRequest,
    WrongInstance,
    MissingObservation,
    NoMatch,
    Ambiguous,
    Unavailable,
    NotInvocable,
    StaleTarget,
    InvalidArguments,
    RequestIdConflict,
    HistoryFull,
    QueueFull,
    StaleCursor,
    PublicationLimit,
    ValidationFailed,
    UnknownRequest,
    TooLate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectionError {
    pub code: InspectionErrorCode,
    pub message: String,
}

impl InspectionError {
    pub fn new(code: InspectionErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(InspectionErrorCode::ValidationFailed, message)
    }
}

impl std::fmt::Display for InspectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}
impl std::error::Error for InspectionError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptState {
    Unknown,
    Queued,
    Completed,
    Rejected,
    Cancelled,
    Uncertain,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationReceipt {
    pub request_id: String,
    pub state: ReceiptState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_observation: Option<u64>,
    /// For a completed invocation, the first observation that includes its
    /// effect. Wait for an observation at least this new before reading the
    /// outcome or discovering targets whose meaning it changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_observation: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<ResolvedTarget>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<InspectionError>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObservedNode {
    pub node: UiNode,
    pub targets: Vec<ResolvedTarget>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectionLimits {
    pub query: usize,
    pub nodes: usize,
    pub text: usize,
    pub publication_bytes: usize,
    pub request_bytes: usize,
    pub mutations: usize,
    pub pending: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InspectionResult {
    Hello {
        application: ApplicationIdentity,
        limits: InspectionLimits,
        operations: Vec<InspectionOperationKind>,
        /// Absent when the host does not know its renderer, as in tests.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        renderer: Option<RendererIdentity>,
    },
    Observe {
        observation: CompletedObservation,
    },
    Query {
        observation: u64,
        total: usize,
        nodes: Vec<ObservedNode>,
        next_cursor: Option<InspectionCursor>,
    },
    Discover {
        observation: u64,
        total: usize,
        capabilities: Vec<CapabilityDescription>,
        next_cursor: Option<InspectionCursor>,
    },
    Receipt {
        receipt: InvocationReceipt,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InspectionReply {
    pub version: u32,
    pub request_id: String,
    pub instance: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<InspectionResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<InspectionError>,
}

/// Validated current physical allocation, in both logical and physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhysicalTarget {
    pub logical_rect: UiRect,
    pub physical_rect: UiRect,
    pub centre: [f32; 2],
    pub pixels_per_point: f32,
}

/// Resolve one current node and validate scale, finite positive geometry,
/// enabled state and exact root containment before supplying pointer pixels.
/// Call again from a fresh observation before each physical interaction.
pub fn resolve_physical_target(
    observation: &CompletedObservation,
    selector: &InspectionSelector,
) -> Result<PhysicalTarget, InspectionError> {
    selector.validate()?;
    let snapshot = &observation.snapshot;
    let mut matches = snapshot
        .nodes
        .iter()
        .filter(|node| selector.matches_node(node));
    let node = matches.next().ok_or_else(|| {
        InspectionError::new(InspectionErrorCode::NoMatch, "no physical target matches")
    })?;
    if matches.next().is_some() {
        return Err(InspectionError::new(
            InspectionErrorCode::Ambiguous,
            "multiple physical targets match",
        ));
    }
    if !node.enabled {
        return Err(InspectionError::new(
            InspectionErrorCode::Unavailable,
            "physical target is disabled",
        ));
    }
    let root = snapshot
        .node(&snapshot.root)
        .ok_or_else(|| InspectionError::validation("snapshot has no root allocation"))?;
    let scale = snapshot.pixels_per_point;
    if !scale.is_finite()
        || scale <= 0.0
        || !root.rect.is_positive()
        || !node.rect.is_positive()
        || !root.rect.contains(node.rect, 0.0)
    {
        return Err(InspectionError::validation(
            "physical target requires finite positive scale and geometry within root",
        ));
    }
    let logical = node.rect;
    let physical = UiRect {
        min_x: logical.min_x * scale,
        min_y: logical.min_y * scale,
        max_x: logical.max_x * scale,
        max_y: logical.max_y * scale,
    };
    let centre = [
        physical.min_x / 2.0 + physical.max_x / 2.0,
        physical.min_y / 2.0 + physical.max_y / 2.0,
    ];
    if !physical.is_positive() || centre.iter().any(|value| !value.is_finite()) {
        return Err(InspectionError::validation(
            "scaled physical target geometry is not finite and positive",
        ));
    }
    Ok(PhysicalTarget {
        logical_rect: logical,
        physical_rect: physical,
        centre,
        pixels_per_point: scale,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn binding(id: &str, argument: Option<serde_json::Value>) -> CapabilityDescription {
        let target = ResolvedTarget {
            id: SemanticUiId::new(id),
            capability: "open".into(),
            pane: None,
            domain: None,
            meaning: "stable".into(),
            observation: 1,
            argument: argument.clone(),
        };
        CapabilityDescription {
            id: target.id.clone(),
            capability: target.capability.clone(),
            pane: None,
            domain: None,
            label: "Open".into(),
            description: "Open the row with this ID".into(),
            scope: ActionScope::Application,
            shortcut: None,
            availability: Availability::Enabled,
            rendered: argument.is_some(),
            control_enabled: None,
            semantic_invocable: true,
            arguments: ArgumentContract::Number {
                minimum: 1.0,
                maximum: 100.0,
                integer: true,
            },
            argument,
            target,
        }
    }

    #[test]
    fn integers_beyond_what_a_number_carries_exactly_are_refused() {
        let edge = MAX_EXACT_INTEGER;
        assert_eq!(ActionArgument::integer(edge).to_value(), Some(json!(edge)));
        assert_eq!(
            ActionArgument::integer(-edge).to_value(),
            Some(json!(-edge))
        );
        assert_eq!(
            ActionArgument::try_integer(edge as u64),
            Ok(ActionArgument::Number(edge as f64))
        );
        for refused in [edge + 1, -edge - 1, i64::MAX, i64::MIN] {
            let error = ActionArgument::try_integer(refused).unwrap_err();
            assert_eq!(error, InexactInteger(refused.to_string()));
            assert!(error.to_string().contains("cannot be represented exactly"));
        }
        assert!(ActionArgument::try_integer(u64::MAX).is_err());
        // The infallible forms refuse the same values, in release builds too.
        for refused in [
            std::panic::catch_unwind(|| ActionArgument::integer(edge + 1)),
            std::panic::catch_unwind(|| ActionArgument::from(edge + 1)),
            std::panic::catch_unwind(|| ActionArgument::from((edge + 1) as u64)),
            std::panic::catch_unwind(|| ActionArgument::from(u64::MAX)),
        ] {
            let message = refused.unwrap_err();
            let message = message.downcast_ref::<String>().unwrap();
            assert!(
                message.contains("cannot be represented exactly"),
                "{message}"
            );
        }
    }

    #[test]
    fn arguments_have_one_wire_value_and_compare_numbers_by_value() {
        assert_eq!(ActionArgument::None.to_value(), None);
        assert_eq!(ActionArgument::from(42_u32).to_value(), Some(json!(42)));
        assert_eq!(ActionArgument::integer(-3).to_value(), Some(json!(-3)));
        assert_eq!(ActionArgument::Number(2.5).to_value(), Some(json!(2.5)));
        assert_eq!(ActionArgument::from("1013").to_value(), Some(json!("1013")));
        assert_eq!(ActionArgument::from(true).to_value(), Some(json!(true)));
        assert_eq!(
            ActionArgument::Choice("ideas".into()).to_value(),
            Some(json!("ideas"))
        );
        assert!(same_argument(&json!(7), &json!(7.0)));
        assert!(!same_argument(&json!(7), &json!("7")));
        assert!(same_argument(&json!("a"), &json!("a")));
    }

    #[test]
    fn selectors_match_the_argument_a_node_carries() {
        let selector: InspectionSelector =
            serde_json::from_value(json!({ "capability": "open", "argument": 2 })).unwrap();
        let mut node = UiNode::container(
            SemanticUiId::new("row-2"),
            None,
            UiRole::Button,
            UiRect::default(),
        );
        node.actions.push(crate::SemanticActionId("open".into()));
        assert!(
            !selector.matches_node(&node),
            "an unbound node carries nothing"
        );
        node.argument = Some(json!(2.0));
        assert!(selector.matches_node(&node));
        node.argument = Some(json!(3));
        assert!(!selector.matches_node(&node));
        // Selectors without an argument keep their wire form.
        assert_eq!(
            serde_json::to_value(InspectionSelector::default()).unwrap(),
            json!({ "id": null, "role": null, "name": null, "capability": null, "pane": null, "domain": null })
        );
        assert!(
            InspectionSelector {
                argument: Some(json!("x".repeat(4097))),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn a_rows_binding_takes_no_value_or_its_own() {
        let row = binding("row-2", Some(json!(2)));
        assert_eq!(
            row.parse_arguments(&serde_json::Value::Null),
            Ok(ActionArgument::Number(2.0))
        );
        assert_eq!(
            row.parse_arguments(&json!({ "value": 2.0 })),
            Ok(ActionArgument::Number(2.0))
        );
        let refused = row.parse_arguments(&json!({ "value": 3 })).unwrap_err();
        assert_eq!(refused.code, InspectionErrorCode::InvalidArguments);
        assert_eq!(
            refused.message,
            "this control acts on 2; send no value, or that value"
        );
        // The action's own binding takes any value its contract allows.
        let action = binding("action.open", None);
        assert_eq!(
            action.parse_arguments(&json!({ "value": 3 })),
            Ok(ActionArgument::Number(3.0))
        );
        assert!(action.parse_arguments(&serde_json::Value::Null).is_err());
    }

    #[test]
    fn a_value_selects_the_control_carrying_it_before_the_actions_binding() {
        let candidates = [
            binding("row-1", Some(json!(1))),
            binding("row-2", Some(json!(2))),
            binding("action.open", None),
        ];
        let select = |arguments: serde_json::Value| {
            select_invocation_target(&candidates, |candidate| candidate, &arguments)
                .map(|chosen| chosen.id.0.as_str())
                .map_err(|error| error.code)
        };
        assert_eq!(select(json!({ "value": 2 })), Ok("row-2"));
        assert_eq!(select(json!({ "value": 9 })), Ok("action.open"));
        assert_eq!(
            select(serde_json::Value::Null),
            Err(InspectionErrorCode::Ambiguous)
        );
        // A control selected alone is kept, so that its own contract
        // refuses another value.
        let grace = &candidates[1..2];
        let chosen =
            select_invocation_target(grace, |candidate| candidate, &json!({ "value": 9 })).unwrap();
        assert_eq!(
            chosen
                .parse_arguments(&json!({ "value": 9 }))
                .unwrap_err()
                .code,
            InspectionErrorCode::InvalidArguments
        );
        // Without the action's binding, a value no row carries matches nothing.
        let rows = &candidates[..2];
        assert_eq!(
            select_invocation_target(rows, |candidate| candidate, &json!({ "value": 9 }))
                .map_err(|error| error.code)
                .unwrap_err(),
            InspectionErrorCode::NoMatch
        );
        let none: [CapabilityDescription; 0] = [];
        assert_eq!(
            select_invocation_target(&none, |candidate| candidate, &serde_json::Value::Null)
                .unwrap_err()
                .code,
            InspectionErrorCode::NoMatch
        );
    }
}
