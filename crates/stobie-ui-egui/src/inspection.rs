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

use stobie_core::PaneId;

use crate::{
    ActionKey, ActionScope, ActionTarget, Availability, DomainReference, SemanticUiId, UiNode,
    UiRole, UiSnapshot,
};

pub use stobie_protocol::inspection::*;

#[cfg(all(unix, not(target_arch = "wasm32")))]
mod native;
#[cfg(all(unix, not(target_arch = "wasm32")))]
pub use native::NativeInspectionHost;

/// Native socket hosting is unavailable on non-Unix hosts. The opt-in start
/// method reports that limitation while ordinary application builds remain portable.
#[cfg(all(not(unix), not(target_arch = "wasm32")))]
pub struct NativeInspectionHost;

#[derive(Clone, Debug)]
pub struct InspectionBinding<A: ActionKey> {
    pub target: ActionTarget<A>,
    pub id: SemanticUiId,
    pub domain: Option<DomainReference>,
    pub availability: Availability,
    pub semantic_invocable: bool,
    pub meaning: String,
    /// The argument this binding's control carries, such as its row's ID;
    /// [`ActionArgument::None`] for a binding that takes any value its
    /// contract allows.
    pub argument: ActionArgument,
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
            argument: ActionArgument::None,
        }
    }

    /// Bind the argument this binding's control carries.
    pub fn with_argument(mut self, argument: ActionArgument) -> Self {
        self.argument = argument;
        self
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
        let argument = self.argument.to_value();
        CapabilityDescription {
            argument: argument.clone(),
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
            arguments: self.target.action.argument(),
            target: ResolvedTarget {
                argument,
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

/// A rendered framework text field that set text can edit: its semantic
/// identity and the egui widget that edits it. Hosts bind every such field,
/// without application code.
#[derive(Clone, Debug)]
pub struct TextFieldBinding {
    pub id: SemanticUiId,
    pub widget: egui::Id,
    pub pane: Option<PaneId>,
    pub domain: Option<DomainReference>,
    pub availability: Availability,
}

impl TextFieldBinding {
    fn describe(&self, observation: u64) -> CapabilityDescription {
        let target = ResolvedTarget {
            argument: None,
            id: self.id.clone(),
            capability: SET_TEXT_CAPABILITY.into(),
            pane: self.pane,
            domain: self.domain.clone(),
            meaning: "text-field".into(),
            observation,
        };
        CapabilityDescription {
            argument: None,
            id: self.id.clone(),
            capability: SET_TEXT_CAPABILITY.into(),
            pane: self.pane,
            domain: self.domain.clone(),
            label: "Set text".into(),
            description: "Replace the field's text through the same edit route as typing".into(),
            scope: if self.pane.is_some() {
                ActionScope::Pane
            } else {
                ActionScope::Application
            },
            shortcut: None,
            availability: self.availability.clone(),
            rendered: false,
            control_enabled: None,
            semantic_invocable: true,
            arguments: ArgumentContract::Text { max_length: None },
            target,
        }
    }
}

/// A presented link that agents can open: its semantic identity and URL.
/// Hosts bind every enabled or disabled [`UiRole::Link`] node, without
/// application code. The URL is the target's meaning, so a link whose URL
/// changed after discovery is stale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkBinding {
    pub id: SemanticUiId,
    pub url: String,
    pub pane: Option<PaneId>,
    pub domain: Option<DomainReference>,
    pub availability: Availability,
}

impl LinkBinding {
    /// The binding for a presented link node, or `None` for any other node or
    /// a link without a URL value. A disabled link, including one whose URL
    /// the link recipe rejected, binds as unavailable with its reason.
    pub fn from_node(node: &UiNode) -> Option<Self> {
        let url = node.value.as_ref().filter(|_| node.role == UiRole::Link)?;
        Some(Self {
            id: node.id.clone(),
            url: url.clone(),
            pane: node.pane,
            domain: node.domain_reference.clone(),
            availability: if node.enabled {
                Availability::Enabled
            } else {
                Availability::Disabled {
                    reason: node
                        .disabled_reason
                        .clone()
                        .unwrap_or_else(|| "The link is unavailable".into())
                        .into(),
                }
            },
        })
    }

    fn describe(&self, observation: u64) -> CapabilityDescription {
        let target = ResolvedTarget {
            argument: None,
            id: self.id.clone(),
            capability: OPEN_LINK_CAPABILITY.into(),
            pane: self.pane,
            domain: self.domain.clone(),
            meaning: self.url.clone(),
            observation,
        };
        CapabilityDescription {
            argument: None,
            id: self.id.clone(),
            capability: OPEN_LINK_CAPABILITY.into(),
            pane: self.pane,
            domain: self.domain.clone(),
            label: "Open link".into(),
            description: "Open the link's URL in a new browser tab, or the platform's default browser natively".into(),
            scope: if self.pane.is_some() {
                ActionScope::Pane
            } else {
                ActionScope::Application
            },
            shortcut: None,
            availability: self.availability.clone(),
            rendered: false,
            control_enabled: None,
            semantic_invocable: true,
            arguments: ArgumentContract::None,
            target,
        }
    }
}

/// A presented copy control that agents can press: its semantic identity,
/// the egui widget that copies and the revision of its text. Hosts bind every
/// presented copy control, without application code. The text is never
/// published; its revision is the target's meaning, so a control whose text
/// changed after discovery is stale.
#[derive(Clone, Debug, PartialEq)]
pub struct CopyBinding {
    pub id: SemanticUiId,
    pub widget: egui::Id,
    pub revision: u64,
    pub pane: Option<PaneId>,
    pub domain: Option<DomainReference>,
    pub availability: Availability,
}

impl CopyBinding {
    fn describe(&self, observation: u64) -> CapabilityDescription {
        let target = ResolvedTarget {
            argument: None,
            id: self.id.clone(),
            capability: COPY_TEXT_CAPABILITY.into(),
            pane: self.pane,
            domain: self.domain.clone(),
            meaning: format!("text-revision:{}", self.revision),
            observation,
        };
        CapabilityDescription {
            argument: None,
            id: self.id.clone(),
            capability: COPY_TEXT_CAPABILITY.into(),
            pane: self.pane,
            domain: self.domain.clone(),
            label: "Copy text".into(),
            description:
                "Copy the control's text to the clipboard through the same route as pressing it"
                    .into(),
            scope: if self.pane.is_some() {
                ActionScope::Pane
            } else {
                ActionScope::Application
            },
            shortcut: None,
            availability: self.availability.clone(),
            rendered: false,
            control_enabled: None,
            semantic_invocable: true,
            arguments: ArgumentContract::None,
            target,
        }
    }
}

/// The application's reveal capability, which scrolls the node a semantic ID
/// names into view. A host offers it with [`Inspection::offer_reveal`].
fn reveal_description(observation: u64) -> CapabilityDescription {
    let id = SemanticUiId::new(REVEAL_CAPABILITY);
    CapabilityDescription {
        argument: None,
        id: id.clone(),
        capability: REVEAL_CAPABILITY.into(),
        pane: None,
        domain: None,
        label: "Reveal".into(),
        description:
            "Scroll the node with this semantic ID into view, so it is observed and can be acted on"
                .into(),
        scope: ActionScope::Application,
        shortcut: None,
        availability: Availability::Enabled,
        rendered: false,
        control_enabled: None,
        semantic_invocable: true,
        arguments: ArgumentContract::Text {
            max_length: Some(REVEAL_ID_LENGTH as u32),
        },
        target: ResolvedTarget {
            argument: None,
            id,
            capability: REVEAL_CAPABILITY.into(),
            pane: None,
            domain: None,
            meaning: "reveal".into(),
            observation,
        },
    }
}

/// The longest semantic ID a reveal takes, in characters.
const REVEAL_ID_LENGTH: usize = 512;

/// The framework's own bindings for one pass: text fields, links and copy
/// controls, which hosts bind without application code.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameworkBindings<'a> {
    pub text_fields: &'a [TextFieldBinding],
    pub links: &'a [LinkBinding],
    pub copies: &'a [CopyBinding],
}

/// One admitted invocation, executed on the application thread.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Invocation<A: ActionKey> {
    /// Perform an application action with its validated argument.
    Action {
        target: ActionTarget<A>,
        argument: ActionArgument,
    },
    /// Replace a framework text field's text. The edit applies on a later UI
    /// pass, so the host defers its completion with
    /// [`Execution::Deferred`] and reports it with [`Inspection::complete`].
    SetText {
        request_id: String,
        field: TextFieldBinding,
        text: String,
    },
    /// Open a presented link's URL. A semantic open always asks for a new
    /// browser tab, so the application under inspection stays open.
    OpenLink { link: LinkBinding },
    /// Copy a copy control's text, as pressing it would. The control copies
    /// on its next pass and shows its confirmation or failure, which the
    /// receipt does not wait for.
    CopyText { copy: CopyBinding },
    /// Scroll the node `id` names into view, for a host that offers reveal
    /// with [`Inspection::offer_reveal`]. The scroll applies on later passes,
    /// so the host defers its completion with [`Execution::Deferred`] and
    /// reports it with [`Inspection::complete`] once a pass presents the node
    /// in view, or rejects it.
    Reveal {
        request_id: String,
        id: SemanticUiId,
    },
}

/// How far an executed invocation got.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Execution {
    /// The effect is in place; the receipt completes now.
    Completed,
    /// The effect applies later. The receipt stays queued, and cannot be
    /// cancelled, until the host calls [`Inspection::complete`].
    Deferred,
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
    renderer: Option<RendererIdentity>,
    instance: String,
    publication: Option<Publication>,
    observation: u64,
    ledger: BTreeMap<String, LedgerEntry>,
    pending: VecDeque<String>,
    waker: Option<Arc<dyn Fn() + Send + Sync>>,
    staged: Option<StagedPublication>,
    /// Whether the host scrolls nodes into view on request.
    reveal: bool,
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
                renderer: None,
                instance,
                publication: None,
                observation: 0,
                ledger: BTreeMap::new(),
                pending: VecDeque::new(),
                waker: None,
                staged: None,
                reveal: false,
            })),
        })
    }

    /// Report the renderer's backend and adapter in `hello`.
    pub fn set_renderer(&self, renderer: Option<RendererIdentity>) {
        self.state
            .lock()
            .expect("inspection mutex poisoned")
            .renderer = renderer;
    }

    pub fn instance(&self) -> String {
        self.state
            .lock()
            .expect("inspection mutex poisoned")
            .instance
            .clone()
    }

    /// Publish the application's [`REVEAL_CAPABILITY`] from now on, for a
    /// host that executes [`Invocation::Reveal`]. [`crate::AppHost`] offers
    /// it when inspection is enabled.
    pub fn offer_reveal(&self) {
        self.state.lock().expect("inspection mutex poisoned").reveal = true;
    }

    /// Install once before the first app update, after other UI plugins. Later
    /// plugins must not request discard after this completion hook has run.
    /// Reads do not install hooks or request a repaint.
    pub fn install_completion_hook(&self, context: &egui::Context) {
        let key = egui::Id::new(("stobie.inspection.completion", self.instance()));
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
                "stobie.inspection.completed",
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
        self.stage_completed_with_fields(context, snapshot, bindings, &[], metadata, facts)
    }

    /// [`Self::stage_completed`] with the pass's framework text fields, which
    /// discovery publishes as `stobie.set-text` capabilities.
    pub fn stage_completed_with_fields<A: ActionKey>(
        &self,
        context: &egui::Context,
        snapshot: UiSnapshot,
        bindings: &[InspectionBinding<A>],
        text_fields: &[TextFieldBinding],
        metadata: CollectionMetadata,
        facts: BTreeMap<String, ObservationValue>,
    ) -> Result<(), InspectionError> {
        self.stage_completed_with_links(
            context,
            snapshot,
            bindings,
            text_fields,
            &[],
            metadata,
            facts,
        )
    }

    /// [`Self::stage_completed_with_fields`] with the pass's presented links,
    /// which discovery publishes as `stobie.open-link` capabilities.
    #[allow(clippy::too_many_arguments)]
    pub fn stage_completed_with_links<A: ActionKey>(
        &self,
        context: &egui::Context,
        snapshot: UiSnapshot,
        bindings: &[InspectionBinding<A>],
        text_fields: &[TextFieldBinding],
        links: &[LinkBinding],
        metadata: CollectionMetadata,
        facts: BTreeMap<String, ObservationValue>,
    ) -> Result<(), InspectionError> {
        self.stage_completed_with_framework(
            context,
            snapshot,
            bindings,
            FrameworkBindings {
                text_fields,
                links,
                copies: &[],
            },
            metadata,
            facts,
        )
    }

    /// [`Self::stage_completed`] with the pass's framework bindings, which
    /// discovery publishes as `stobie.set-text`, `stobie.open-link` and
    /// `stobie.copy-text` capabilities.
    pub fn stage_completed_with_framework<A: ActionKey>(
        &self,
        context: &egui::Context,
        snapshot: UiSnapshot,
        bindings: &[InspectionBinding<A>],
        framework: FrameworkBindings<'_>,
        metadata: CollectionMetadata,
        facts: BTreeMap<String, ObservationValue>,
    ) -> Result<(), InspectionError> {
        let reveal = self.state.lock().expect("inspection mutex poisoned").reveal;
        let publication =
            prepare_publication(snapshot, bindings, framework, reveal, metadata, facts)?;
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
        let reveal = self.state.lock().expect("inspection mutex poisoned").reveal;
        let publication = prepare_publication(
            snapshot,
            bindings,
            FrameworkBindings::default(),
            reveal,
            metadata,
            facts,
        )?;
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

    /// Execute at most one queued no-argument action. Recalculate bindings
    /// from current application state before every call, including between
    /// queued actions. Success proves validated dispatch only, never a later
    /// observed postcondition. Hosts with typed arguments or text fields use
    /// [`Self::drain_invocation`].
    pub fn drain<A: ActionKey>(
        &self,
        current: &[InspectionBinding<A>],
        mut execute: impl FnMut(ActionTarget<A>) -> Result<(), InspectionError>,
    ) -> bool {
        self.drain_invocation(current, &[], |invocation| match invocation {
            Invocation::Action {
                target,
                argument: ActionArgument::None,
            } => execute(target).map(|()| Execution::Completed),
            _ => Err(InspectionError::validation(
                "this host executes only no-argument actions",
            )),
        })
    }

    /// Execute at most one queued invocation against the current action
    /// bindings and text fields, with its argument revalidated against the
    /// current contract.
    pub fn drain_invocation<A: ActionKey>(
        &self,
        current: &[InspectionBinding<A>],
        text_fields: &[TextFieldBinding],
        execute: impl FnMut(Invocation<A>) -> Result<Execution, InspectionError>,
    ) -> bool {
        self.drain_invocation_with_links(current, text_fields, &[], execute)
    }

    /// [`Self::drain_invocation`] with the current presented links.
    pub fn drain_invocation_with_links<A: ActionKey>(
        &self,
        current: &[InspectionBinding<A>],
        text_fields: &[TextFieldBinding],
        links: &[LinkBinding],
        execute: impl FnMut(Invocation<A>) -> Result<Execution, InspectionError>,
    ) -> bool {
        self.drain_invocation_with_framework(
            current,
            FrameworkBindings {
                text_fields,
                links,
                copies: &[],
            },
            execute,
        )
    }

    /// [`Self::drain_invocation`] with the current framework bindings.
    pub fn drain_invocation_with_framework<A: ActionKey>(
        &self,
        current: &[InspectionBinding<A>],
        framework: FrameworkBindings<'_>,
        mut execute: impl FnMut(Invocation<A>) -> Result<Execution, InspectionError>,
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
            selector,
            expected,
            arguments,
        } = operation
        else {
            unreachable!("only invocation is queued")
        };
        let validation = state.resolve_current(
            current,
            framework,
            &request_id,
            &selector,
            &expected,
            &arguments,
        );
        state
            .ledger
            .get_mut(&request_id)
            .expect("queued ledger entry")
            .executing = true;
        drop(state);
        let outcome = validation.and_then(&mut execute);
        if outcome != Ok(Execution::Deferred) {
            self.complete(&request_id, outcome.map(drop));
        }
        true
    }

    /// Finish an invocation whose execution was [`Execution::Deferred`] (or
    /// any executing invocation): record its outcome and, on success, the
    /// first observation that will present its effect. Call it once the
    /// effect is in the application's state; an observation staged before
    /// then does not present it.
    pub fn complete(&self, request_id: &str, outcome: Result<(), InspectionError>) {
        let mut state = self.state.lock().expect("inspection mutex poisoned");
        // A staged pass commits before any later one, without the effect.
        let next = state.observation + 1 + u64::from(state.staged.is_some());
        let Some(entry) = state.ledger.get_mut(request_id) else {
            return;
        };
        if !entry.executing {
            return;
        }
        entry.executing = false;
        entry.receipt.state = if outcome.is_ok() {
            ReceiptState::Completed
        } else {
            ReceiptState::Rejected
        };
        entry.receipt.after_observation = outcome.is_ok().then_some(next);
        entry.receipt.error = outcome.err();
    }

    /// Convenience for a browser host already running on the app thread. This
    /// drains one request only; a host with other pending work must recalculate
    /// bindings before continuing to drain.
    pub fn dispatch<A: ActionKey>(
        &self,
        request: InspectionRequest,
        current: &[InspectionBinding<A>],
        mut execute: impl FnMut(ActionTarget<A>) -> Result<(), InspectionError>,
    ) -> InspectionReply {
        self.dispatch_invocation(request, current, &[], |invocation| match invocation {
            Invocation::Action {
                target,
                argument: ActionArgument::None,
            } => execute(target).map(|()| Execution::Completed),
            _ => Err(InspectionError::validation(
                "this host executes only no-argument actions",
            )),
        })
    }

    /// [`Self::dispatch`] for hosts with typed arguments and text fields.
    pub fn dispatch_invocation<A: ActionKey>(
        &self,
        request: InspectionRequest,
        current: &[InspectionBinding<A>],
        text_fields: &[TextFieldBinding],
        execute: impl FnMut(Invocation<A>) -> Result<Execution, InspectionError>,
    ) -> InspectionReply {
        self.dispatch_invocation_with_links(request, current, text_fields, &[], execute)
    }

    /// [`Self::dispatch_invocation`] with the current presented links.
    pub fn dispatch_invocation_with_links<A: ActionKey>(
        &self,
        request: InspectionRequest,
        current: &[InspectionBinding<A>],
        text_fields: &[TextFieldBinding],
        links: &[LinkBinding],
        execute: impl FnMut(Invocation<A>) -> Result<Execution, InspectionError>,
    ) -> InspectionReply {
        self.dispatch_invocation_with_framework(
            request,
            current,
            FrameworkBindings {
                text_fields,
                links,
                copies: &[],
            },
            execute,
        )
    }

    /// [`Self::dispatch_invocation`] with the current framework bindings.
    pub fn dispatch_invocation_with_framework<A: ActionKey>(
        &self,
        request: InspectionRequest,
        current: &[InspectionBinding<A>],
        framework: FrameworkBindings<'_>,
        execute: impl FnMut(Invocation<A>) -> Result<Execution, InspectionError>,
    ) -> InspectionReply {
        let invocation = matches!(request.operation, InspectionOperation::Invoke { .. });
        let request_id = request.request_id.clone();
        let reply = self.handle(request);
        let queued = matches!(&reply.result, Some(InspectionResult::Receipt { receipt })
            if receipt.state == ReceiptState::Queued);
        if invocation && queued {
            self.drain_invocation_with_framework(current, framework, execute);
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
        let Some(path) = std::env::var_os("STOBIE_AUTOMATION_SOCKET") else {
            return Ok(None);
        };
        NativeInspectionHost::start(self.clone(), path.into(), Arc::new(wake)).map(Some)
    }

    #[cfg(all(not(unix), not(target_arch = "wasm32")))]
    pub fn start_native(
        &self,
        _wake: impl Fn() + Send + Sync + 'static,
    ) -> std::io::Result<Option<NativeInspectionHost>> {
        if std::env::var_os("STOBIE_AUTOMATION_SOCKET").is_none() {
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
                        after_observation: None,
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
                renderer: self.renderer.clone(),
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
                selector.validate()?;
                let publication = self.publication()?;
                let nodes: Vec<_> = publication
                    .observation
                    .snapshot
                    .nodes
                    .iter()
                    .filter(|node| selector.matches_node(node))
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
                selector.validate()?;
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
                let validation = selector
                    .validate()
                    .and_then(|()| self.resolve_published(selector, expected, arguments));
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
                    after_observation: None,
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
                        after_observation: None,
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
        arguments: &serde_json::Value,
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
        let capability = select_invocation_target(&candidates, |candidate| *candidate, arguments)?;
        validate_expected(&capability.target, expected)?;
        validate_invocable(capability)?;
        capability.parse_arguments(arguments).map(drop)
    }

    fn resolve_current<A: ActionKey>(
        &self,
        bindings: &[InspectionBinding<A>],
        framework: FrameworkBindings<'_>,
        request_id: &str,
        selector: &InspectionSelector,
        expected: &ResolvedTarget,
        arguments: &serde_json::Value,
    ) -> Result<Invocation<A>, InspectionError> {
        let publication = self.publication()?;
        let observation = publication.observation.id;
        let snapshot = &publication.observation.snapshot;
        let actions = bindings
            .iter()
            .map(|binding| (binding.describe(observation), Current::Action(binding)));
        let fields = framework
            .text_fields
            .iter()
            .map(|field| (field.describe(observation), Current::Field(field)));
        let links = framework
            .links
            .iter()
            .map(|link| (link.describe(observation), Current::Link(link)));
        let copies = framework
            .copies
            .iter()
            .map(|copy| (copy.describe(observation), Current::Copy(copy)));
        let reveal = self
            .reveal
            .then(|| (reveal_description(observation), Current::Reveal));
        let candidates: Vec<_> = actions
            .chain(fields)
            .chain(links)
            .chain(copies)
            .chain(reveal)
            .filter(|(description, ..)| {
                selector_matches_binding(selector, description, snapshot.node(&description.id))
            })
            .collect();
        // A target that is no longer presented, such as a removed row's
        // control, is stale rather than unknown: it was admitted.
        let (description, current) =
            select_invocation_target(&candidates, |(description, _)| description, arguments)
                .map_err(|error| match error.code {
                    InspectionErrorCode::NoMatch => InspectionError::new(
                        InspectionErrorCode::StaleTarget,
                        "target disappeared before execution",
                    ),
                    InspectionErrorCode::Ambiguous => InspectionError::new(
                        InspectionErrorCode::Ambiguous,
                        "current selector resolves multiple bindings",
                    ),
                    _ => error,
                })?;
        validate_expected(&description.target, expected)?;
        validate_invocable(description)?;
        let argument = description.parse_arguments(arguments)?;
        Ok(match current {
            Current::Action(binding) => Invocation::Action {
                target: binding.target,
                argument,
            },
            Current::Field(field) => Invocation::SetText {
                request_id: request_id.to_owned(),
                field: (*field).clone(),
                text: match argument {
                    ActionArgument::Text(text) => text,
                    _ => unreachable!("set text takes a text argument"),
                },
            },
            Current::Link(link) => Invocation::OpenLink {
                link: (*link).clone(),
            },
            Current::Copy(copy) => Invocation::CopyText {
                copy: (*copy).clone(),
            },
            Current::Reveal => match argument {
                ActionArgument::Text(id) if !id.is_empty() => Invocation::Reveal {
                    request_id: request_id.to_owned(),
                    id: SemanticUiId::new(id),
                },
                _ => {
                    return Err(InspectionError::validation(
                        "reveal takes the semantic ID of the node to reveal",
                    ));
                }
            },
        })
    }
}

/// The current binding a candidate capability resolves to.
enum Current<'a, A: ActionKey> {
    Action(&'a InspectionBinding<A>),
    Field(&'a TextFieldBinding),
    Link(&'a LinkBinding),
    Copy(&'a CopyBinding),
    Reveal,
}

fn prepare_publication<A: ActionKey>(
    snapshot: UiSnapshot,
    bindings: &[InspectionBinding<A>],
    framework: FrameworkBindings<'_>,
    reveal: bool,
    metadata: CollectionMetadata,
    facts: BTreeMap<String, ObservationValue>,
) -> Result<Publication, InspectionError> {
    use InspectionErrorCode as Code;
    if snapshot.nodes.len() > INSPECTION_NODE_LIMIT
        || snapshot.text.len() > INSPECTION_TEXT_LIMIT
        || bindings.len()
            + framework.text_fields.len()
            + framework.links.len()
            + framework.copies.len()
            + usize::from(reveal)
            > INSPECTION_NODE_LIMIT
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
    for field in framework.text_fields {
        let presented = snapshot.node(&field.id).is_some_and(|node| {
            matches!(node.role, UiRole::TextInput | UiRole::MultilineTextInput)
        });
        if !presented || !targets.insert((&field.id, SET_TEXT_CAPABILITY)) {
            return Err(InspectionError::new(
                Code::InvalidRequest,
                "text fields require a unique, presented text input node",
            ));
        }
    }
    for link in framework.links {
        let presented = snapshot
            .node(&link.id)
            .is_some_and(|node| node.role == UiRole::Link);
        // A link with a rejected or blank URL still publishes, as an
        // unavailable binding: the host refuses to open it.
        if !presented || !targets.insert((&link.id, OPEN_LINK_CAPABILITY)) {
            return Err(InspectionError::new(
                Code::InvalidRequest,
                "links require a unique, presented link node",
            ));
        }
    }
    for copy in framework.copies {
        let presented = snapshot
            .node(&copy.id)
            .is_some_and(|node| node.role == UiRole::Button);
        if !presented || !targets.insert((&copy.id, COPY_TEXT_CAPABILITY)) {
            return Err(InspectionError::new(
                Code::InvalidRequest,
                "copy controls require a unique, presented button node",
            ));
        }
    }
    let capabilities = bindings
        .iter()
        .map(|binding| binding.describe(0))
        .chain(framework.text_fields.iter().map(|field| field.describe(0)))
        .chain(framework.links.iter().map(|link| link.describe(0)))
        .chain(framework.copies.iter().map(|copy| copy.describe(0)))
        .chain(reveal.then(|| reveal_description(0)))
        .collect();
    let publication = Publication {
        observation: CompletedObservation {
            instance: String::new(),
            application: ApplicationIdentity::default(),
            id: 0,
            snapshot,
            collection: metadata,
            facts,
        },
        capabilities,
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
        && selector.argument.as_ref().is_none_or(|argument| {
            binding
                .argument
                .as_ref()
                .is_some_and(|carried| same_argument(argument, carried))
        })
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
        || !match (&current.argument, &expected.argument) {
            (Some(current), Some(expected)) => same_argument(current, expected),
            (None, None) => true,
            _ => false,
        }
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

#[cfg(test)]
mod tests;
