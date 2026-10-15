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
        if text_bytes + domain_bytes > 4096 {
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
}

/// The framework capability that replaces a text field's text as typing would.
pub const SET_TEXT_CAPABILITY: &str = "stobie.set-text";

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
    pub target: ResolvedTarget,
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
