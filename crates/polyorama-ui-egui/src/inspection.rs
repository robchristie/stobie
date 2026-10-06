//! Retained, versioned inspection and validated application action exercise.
//!
//! The host publishes only a completed, non-discarded egui UI pass. Reads use
//! that retained observation and never wake the UI. Publications describe UI
//! submission, not GPU presentation or a proved application postcondition.
//! Invocation queues only application-owned typed targets; the UI thread must
//! recalculate bindings before each [`Inspection::drain`] call and execute the
//! target through its existing validated intent/command route.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use polyorama_core::PaneId;
use serde::{Deserialize, Serialize};

use crate::{
    ActionKey, ActionScope, ActionShortcut, ActionTarget, Availability, DomainReference,
    SemanticUiId, UiNode, UiRect, UiRole, UiSnapshot,
};

#[cfg(all(unix, not(target_arch = "wasm32")))]
mod native;
#[cfg(all(unix, not(target_arch = "wasm32")))]
pub use native::NativeInspectionHost;

/// Native socket hosting is unavailable on non-Unix hosts. The opt-in start
/// method reports that limitation while ordinary application builds remain portable.
#[cfg(all(not(unix), not(target_arch = "wasm32")))]
pub struct NativeInspectionHost;

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

#[derive(Clone, Debug)]
pub struct InspectionBinding<A: ActionKey> {
    pub target: ActionTarget<A>,
    pub id: SemanticUiId,
    pub domain: Option<DomainReference>,
    pub availability: Availability,
    pub semantic_invocable: bool,
    pub meaning: String,
}

impl<A: ActionKey> InspectionBinding<A> {
    pub fn new(
        target: ActionTarget<A>,
        availability: Availability,
        semantic_invocable: bool,
        meaning: impl Into<String>,
    ) -> Self {
        Self {
            id: target.semantic_id().into(),
            target,
            domain: None,
            availability,
            semantic_invocable,
            meaning: meaning.into(),
        }
    }

    pub fn with_id(mut self, id: SemanticUiId) -> Self {
        self.id = id;
        self
    }
    pub fn with_domain(mut self, domain: DomainReference) -> Self {
        self.domain = Some(domain);
        self
    }

    fn describe(&self, observation: u64) -> CapabilityDescription {
        let spec = self.target.action.specification();
        CapabilityDescription {
            id: self.id.clone(),
            capability: self.target.action.stable_id().into(),
            pane: self.target.pane,
            domain: self.domain.clone(),
            label: spec.label.into(),
            description: spec.description.into(),
            scope: spec.scope,
            shortcut: spec.shortcut,
            availability: self.availability.clone(),
            rendered: false,
            control_enabled: None,
            semantic_invocable: self.semantic_invocable,
            arguments: ArgumentContract::None,
            target: ResolvedTarget {
                id: self.id.clone(),
                capability: self.target.action.stable_id().into(),
                pane: self.target.pane,
                domain: self.domain.clone(),
                meaning: self.meaning.clone(),
                observation,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArgumentContract {
    None,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

struct Publication {
    observation: CompletedObservation,
    capabilities: Vec<CapabilityDescription>,
}
struct LedgerEntry {
    body: Vec<u8>,
    receipt: InvocationReceipt,
    operation: InspectionOperation,
    executing: bool,
}
struct StagedPublication {
    viewport: egui::ViewportId,
    pass: u64,
    publication: Publication,
}
struct InspectionState {
    application: ApplicationIdentity,
    instance: String,
    publication: Option<Publication>,
    observation: u64,
    ledger: BTreeMap<String, LedgerEntry>,
    pending: VecDeque<String>,
    waker: Option<Arc<dyn Fn() + Send + Sync>>,
    staged: Option<StagedPublication>,
}

/// A clone shares retained observations and a bounded, non-evicting mutation
/// ledger. No worker thread owns or inspects application model memory.
#[derive(Clone)]
pub struct Inspection {
    state: Arc<Mutex<InspectionState>>,
}

impl Inspection {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(application: impl Into<String>) -> Self {
        Self::with_build_identity(application, BuildIdentity::default())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_build_identity(application: impl Into<String>, build: BuildIdentity) -> Self {
        static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);
        let serial = NEXT_INSTANCE.fetch_add(1, Ordering::Relaxed);
        let epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let instance = format!("{:x}-{epoch:x}-{serial:x}", std::process::id());
        Self::with_instance(application, build, instance)
            .expect("generated native instance is valid")
    }

    /// Supply an opaque host nonce unique across app restarts. Browser hosts
    /// should use `crypto.randomUUID()` rather than a frame counter or label.
    /// This constructor uses neither process APIs nor an unsupported WASM clock.
    pub fn with_instance(
        application: impl Into<String>,
        build: BuildIdentity,
        instance: impl Into<String>,
    ) -> Result<Self, InspectionError> {
        let application = application.into();
        let instance = instance.into();
        if instance.is_empty() || instance.len() > 128 {
            return Err(InspectionError::new(
                InspectionErrorCode::InvalidRequest,
                "host instance nonce must contain 1..128 bytes",
            ));
        }
        if application.len() > 256
            || build.version.len() > 128
            || build
                .source_revision
                .as_ref()
                .is_some_and(|revision| revision.len() > 128)
        {
            return Err(InspectionError::new(
                InspectionErrorCode::InvalidRequest,
                "application/build identity exceeds its byte bounds",
            ));
        }
        Ok(Self {
            state: Arc::new(Mutex::new(InspectionState {
                application: ApplicationIdentity {
                    name: application,
                    build,
                },
                instance,
                publication: None,
                observation: 0,
                ledger: BTreeMap::new(),
                pending: VecDeque::new(),
                waker: None,
                staged: None,
            })),
        })
    }

    pub fn instance(&self) -> String {
        self.state
            .lock()
            .expect("inspection mutex poisoned")
            .instance
            .clone()
    }

    /// Install once before the first app update, after other UI plugins. Later
    /// plugins must not request discard after this completion hook has run.
    /// Reads do not install hooks or request a repaint.
    pub fn install_completion_hook(&self, context: &egui::Context) {
        let key = egui::Id::new(("polyorama.inspection.completion", self.instance()));
        let install = context.data_mut(|data| {
            if data.get_temp::<bool>(key).unwrap_or(false) {
                false
            } else {
                data.insert_temp(key, true);
                true
            }
        });
        if install {
            let inspection = self.clone();
            context.on_end_pass(
                "polyorama.inspection.completed",
                Arc::new(move |ui| {
                    let context = ui.ctx();
                    let mut state = inspection.state.lock().expect("inspection mutex poisoned");
                    let matches = state.staged.as_ref().is_some_and(|staged| {
                        staged.viewport == context.viewport_id()
                            && staged.pass == context.cumulative_pass_nr()
                    });
                    if matches {
                        let staged = state.staged.take().expect("matched staged publication");
                        if !context.will_discard() {
                            state.commit(staged.publication);
                        }
                    }
                }),
            );
        }
    }

    /// Stage owned observations for the installed completion hook. No model
    /// reference survives the call. A discarded pass never becomes observable.
    pub fn stage_completed<A: ActionKey>(
        &self,
        context: &egui::Context,
        snapshot: UiSnapshot,
        bindings: &[InspectionBinding<A>],
        metadata: CollectionMetadata,
        facts: BTreeMap<String, ObservationValue>,
    ) -> Result<(), InspectionError> {
        let publication = prepare_publication(snapshot, bindings, metadata, facts)?;
        self.state.lock().expect("inspection mutex poisoned").staged = Some(StagedPublication {
            viewport: context.viewport_id(),
            pass: context.cumulative_pass_nr(),
            publication,
        });
        Ok(())
    }

    /// Publish an already completed UI submission atomically. Hosts using egui
    /// end-pass hooks should prefer [`Self::stage_completed`].
    pub fn publish<A: ActionKey>(
        &self,
        snapshot: UiSnapshot,
        bindings: &[InspectionBinding<A>],
        metadata: CollectionMetadata,
        facts: BTreeMap<String, ObservationValue>,
    ) -> Result<u64, InspectionError> {
        let publication = prepare_publication(snapshot, bindings, metadata, facts)?;
        Ok(self
            .state
            .lock()
            .expect("inspection mutex poisoned")
            .commit(publication))
    }

    /// Service reads from retained state, or admit a mutation to the bounded
    /// queue. A request ID is never evicted or silently reused within an instance.
    pub fn handle(&self, request: InspectionRequest) -> InspectionReply {
        let mut state = self.state.lock().expect("inspection mutex poisoned");
        let pending_before = state.pending.len();
        let result = state.handle(&request);
        let wake = if state.pending.len() > pending_before {
            state.waker.clone()
        } else {
            None
        };
        let reply = state.reply(request.request_id, result);
        drop(state);
        if let Some(wake) = wake {
            wake();
        }
        reply
    }

    /// Parse the same wire envelope on every host. Parsing/size errors never
    /// admit a mutation. IDs from malformed JSON cannot be relied upon.
    pub fn handle_json(&self, json: &str) -> String {
        if json.len() <= INSPECTION_REQUEST_BYTES
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(json)
            && let Some(version) = value.get("version").and_then(|v| v.as_u64())
            && version != u64::from(INSPECTION_VERSION)
        {
            let request_id = value
                .get("request_id")
                .and_then(|v| v.as_str())
                .filter(|id| !id.is_empty() && id.len() <= 128)
                .unwrap_or_default()
                .to_owned();
            return serde_json::to_string(&self.error_reply(
                request_id,
                InspectionError::new(
                    InspectionErrorCode::UnsupportedVersion,
                    "protocol version is not supported",
                ),
            ))
            .expect("inspection reply serialises");
        }
        // Unknown operation names are distinct from malformed arguments. Use
        // the shared operation type rather than a second string registry.
        if json.len() <= INSPECTION_REQUEST_BYTES
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(json)
            && let Some(operation) = value.get("operation").and_then(|v| v.get("op"))
            && operation.is_string()
            && serde_json::from_value::<InspectionOperationKind>(operation.clone()).is_err()
        {
            let request_id = value
                .get("request_id")
                .and_then(|v| v.as_str())
                .filter(|id| !id.is_empty() && id.len() <= 128)
                .unwrap_or_default()
                .to_owned();
            return serde_json::to_string(&self.error_reply(
                request_id,
                InspectionError::new(
                    InspectionErrorCode::UnsupportedOperation,
                    "operation is not supported by this protocol",
                ),
            ))
            .expect("inspection reply serialises");
        }
        let reply = if json.len() > INSPECTION_REQUEST_BYTES {
            self.error_reply(
                String::new(),
                InspectionError::new(
                    InspectionErrorCode::InvalidRequest,
                    "request exceeds byte limit",
                ),
            )
        } else {
            match serde_json::from_str::<InspectionRequest>(json) {
                Ok(request) => self.handle(request),
                Err(error) => self.error_reply(
                    serde_json::from_str::<serde_json::Value>(json)
                        .ok()
                        .and_then(|value| {
                            value
                                .get("request_id")
                                .and_then(|id| id.as_str())
                                .filter(|id| !id.is_empty() && id.len() <= 128)
                                .map(str::to_owned)
                        })
                        .unwrap_or_default(),
                    InspectionError::new(
                        InspectionErrorCode::InvalidRequest,
                        format!("invalid request envelope: {error}"),
                    ),
                ),
            }
        };
        serde_json::to_string(&reply).expect("inspection reply serialises")
    }

    fn error_reply(&self, request_id: String, error: InspectionError) -> InspectionReply {
        self.state
            .lock()
            .expect("inspection mutex poisoned")
            .reply(request_id, Err(error))
    }

    pub fn has_pending(&self) -> bool {
        !self
            .state
            .lock()
            .expect("inspection mutex poisoned")
            .pending
            .is_empty()
    }

    /// Execute at most one queued mutation. Recalculate bindings from current
    /// application state before every call, including between queued actions.
    /// Success proves validated dispatch only, never a later observed postcondition.
    pub fn drain<A: ActionKey>(
        &self,
        current: &[InspectionBinding<A>],
        mut execute: impl FnMut(ActionTarget<A>) -> Result<(), InspectionError>,
    ) -> bool {
        let mut state = self.state.lock().expect("inspection mutex poisoned");
        let Some(request_id) = state.pending.pop_front() else {
            return false;
        };
        let operation = state
            .ledger
            .get(&request_id)
            .expect("queued ledger entry")
            .operation
            .clone();
        let InspectionOperation::Invoke {
            selector, expected, ..
        } = operation
        else {
            unreachable!("only invocation is queued")
        };
        let validation = state.resolve_current(current, &selector, &expected);
        state
            .ledger
            .get_mut(&request_id)
            .expect("queued ledger entry")
            .executing = true;
        drop(state);
        let outcome = validation.and_then(&mut execute);
        let mut state = self.state.lock().expect("inspection mutex poisoned");
        let entry = state
            .ledger
            .get_mut(&request_id)
            .expect("ledger entries are never evicted");
        entry.executing = false;
        entry.receipt.state = if outcome.is_ok() {
            ReceiptState::Completed
        } else {
            ReceiptState::Rejected
        };
        entry.receipt.error = outcome.err();
        true
    }

    /// Convenience for a browser host already running on the app thread. This
    /// drains one request only; a host with other pending work must recalculate
    /// bindings before continuing to drain.
    pub fn dispatch<A: ActionKey>(
        &self,
        request: InspectionRequest,
        current: &[InspectionBinding<A>],
        execute: impl FnMut(ActionTarget<A>) -> Result<(), InspectionError>,
    ) -> InspectionReply {
        let invocation = matches!(request.operation, InspectionOperation::Invoke { .. });
        let request_id = request.request_id.clone();
        let reply = self.handle(request);
        let queued = matches!(&reply.result, Some(InspectionResult::Receipt { receipt })
            if receipt.state == ReceiptState::Queued);
        if invocation && queued {
            self.drain(current, execute);
            let state = self.state.lock().expect("inspection mutex poisoned");
            if let Some(entry) = state.ledger.get(&request_id) {
                return state.reply(
                    request_id,
                    Ok(InspectionResult::Receipt {
                        receipt: entry.receipt.clone(),
                    }),
                );
            }
        }
        reply
    }

    #[cfg(all(unix, not(target_arch = "wasm32")))]
    pub fn start_native(
        &self,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> std::io::Result<Option<NativeInspectionHost>> {
        let Some(path) = std::env::var_os("POLYORAMA_AUTOMATION_SOCKET") else {
            return Ok(None);
        };
        NativeInspectionHost::start(self.clone(), path.into(), Arc::new(wake)).map(Some)
    }

    #[cfg(all(not(unix), not(target_arch = "wasm32")))]
    pub fn start_native(
        &self,
        _wake: impl Fn() + Send + Sync + 'static,
    ) -> std::io::Result<Option<NativeInspectionHost>> {
        if std::env::var_os("POLYORAMA_AUTOMATION_SOCKET").is_none() {
            return Ok(None);
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "private native inspection sockets require a Unix host",
        ))
    }
}

impl InspectionState {
    fn commit(&mut self, mut publication: Publication) -> u64 {
        self.observation = self
            .observation
            .checked_add(1)
            .expect("observation counter exhausted");
        publication.observation.id = self.observation;
        publication.observation.instance = self.instance.clone();
        publication.observation.application = self.application.clone();
        for capability in &mut publication.capabilities {
            capability.target.observation = self.observation;
            let node = publication.observation.snapshot.node(&capability.id);
            capability.rendered = node.is_some();
            capability.control_enabled = node.map(|node| node.enabled);
        }
        self.publication = Some(publication);
        self.observation
    }

    fn reply(
        &self,
        request_id: String,
        outcome: Result<InspectionResult, InspectionError>,
    ) -> InspectionReply {
        let (result, error) = match outcome {
            Ok(result) => (Some(result), None),
            Err(error) => (None, Some(error)),
        };
        InspectionReply {
            version: INSPECTION_VERSION,
            request_id,
            instance: self.instance.clone(),
            result,
            error,
        }
    }

    fn handle(&mut self, request: &InspectionRequest) -> Result<InspectionResult, InspectionError> {
        use InspectionErrorCode as Code;
        if request.version != INSPECTION_VERSION {
            return Err(InspectionError::new(
                Code::UnsupportedVersion,
                "supported protocol version is 1",
            ));
        }
        if request.request_id.is_empty() || request.request_id.len() > 128 {
            return Err(InspectionError::new(
                Code::InvalidRequest,
                "request_id must contain 1..128 bytes",
            ));
        }
        if serde_json::to_vec(request)
            .expect("request serialises")
            .len()
            > INSPECTION_REQUEST_BYTES
        {
            return Err(InspectionError::new(
                Code::InvalidRequest,
                "request exceeds byte limit",
            ));
        }
        if !matches!(request.operation, InspectionOperation::Hello) && request.instance.is_none() {
            return Err(InspectionError::new(
                Code::WrongInstance,
                "instance is required; hello before addressing the application",
            ));
        }
        if !matches!(request.operation, InspectionOperation::Hello)
            && request.instance.as_deref() != Some(&self.instance)
        {
            if matches!(request.operation, InspectionOperation::Receipt { .. }) {
                let InspectionOperation::Receipt { request_id } = &request.operation else {
                    unreachable!()
                };
                return Ok(InspectionResult::Receipt {
                    receipt: InvocationReceipt {
                        request_id: request_id.clone(),
                        state: ReceiptState::Uncertain,
                        route: None,
                        before_observation: None,
                        target: None,
                        error: Some(InspectionError::new(
                            Code::WrongInstance,
                            "receipt outcome is unknown across application instances",
                        )),
                    },
                });
            }
            return Err(InspectionError::new(
                Code::WrongInstance,
                "instance is missing or changed; hello and observe before targeting the current application",
            ));
        }
        if let Some(entry) = self.ledger.get(&request.request_id) {
            let body = serde_json::to_vec(request).expect("request serialises");
            if entry.body == body {
                return Ok(InspectionResult::Receipt {
                    receipt: entry.receipt.clone(),
                });
            }
            return Err(InspectionError::new(
                Code::RequestIdConflict,
                "request_id was already admitted with a different request body",
            ));
        }
        match &request.operation {
            InspectionOperation::Hello => Ok(InspectionResult::Hello {
                application: self.application.clone(),
                operations: vec![
                    InspectionOperationKind::Hello,
                    InspectionOperationKind::Observe,
                    InspectionOperationKind::Query,
                    InspectionOperationKind::Discover,
                    InspectionOperationKind::Invoke,
                    InspectionOperationKind::Receipt,
                    InspectionOperationKind::Cancel,
                ],
                limits: InspectionLimits {
                    query: INSPECTION_QUERY_LIMIT,
                    nodes: INSPECTION_NODE_LIMIT,
                    text: INSPECTION_TEXT_LIMIT,
                    publication_bytes: INSPECTION_PUBLICATION_BYTES,
                    request_bytes: INSPECTION_REQUEST_BYTES,
                    mutations: INSPECTION_HISTORY_LIMIT,
                    pending: INSPECTION_QUEUE_LIMIT,
                },
            }),
            InspectionOperation::Observe => Ok(InspectionResult::Observe {
                observation: self.publication()?.observation.clone(),
            }),
            InspectionOperation::Query {
                selector,
                limit,
                cursor,
            } => {
                validate_selector(selector)?;
                let publication = self.publication()?;
                let nodes: Vec<_> = publication
                    .observation
                    .snapshot
                    .nodes
                    .iter()
                    .filter(|node| selector_matches_node(selector, node))
                    .map(|node| ObservedNode {
                        node: node.clone(),
                        targets: publication
                            .capabilities
                            .iter()
                            .filter(|capability| capability.id == node.id)
                            .map(|capability| capability.target.clone())
                            .collect(),
                    })
                    .collect();
                let (start, end, next_cursor) =
                    page_bounds(publication.observation.id, nodes.len(), *limit, *cursor)?;
                Ok(InspectionResult::Query {
                    observation: publication.observation.id,
                    total: nodes.len(),
                    nodes: nodes[start..end].to_vec(),
                    next_cursor,
                })
            }
            InspectionOperation::Discover {
                selector,
                limit,
                cursor,
            } => {
                validate_selector(selector)?;
                let publication = self.publication()?;
                let capabilities: Vec<_> = publication
                    .capabilities
                    .iter()
                    .filter(|capability| {
                        selector_matches_binding(
                            selector,
                            capability,
                            publication.observation.snapshot.node(&capability.id),
                        )
                    })
                    .cloned()
                    .collect();
                let (start, end, next_cursor) = page_bounds(
                    publication.observation.id,
                    capabilities.len(),
                    *limit,
                    *cursor,
                )?;
                Ok(InspectionResult::Discover {
                    observation: publication.observation.id,
                    total: capabilities.len(),
                    capabilities: capabilities[start..end].to_vec(),
                    next_cursor,
                })
            }
            InspectionOperation::Invoke {
                selector,
                expected,
                arguments,
            } => {
                if self.ledger.len() >= INSPECTION_HISTORY_LIMIT {
                    return Err(InspectionError::new(
                        Code::HistoryFull,
                        "mutation history is full; restart explicitly to establish a new instance",
                    ));
                }
                let validation = validate_selector(selector)
                    .and_then(|()| validate_arguments(arguments))
                    .and_then(|()| self.resolve_published(selector, expected));
                let validation = validation.and_then(|()| {
                    if self.pending.len() >= INSPECTION_QUEUE_LIMIT {
                        Err(InspectionError::new(
                            Code::QueueFull,
                            "pending invocation queue is full",
                        ))
                    } else {
                        Ok(())
                    }
                });
                let receipt = InvocationReceipt {
                    request_id: request.request_id.clone(),
                    state: if validation.is_ok() {
                        ReceiptState::Queued
                    } else {
                        ReceiptState::Rejected
                    },
                    route: Some("semantic".into()),
                    before_observation: self
                        .publication
                        .as_ref()
                        .map(|publication| publication.observation.id),
                    target: Some(expected.clone()),
                    error: validation.err(),
                };
                if receipt.state == ReceiptState::Queued {
                    self.pending.push_back(request.request_id.clone());
                }
                self.ledger.insert(
                    request.request_id.clone(),
                    LedgerEntry {
                        body: serde_json::to_vec(request).expect("request serialises"),
                        receipt: receipt.clone(),
                        operation: request.operation.clone(),
                        executing: false,
                    },
                );
                Ok(InspectionResult::Receipt { receipt })
            }
            InspectionOperation::Receipt { request_id } => {
                let receipt = self.ledger.get(request_id).map_or_else(
                    || InvocationReceipt {
                        request_id: request_id.clone(),
                        state: ReceiptState::Unknown,
                        route: None,
                        before_observation: None,
                        target: None,
                        error: None,
                    },
                    |entry| entry.receipt.clone(),
                );
                Ok(InspectionResult::Receipt { receipt })
            }
            InspectionOperation::Cancel { request_id } => {
                let entry = self.ledger.get_mut(request_id).ok_or_else(|| {
                    InspectionError::new(
                        Code::UnknownRequest,
                        "no mutation was admitted with this request_id",
                    )
                })?;
                if entry.executing || entry.receipt.state != ReceiptState::Queued {
                    return Err(InspectionError::new(
                        Code::TooLate,
                        "only a queued, unstarted invocation can be cancelled",
                    ));
                }
                entry.receipt.state = ReceiptState::Cancelled;
                self.pending.retain(|pending| pending != request_id);
                Ok(InspectionResult::Receipt {
                    receipt: entry.receipt.clone(),
                })
            }
        }
    }

    fn publication(&self) -> Result<&Publication, InspectionError> {
        self.publication.as_ref().ok_or_else(|| {
            InspectionError::new(
                InspectionErrorCode::MissingObservation,
                "no completed UI observation has been published",
            )
        })
    }

    fn resolve_published(
        &self,
        selector: &InspectionSelector,
        expected: &ResolvedTarget,
    ) -> Result<(), InspectionError> {
        let publication = self.publication()?;
        let candidates: Vec<_> = publication
            .capabilities
            .iter()
            .filter(|capability| {
                selector_matches_binding(
                    selector,
                    capability,
                    publication.observation.snapshot.node(&capability.id),
                )
            })
            .collect();
        let capability = unique_binding(&candidates)?;
        validate_expected(&capability.target, expected)?;
        validate_invocable(capability)
    }

    fn resolve_current<A: ActionKey>(
        &self,
        bindings: &[InspectionBinding<A>],
        selector: &InspectionSelector,
        expected: &ResolvedTarget,
    ) -> Result<ActionTarget<A>, InspectionError> {
        let publication = self.publication()?;
        let candidates: Vec<_> = bindings
            .iter()
            .filter(|binding| {
                selector_matches_binding(
                    selector,
                    &binding.describe(publication.observation.id),
                    publication.observation.snapshot.node(&binding.id),
                )
            })
            .collect();
        if candidates.is_empty() {
            return Err(InspectionError::new(
                InspectionErrorCode::StaleTarget,
                "target disappeared before execution",
            ));
        }
        if candidates.len() != 1 {
            return Err(InspectionError::new(
                InspectionErrorCode::Ambiguous,
                "current selector resolves multiple bindings",
            ));
        }
        let binding = candidates[0];
        let description = binding.describe(publication.observation.id);
        validate_expected(&description.target, expected)?;
        validate_invocable(&description)?;
        Ok(binding.target)
    }
}

fn prepare_publication<A: ActionKey>(
    snapshot: UiSnapshot,
    bindings: &[InspectionBinding<A>],
    metadata: CollectionMetadata,
    facts: BTreeMap<String, ObservationValue>,
) -> Result<Publication, InspectionError> {
    use InspectionErrorCode as Code;
    if snapshot.nodes.len() > INSPECTION_NODE_LIMIT
        || snapshot.text.len() > INSPECTION_TEXT_LIMIT
        || bindings.len() > INSPECTION_NODE_LIMIT
        || metadata.collections.len() > 256
        || facts.len() > 256
    {
        return Err(InspectionError::new(
            Code::PublicationLimit,
            "completed observation exceeds collection limits",
        ));
    }
    let mut ids = BTreeSet::new();
    if snapshot.nodes.iter().any(|node| !ids.insert(&node.id)) {
        return Err(InspectionError::new(
            Code::InvalidRequest,
            "snapshot node IDs must be unique",
        ));
    }
    if !snapshot.pixels_per_point.is_finite()
        || snapshot.pixels_per_point <= 0.0
        || snapshot.node(&snapshot.root).is_none()
        || snapshot.nodes.iter().any(|node| !node.rect.is_finite())
    {
        return Err(InspectionError::new(
            Code::InvalidRequest,
            "snapshot requires a root, finite geometry and finite positive pixels_per_point",
        ));
    }
    let mut targets = BTreeSet::new();
    for binding in bindings {
        let scope_matches = (binding.target.action.specification().scope
            == ActionScope::Application)
            == binding.target.pane.is_none();
        if binding.id.0.is_empty()
            || binding.meaning.is_empty()
            || !scope_matches
            || !targets.insert((&binding.id, binding.target.action.stable_id()))
        {
            return Err(InspectionError::new(
                Code::InvalidRequest,
                "bindings require unique IDs/capabilities, meaning and valid typed scope",
            ));
        }
    }
    let publication = Publication {
        observation: CompletedObservation {
            instance: String::new(),
            application: ApplicationIdentity::default(),
            id: 0,
            snapshot,
            collection: metadata,
            facts,
        },
        capabilities: bindings.iter().map(|binding| binding.describe(0)).collect(),
    };
    let observation_size = serde_json::to_vec(&publication.observation)
        .map_err(|error| {
            InspectionError::new(
                Code::InvalidRequest,
                format!("observation cannot serialise: {error}"),
            )
        })?
        .len();
    let capability_size = serde_json::to_vec(&publication.capabilities)
        .map_err(|error| {
            InspectionError::new(
                Code::InvalidRequest,
                format!("capabilities cannot serialise: {error}"),
            )
        })?
        .len();
    // Reserve space for committed instance/build identity and reply framing.
    if observation_size + capability_size + 4096 > INSPECTION_PUBLICATION_BYTES {
        return Err(InspectionError::new(
            Code::PublicationLimit,
            "completed observation exceeds byte limit",
        ));
    }
    Ok(publication)
}

fn validate_selector(selector: &InspectionSelector) -> Result<(), InspectionError> {
    let text_bytes = selector.id.as_ref().map_or(0, |id| id.0.len())
        + selector.name.as_ref().map_or(0, String::len)
        + selector.capability.as_ref().map_or(0, String::len);
    let domain_bytes = selector.domain.as_ref().map_or(0, |domain| {
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

fn selector_matches_node(selector: &InspectionSelector, node: &UiNode) -> bool {
    selector.id.as_ref().is_none_or(|id| id == &node.id)
        && selector.role.is_none_or(|role| role == node.role)
        && selector.name.as_ref().is_none_or(|name| name == &node.name)
        && selector
            .capability
            .as_ref()
            .is_none_or(|capability| node.actions.iter().any(|action| &action.0 == capability))
        && selector.pane.is_none_or(|pane| Some(pane) == node.pane)
        && selector
            .domain
            .as_ref()
            .is_none_or(|domain| Some(domain) == node.domain_reference.as_ref())
}

fn selector_matches_binding(
    selector: &InspectionSelector,
    binding: &CapabilityDescription,
    node: Option<&UiNode>,
) -> bool {
    selector.id.as_ref().is_none_or(|id| id == &binding.id)
        && selector
            .capability
            .as_ref()
            .is_none_or(|capability| capability == &binding.capability)
        && selector.pane.is_none_or(|pane| Some(pane) == binding.pane)
        && selector
            .domain
            .as_ref()
            .is_none_or(|domain| Some(domain) == binding.domain.as_ref())
        && selector
            .role
            .is_none_or(|role| node.is_some_and(|node| node.role == role))
        && selector
            .name
            .as_ref()
            .is_none_or(|name| node.is_some_and(|node| &node.name == name))
}

fn unique_binding<'a>(
    bindings: &[&'a CapabilityDescription],
) -> Result<&'a CapabilityDescription, InspectionError> {
    match bindings {
        [binding] => Ok(binding),
        [] => Err(InspectionError::new(
            InspectionErrorCode::NoMatch,
            "selector resolves no capability binding",
        )),
        _ => Err(InspectionError::new(
            InspectionErrorCode::Ambiguous,
            "selector resolves multiple capability bindings",
        )),
    }
}

fn validate_expected(
    current: &ResolvedTarget,
    expected: &ResolvedTarget,
) -> Result<(), InspectionError> {
    if expected.observation == 0
        || expected.observation > current.observation
        || current.id != expected.id
        || current.capability != expected.capability
        || current.pane != expected.pane
        || current.domain != expected.domain
        || current.meaning != expected.meaning
    {
        Err(InspectionError::new(
            InspectionErrorCode::StaleTarget,
            "target identity or application-owned meaning changed",
        ))
    } else {
        Ok(())
    }
}

fn validate_invocable(capability: &CapabilityDescription) -> Result<(), InspectionError> {
    if !capability.semantic_invocable {
        return Err(InspectionError::new(
            InspectionErrorCode::NotInvocable,
            "capability is discoverable but requires physical interaction",
        ));
    }
    if !capability.availability.enabled() {
        return Err(InspectionError::new(
            InspectionErrorCode::Unavailable,
            capability
                .availability
                .disabled_reason()
                .unwrap_or("capability is hidden"),
        ));
    }
    Ok(())
}

fn validate_arguments(arguments: &serde_json::Value) -> Result<(), InspectionError> {
    if arguments.is_null() || arguments.as_object().is_some_and(serde_json::Map::is_empty) {
        Ok(())
    } else {
        Err(InspectionError::new(
            InspectionErrorCode::InvalidArguments,
            "this capability accepts no arguments",
        ))
    }
}

fn page_bounds(
    observation: u64,
    total: usize,
    limit: usize,
    cursor: Option<InspectionCursor>,
) -> Result<(usize, usize, Option<InspectionCursor>), InspectionError> {
    if !(1..=INSPECTION_QUERY_LIMIT).contains(&limit) {
        return Err(InspectionError::new(
            InspectionErrorCode::InvalidRequest,
            "query limit must be 1..256",
        ));
    }
    let start = match cursor {
        Some(cursor) if cursor.observation != observation || cursor.offset > total => {
            return Err(InspectionError::new(
                InspectionErrorCode::StaleCursor,
                "cursor does not identify the current bounded observation",
            ));
        }
        Some(cursor) => cursor.offset,
        None => 0,
    };
    let end = start.saturating_add(limit).min(total);
    let next = (end < total).then_some(InspectionCursor {
        observation,
        offset: end,
    });
    Ok((start, end, next))
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
    validate_selector(selector)?;
    let snapshot = &observation.snapshot;
    let mut matches = snapshot
        .nodes
        .iter()
        .filter(|node| selector_matches_node(selector, node));
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
mod tests;
