//! The application interface client: requests, invocation with receipts,
//! set text and waits.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use serde::Serialize;
use stobie_protocol::{
    ApplicationIdentity, CapabilityDescription, CompletedObservation, INSPECTION_VERSION,
    InspectionCursor, InspectionErrorCode, InspectionLimits, InspectionOperation,
    InspectionOperationKind, InspectionReply, InspectionRequest, InspectionResult,
    InspectionSelector, InvocationReceipt, ObservedNode, REVEAL_CAPABILITY, ReceiptState,
    RendererIdentity, ResolvedTarget, SET_TEXT_CAPABILITY, SemanticUiId, UiNode,
};

use crate::condition::{one_node, selected_ids};
use crate::{ClientError, Condition, Transport};

/// How often waits and receipt reads poll.
pub const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// The default deadline for one client operation.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

/// What `hello` returns: the application and what it supports.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Hello {
    pub instance: String,
    pub application: ApplicationIdentity,
    pub limits: InspectionLimits,
    pub operations: Vec<InspectionOperationKind>,
    /// The GPU backend and adapter, when the application reports them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub renderer: Option<RendererIdentity>,
}

/// One page of a query.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct QueryPage {
    pub observation: u64,
    pub total: usize,
    pub nodes: Vec<ObservedNode>,
    pub next_cursor: Option<InspectionCursor>,
}

/// One page of discovery.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DiscoverPage {
    pub observation: u64,
    pub total: usize,
    pub capabilities: Vec<CapabilityDescription>,
    pub next_cursor: Option<InspectionCursor>,
}

/// A page request: at most `limit` entries, continuing from `cursor`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Page {
    pub limit: usize,
    pub cursor: Option<InspectionCursor>,
}

impl Default for Page {
    fn default() -> Self {
        Self {
            limit: 64,
            cursor: None,
        }
    }
}

/// How to invoke a capability.
#[derive(Clone, Debug, Default)]
pub struct InvokeOptions {
    /// A previously discovered target; otherwise the selector must discover
    /// exactly one.
    pub expected: Option<ResolvedTarget>,
    /// The `arguments` object, such as `{"value": "ideas"}`.
    pub arguments: Option<serde_json::Value>,
    /// The mutation's ID; a retry with the same ID and body never runs twice.
    pub request_id: Option<String>,
    /// Return once the receipt is admitted, without waiting for completion.
    pub no_wait: bool,
}

/// The outcome of an invocation: its receipt and, when it completed, the
/// first observation that presents its effect.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Invoked {
    pub receipt: InvocationReceipt,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observation: Option<CompletedObservation>,
}

/// What a satisfied wait observed.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Waited {
    pub observation: CompletedObservation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<UiNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability: Option<CapabilityDescription>,
}

/// A client bound to one application instance through one transport.
pub struct Client<T: Transport> {
    transport: T,
    instance: Option<String>,
    application: Option<ApplicationIdentity>,
    timeout: Duration,
    sequence: u64,
}

impl<T: Transport> Client<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            instance: None,
            application: None,
            timeout: DEFAULT_TIMEOUT,
            sequence: 0,
        }
    }

    /// The deadline for each operation.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    pub fn instance(&self) -> Option<&str> {
        self.instance.as_deref()
    }

    pub fn application(&self) -> Option<&ApplicationIdentity> {
        self.application.as_ref()
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    fn deadline(&self) -> Instant {
        Instant::now() + self.timeout
    }

    fn request_id(&mut self) -> String {
        self.sequence += 1;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        format!(
            "stobie-{:x}-{nanos:x}-{}",
            std::process::id(),
            self.sequence
        )
    }

    /// Send one operation and check the reply's version, request ID, instance
    /// and result kind. An application error comes back as its protocol code.
    fn request(
        &mut self,
        operation: InspectionOperation,
        request_id: String,
        deadline: Instant,
    ) -> Result<InspectionResult, ClientError> {
        let hello = matches!(operation, InspectionOperation::Hello);
        if !hello && self.instance.is_none() {
            return Err(ClientError::usage(
                "hello before addressing the application",
            ));
        }
        let kind = operation.kind();
        let request = InspectionRequest {
            version: INSPECTION_VERSION,
            request_id: request_id.clone(),
            instance: self.instance.clone(),
            operation,
        };
        let body = serde_json::to_string(&request).expect("requests serialise");
        let reply = self.transport.request(&body, deadline)?;
        let reply: InspectionReply = serde_json::from_str(&reply).map_err(|error| {
            ClientError::new(
                "protocol_violation",
                format!("the reply is not a version 1 envelope: {error}"),
                "check that this is a Stobie application",
            )
        })?;
        if let Some(error) = reply.error {
            return Err(ClientError::protocol(error));
        }
        if reply.version != INSPECTION_VERSION || reply.request_id != request_id {
            return Err(ClientError::new(
                "protocol_violation",
                "the reply does not answer this request",
                "check that this is a Stobie application",
            ));
        }
        if !hello && Some(reply.instance.as_str()) != self.instance.as_deref() {
            return Err(ClientError::new(
                "wrong_instance",
                "the application instance changed",
                "the application restarted; attach again",
            ));
        }
        let result = reply.result.ok_or_else(|| {
            ClientError::new(
                "protocol_violation",
                "the reply has neither a result nor an error",
                "report this application",
            )
        })?;
        let expected = match kind {
            InspectionOperationKind::Invoke
            | InspectionOperationKind::Receipt
            | InspectionOperationKind::Cancel => matches!(result, InspectionResult::Receipt { .. }),
            InspectionOperationKind::Hello => matches!(result, InspectionResult::Hello { .. }),
            InspectionOperationKind::Observe => matches!(result, InspectionResult::Observe { .. }),
            InspectionOperationKind::Query => matches!(result, InspectionResult::Query { .. }),
            InspectionOperationKind::Discover => {
                matches!(result, InspectionResult::Discover { .. })
            }
        };
        if !expected {
            return Err(ClientError::new(
                "protocol_violation",
                "the reply's result does not match the operation",
                "report this application",
            ));
        }
        if hello {
            self.instance = Some(reply.instance);
        }
        Ok(result)
    }

    fn read(&mut self, operation: InspectionOperation) -> Result<InspectionResult, ClientError> {
        let deadline = self.deadline();
        self.read_by(operation, deadline)
    }

    /// Send a read, waiting out `missing_observation` until `deadline`: an
    /// application that has just started publishes its first observation
    /// after its first pass.
    fn read_by(
        &mut self,
        operation: InspectionOperation,
        deadline: Instant,
    ) -> Result<InspectionResult, ClientError> {
        loop {
            let id = self.request_id();
            match self.request(operation.clone(), id, deadline) {
                Err(error)
                    if error.code == "missing_observation"
                        && Instant::now() + POLL_INTERVAL < deadline =>
                {
                    std::thread::sleep(POLL_INTERVAL);
                }
                result => return result,
            }
        }
    }

    /// Bind to the application and read its identity, limits and operations.
    pub fn hello(&mut self) -> Result<Hello, ClientError> {
        let InspectionResult::Hello {
            application,
            limits,
            operations,
            renderer,
        } = self.read(InspectionOperation::Hello)?
        else {
            unreachable!("checked kind")
        };
        self.application = Some(application.clone());
        Ok(Hello {
            instance: self.instance.clone().expect("hello binds"),
            application,
            limits,
            operations,
            renderer,
        })
    }

    /// The latest completed observation, waiting until the deadline for the
    /// first one when the application has not published yet.
    pub fn observe(&mut self) -> Result<CompletedObservation, ClientError> {
        let deadline = self.deadline();
        self.observe_by(deadline)
    }

    fn observe_by(&mut self, deadline: Instant) -> Result<CompletedObservation, ClientError> {
        let InspectionResult::Observe { observation } =
            self.read_by(InspectionOperation::Observe, deadline)?
        else {
            unreachable!("checked kind")
        };
        Ok(observation)
    }

    pub fn query(
        &mut self,
        selector: InspectionSelector,
        page: Page,
    ) -> Result<QueryPage, ClientError> {
        let InspectionResult::Query {
            observation,
            total,
            nodes,
            next_cursor,
        } = self.read(InspectionOperation::Query {
            selector,
            limit: page.limit,
            cursor: page.cursor,
        })?
        else {
            unreachable!("checked kind")
        };
        Ok(QueryPage {
            observation,
            total,
            nodes,
            next_cursor,
        })
    }

    pub fn discover(
        &mut self,
        selector: InspectionSelector,
        page: Page,
    ) -> Result<DiscoverPage, ClientError> {
        let deadline = self.deadline();
        self.discover_by(selector, page, deadline)
    }

    fn discover_by(
        &mut self,
        selector: InspectionSelector,
        page: Page,
        deadline: Instant,
    ) -> Result<DiscoverPage, ClientError> {
        let InspectionResult::Discover {
            observation,
            total,
            capabilities,
            next_cursor,
        } = self.read_by(
            InspectionOperation::Discover {
                selector,
                limit: page.limit,
                cursor: page.cursor,
            },
            deadline,
        )?
        else {
            unreachable!("checked kind")
        };
        Ok(DiscoverPage {
            observation,
            total,
            capabilities,
            next_cursor,
        })
    }

    /// The one capability a selector discovers.
    pub fn discover_one(
        &mut self,
        selector: InspectionSelector,
    ) -> Result<CapabilityDescription, ClientError> {
        let page = self.discover(
            selector,
            Page {
                limit: 8,
                cursor: None,
            },
        )?;
        match page.capabilities.as_slice() {
            [capability] if page.total == 1 => Ok(capability.clone()),
            [] => Err(ClientError::new(
                "no_match",
                "the selector discovers no capability",
                "run `stobie discover` to list capabilities and their IDs",
            )),
            capabilities => Err(ClientError::new(
                "ambiguous",
                format!(
                    "the selector discovers {} capabilities: {}",
                    page.total,
                    capabilities
                        .iter()
                        .map(|capability| format!(
                            "{} on {}",
                            capability.capability, capability.id.0
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                "add the control's id, or pane or domain, to the selector",
            )),
        }
    }

    /// The one capability an invocation of `selector` addresses, with the
    /// wire `arguments` it sends, which `arguments_for` builds for each
    /// candidate from its contract (`None` for no value). A sent value also
    /// selects, as the application does: a control carrying another value,
    /// such as another row's Open, is not a candidate, and the control
    /// carrying that value is preferred over the action's own binding.
    ///
    /// When nothing matches a selector that names an ID, such as a field
    /// below the fold of a region observed only in view, and the application
    /// offers [`REVEAL_CAPABILITY`], it reveals that ID and discovers again,
    /// all within one deadline; a failed reveal says why in the `no_match`.
    pub fn discover_for_invocation(
        &mut self,
        selector: InspectionSelector,
        arguments_for: impl FnMut(
            &CapabilityDescription,
        ) -> Result<Option<serde_json::Value>, ClientError>,
    ) -> Result<(CapabilityDescription, Option<serde_json::Value>), ClientError> {
        let deadline = self.deadline();
        self.discover_for_invocation_by(selector, arguments_for, deadline)
    }

    fn discover_for_invocation_by(
        &mut self,
        selector: InspectionSelector,
        mut arguments_for: impl FnMut(
            &CapabilityDescription,
        ) -> Result<Option<serde_json::Value>, ClientError>,
        deadline: Instant,
    ) -> Result<(CapabilityDescription, Option<serde_json::Value>), ClientError> {
        match self.discover_candidates(selector.clone(), &mut arguments_for, deadline) {
            Err(error) if error.code == "no_match" => {}
            result => return result,
        }
        let unmatched = |reason: Option<String>| {
            ClientError::new(
                "no_match",
                match reason {
                    Some(reason) => format!(
                        "the selector discovers no capability, and revealing it failed: {reason}"
                    ),
                    None => "the selector discovers no capability".into(),
                },
                "run `stobie discover` to list capabilities and their IDs",
            )
        };
        let Some(id) = selector
            .id
            .clone()
            .filter(|_| selector.capability.as_deref() != Some(REVEAL_CAPABILITY))
        else {
            return Err(unmatched(None));
        };
        let revealed = match self.reveal_by(&id, deadline) {
            Ok(revealed) => revealed,
            // An application without reveal: the selector simply matches
            // nothing.
            Err(error) if error.code == "no_match" => return Err(unmatched(None)),
            Err(error) => return Err(error),
        };
        if revealed.receipt.state != ReceiptState::Completed {
            let reason = revealed.receipt.error.map_or_else(
                || format!("{:?}", revealed.receipt.state),
                |error| error.message,
            );
            return Err(unmatched(Some(reason)));
        }
        self.discover_candidates(selector, &mut arguments_for, deadline)
    }

    fn discover_candidates(
        &mut self,
        selector: InspectionSelector,
        arguments_for: &mut impl FnMut(
            &CapabilityDescription,
        ) -> Result<Option<serde_json::Value>, ClientError>,
        deadline: Instant,
    ) -> Result<(CapabilityDescription, Option<serde_json::Value>), ClientError> {
        let page = self.discover_by(
            selector,
            Page {
                limit: stobie_protocol::INSPECTION_QUERY_LIMIT,
                cursor: None,
            },
            deadline,
        )?;
        if let [capability] = page.capabilities.as_slice()
            && page.total == 1
        {
            let arguments = arguments_for(capability)?;
            return Ok((capability.clone(), arguments));
        }
        if page.capabilities.is_empty() {
            return Err(ClientError::new(
                "no_match",
                "the selector discovers no capability",
                "run `stobie discover` to list capabilities and their IDs",
            ));
        }
        // Each candidate with the arguments it would be sent; a candidate whose
        // contract refuses the value cannot be the target.
        let mut first_error = None;
        let candidates: Vec<(CapabilityDescription, Option<serde_json::Value>)> = page
            .capabilities
            .iter()
            .filter_map(|capability| match arguments_for(capability) {
                Ok(arguments) => Some((capability.clone(), arguments)),
                Err(error) => {
                    first_error.get_or_insert(error);
                    None
                }
            })
            .collect();
        let sent = |arguments: &Option<serde_json::Value>| {
            arguments.clone().unwrap_or(serde_json::Value::Null)
        };
        let chosen = candidates
            .first()
            .map(|(_, arguments)| sent(arguments))
            .and_then(|arguments| {
                stobie_protocol::select_invocation_target(
                    &candidates,
                    |(capability, _)| capability,
                    &arguments,
                )
                .ok()
                // Every candidate must send the same arguments for the choice
                // to be the application's.
                .filter(|_| candidates.iter().all(|(_, other)| sent(other) == arguments))
            });
        match (chosen, first_error) {
            (Some(chosen), _) if page.next_cursor.is_none() => Ok(chosen.clone()),
            (None, Some(error)) if candidates.is_empty() => Err(error),
            _ => Err(ClientError::new(
                "ambiguous",
                format!(
                    "the selector discovers {} capabilities: {}",
                    page.total,
                    page.capabilities
                        .iter()
                        .take(8)
                        .map(|capability| match &capability.argument {
                            Some(argument) => format!(
                                "{} on {} carrying {argument}",
                                capability.capability, capability.id.0
                            ),
                            None => format!("{} on {}", capability.capability, capability.id.0),
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                "add the control's id, pane or domain to the selector, or the value a row's control carries",
            )),
        }
    }

    pub fn receipt(&mut self, request_id: &str) -> Result<InvocationReceipt, ClientError> {
        let InspectionResult::Receipt { receipt } = self.read(InspectionOperation::Receipt {
            request_id: request_id.into(),
        })?
        else {
            unreachable!("checked kind")
        };
        Ok(receipt)
    }

    pub fn cancel(&mut self, request_id: &str) -> Result<InvocationReceipt, ClientError> {
        let InspectionResult::Receipt { receipt } = self.read(InspectionOperation::Cancel {
            request_id: request_id.into(),
        })?
        else {
            unreachable!("checked kind")
        };
        Ok(receipt)
    }

    /// Invoke one capability: discover its target unless one is expected,
    /// check the arguments against its contract, send the mutation once, wait
    /// for the receipt to leave `queued`, then for the observation that
    /// presents a completed effect. Any failure after sending is
    /// `uncertain_completion`: read the receipt instead of repeating it.
    pub fn invoke(
        &mut self,
        selector: InspectionSelector,
        options: InvokeOptions,
    ) -> Result<Invoked, ClientError> {
        let deadline = self.deadline();
        self.invoke_by(selector, options, deadline)
    }

    fn invoke_by(
        &mut self,
        mut selector: InspectionSelector,
        options: InvokeOptions,
        deadline: Instant,
    ) -> Result<Invoked, ClientError> {
        let arguments = options.arguments.unwrap_or(serde_json::Value::Null);
        let expected = match options.expected {
            Some(expected) => expected,
            None => {
                let given = (!arguments.is_null()).then(|| arguments.clone());
                let (capability, _) = self.discover_for_invocation_by(
                    selector.clone(),
                    |_| Ok(given.clone()),
                    deadline,
                )?;
                capability
                    .parse_arguments(&arguments)
                    .map_err(ClientError::protocol)?;
                // Address the binding discovery chose, so that the
                // application resolves the same one.
                selector.id = Some(capability.id.clone());
                capability.target
            }
        };
        let request_id = options.request_id.unwrap_or_else(|| self.request_id());
        let sent = self.request(
            InspectionOperation::Invoke {
                selector,
                expected,
                arguments,
            },
            request_id.clone(),
            deadline,
        );
        let receipt = match sent {
            Ok(InspectionResult::Receipt { receipt }) => receipt,
            Ok(_) => unreachable!("checked kind"),
            // The application answered: nothing ran.
            Err(error) if error.cause.is_some() => return Err(error),
            Err(error) => {
                return Err(ClientError::uncertain(
                    &request_id,
                    format!(
                        "the invocation was sent but its reply was lost: {}",
                        error.message
                    ),
                    Some(error),
                ));
            }
        };
        if options.no_wait {
            return Ok(Invoked {
                receipt,
                observation: None,
            });
        }
        let receipt = self.settle_receipt(receipt, deadline)?;
        let observation = match (receipt.state, receipt.after_observation) {
            (ReceiptState::Completed, Some(after)) => {
                Some(self.observe_from(after, deadline).map_err(|error| {
                    ClientError::uncertain(
                        &request_id,
                        format!("the invocation completed, but observation {after} was not published before the deadline"),
                        Some(error),
                    )
                })?)
            }
            _ => None,
        };
        Ok(Invoked {
            receipt,
            observation,
        })
    }

    /// Replace a framework text field's text, as typing would.
    pub fn set_text(
        &mut self,
        mut selector: InspectionSelector,
        text: &str,
        mut options: InvokeOptions,
    ) -> Result<Invoked, ClientError> {
        selector.capability = Some(SET_TEXT_CAPABILITY.into());
        options.arguments = Some(serde_json::json!({ "value": text }));
        self.invoke(selector, options)
    }

    /// Scroll the node `id` into view through the application's
    /// [`REVEAL_CAPABILITY`], so that a region observed only in view presents
    /// it and its controls can be discovered. The receipt fails, saying why,
    /// when no pass presents the node, a dialog covers it, or it cannot be
    /// scrolled into view.
    pub fn reveal(&mut self, id: &SemanticUiId) -> Result<Invoked, ClientError> {
        let deadline = self.deadline();
        self.reveal_by(id, deadline)
    }

    fn reveal_by(&mut self, id: &SemanticUiId, deadline: Instant) -> Result<Invoked, ClientError> {
        self.invoke_by(
            InspectionSelector {
                capability: Some(REVEAL_CAPABILITY.into()),
                ..InspectionSelector::default()
            },
            InvokeOptions {
                arguments: Some(serde_json::json!({ "value": id.0 })),
                ..InvokeOptions::default()
            },
            deadline,
        )
    }

    /// Read a receipt until it leaves `queued` or the deadline passes.
    fn settle_receipt(
        &mut self,
        mut receipt: InvocationReceipt,
        deadline: Instant,
    ) -> Result<InvocationReceipt, ClientError> {
        while receipt.state == ReceiptState::Queued {
            if Instant::now() + POLL_INTERVAL >= deadline {
                return Err(ClientError::uncertain(
                    &receipt.request_id,
                    "the invocation is still queued at the deadline",
                    None,
                ));
            }
            std::thread::sleep(POLL_INTERVAL);
            let id = self.request_id();
            receipt = match self.request(
                InspectionOperation::Receipt {
                    request_id: receipt.request_id.clone(),
                },
                id,
                deadline,
            ) {
                Ok(InspectionResult::Receipt { receipt }) => receipt,
                Ok(_) => unreachable!("checked kind"),
                Err(error) => {
                    return Err(ClientError::uncertain(
                        &receipt.request_id,
                        format!("its receipt could not be read: {}", error.message),
                        Some(error),
                    ));
                }
            };
        }
        Ok(receipt)
    }

    /// The first observation at least as new as `id`.
    fn observe_from(
        &mut self,
        id: u64,
        deadline: Instant,
    ) -> Result<CompletedObservation, ClientError> {
        loop {
            let observation = self.observe_by(deadline)?;
            if observation.id >= id {
                return Ok(observation);
            }
            if Instant::now() + POLL_INTERVAL >= deadline {
                return Err(ClientError::timeout(format!(
                    "observation {id} was not published; the latest is {}",
                    observation.id
                ))
                .with_observation(observation));
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    /// Poll observations until `condition` holds or the deadline passes. A
    /// timeout keeps the latest observation and the last failure.
    pub fn wait(&mut self, condition: &Condition) -> Result<Waited, ClientError> {
        let deadline = self.deadline();
        let mut baseline: Option<BTreeSet<_>> = match condition {
            Condition::SelectionChanged {
                previous: Some(previous),
                ..
            } => Some(previous.iter().cloned().collect()),
            _ => None,
        };
        let mut last: Option<CompletedObservation> = None;
        let mut last_error: Option<ClientError>;
        loop {
            match self.observe_by(deadline) {
                Ok(observation) => {
                    last_error = None;
                    if let Some(waited) = self.evaluate(condition, &observation, &mut baseline)? {
                        return Ok(waited);
                    }
                    last = Some(observation);
                }
                // A host that has not published yet may publish soon.
                Err(error) if error.code == "missing_observation" => last_error = Some(error),
                Err(error) => return Err(error),
            }
            if Instant::now() + POLL_INTERVAL >= deadline {
                let latest = last
                    .as_ref()
                    .and_then(|observation| latest_state(condition, observation));
                let mut error = ClientError::timeout(format!(
                    "the condition did not hold within {} ms{}{}",
                    self.timeout.as_millis(),
                    latest
                        .map(|latest| format!("; {latest}"))
                        .unwrap_or_default(),
                    last_error
                        .map(|error| format!("; last error: {error}"))
                        .unwrap_or_default()
                ));
                if let Some(last) = last {
                    error = error.with_observation(last);
                }
                return Err(error);
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    fn evaluate(
        &mut self,
        condition: &Condition,
        observation: &CompletedObservation,
        baseline: &mut Option<BTreeSet<stobie_protocol::SemanticUiId>>,
    ) -> Result<Option<Waited>, ClientError> {
        let waited = |node: Option<&UiNode>| {
            Some(Waited {
                observation: observation.clone(),
                node: node.cloned(),
                capability: None,
            })
        };
        let node = match condition.node_selector() {
            Some(selector) => one_node(observation, &selector)?,
            None => None,
        };
        Ok(match condition {
            Condition::Present { selector } => {
                let found = observation
                    .snapshot
                    .nodes
                    .iter()
                    .find(|node| selector.matches_node(node));
                found.and_then(|node| waited(Some(node)))
            }
            Condition::Absent { selector } => (!observation
                .snapshot
                .nodes
                .iter()
                .any(|node| selector.matches_node(node)))
            .then(|| waited(None))
            .flatten(),
            Condition::Enabled { .. } => node
                .filter(|node| node.enabled)
                .and_then(|node| waited(Some(node))),
            Condition::Focused { .. } => node
                .filter(|node| node.focused)
                .and_then(|node| waited(Some(node))),
            Condition::Selected { .. } => node
                .filter(|node| node.selected)
                .and_then(|node| waited(Some(node))),
            Condition::Checked { value, .. } => node
                .filter(|node| node.checked == Some(*value))
                .and_then(|node| waited(Some(node))),
            Condition::Name { value, .. } | Condition::Status { value, .. } => node
                .filter(|node| &node.name == value)
                .and_then(|node| waited(Some(node))),
            Condition::Value { value, .. } => node
                .filter(|node| node.value.as_ref() == Some(value))
                .and_then(|node| waited(Some(node))),
            Condition::SelectionChanged { selector, .. } => {
                let selected = selected_ids(observation, selector);
                match baseline {
                    None => {
                        *baseline = Some(selected);
                        None
                    }
                    Some(previous) if *previous != selected => waited(None),
                    Some(_) => None,
                }
            }
            Condition::Fact { key, value } => observation
                .facts
                .get(key)
                .filter(|fact| serde_json::to_value(fact).ok().as_ref() == Some(value))
                .and_then(|_| waited(None)),
            Condition::AuditsClear {} => {
                let snapshot = &observation.snapshot;
                (snapshot.semantic_audit.is_empty()
                    && snapshot.text_audit.is_empty()
                    && snapshot.text_audit_coverage.is_some())
                .then(|| waited(None))
                .flatten()
            }
            Condition::Content { value } => observation
                .snapshot
                .content_text()
                .any(|text| text.contains(value.as_str()))
                .then(|| waited(None))
                .flatten(),
            Condition::CapabilityEnabled { selector } => {
                let page = self.discover(
                    selector.clone(),
                    Page {
                        limit: 2,
                        cursor: None,
                    },
                )?;
                if page.total > 1 {
                    return Err(ClientError::new(
                        "ambiguous",
                        format!("the selector discovers {} capabilities", page.total),
                        "add the control's id, or pane or domain, to the selector",
                    ));
                }
                page.capabilities
                    .into_iter()
                    .find(|capability| capability.availability.enabled())
                    .map(|capability| Waited {
                        observation: observation.clone(),
                        node: None,
                        capability: Some(capability),
                    })
            }
        })
    }
}

/// What the node a text or state condition names last presented, for a
/// timeout message: `latest value: "Research"` or `the node has no value`.
fn latest_state(condition: &Condition, observation: &CompletedObservation) -> Option<String> {
    let node = one_node(observation, &condition.node_selector()?).ok()??;
    Some(match condition {
        Condition::Value { .. } => match &node.value {
            Some(value) => format!("latest value: {value:?}"),
            None => format!("#{} has no value", node.id.0),
        },
        Condition::Name { .. } | Condition::Status { .. } => {
            format!("latest name: {:?}", node.name)
        }
        Condition::Checked { .. } => match node.checked {
            Some(checked) => format!("latest checked: {checked}"),
            None => format!("#{} is not checkable", node.id.0),
        },
        _ => return None,
    })
}

/// An application error that a request was well formed but refused.
pub fn is_protocol_code(error: &ClientError, code: InspectionErrorCode) -> bool {
    error.cause.as_ref().map(|cause| cause.code) == Some(code)
}
