use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use stobie_protocol::{
    ActionScope, ApplicationIdentity, ArgumentContract, ArgumentOption, Availability,
    BuildIdentity, CapabilityDescription, CollectionMetadata, CompletedObservation,
    INSPECTION_VERSION, InspectionError, InspectionErrorCode, InspectionLimits,
    InspectionOperation, InspectionReply, InspectionRequest, InspectionResult, InspectionSelector,
    InvocationReceipt, ObservationValue, REVEAL_CAPABILITY, ReceiptState, ResolvedTarget,
    SET_TEXT_CAPABILITY, SemanticUiId, TextLayoutObservation, UiNode, UiRect, UiRole, UiSnapshot,
};

use crate::{Client, ClientError, Condition, ErrorClass, InvokeOptions, Transport};

/// A scripted application: each observe publishes a newer observation, and
/// an invocation completes on its second receipt read, presented three
/// observations later.
#[derive(Default)]
struct App {
    instance: String,
    observation: u64,
    facts: Vec<(String, ObservationValue)>,
    nodes: Vec<UiNode>,
    text: Vec<TextLayoutObservation>,
    invocations: Vec<(String, serde_json::Value)>,
    /// The expected target's ID for each invocation.
    targets: Vec<String>,
    receipt_reads: usize,
    after: u64,
    lose_invoke_reply: bool,
    /// How many more reads it answers `missing_observation`, as before its
    /// first publication.
    unpublished: usize,
    /// The IDs a reveal has scrolled into view; `desk.far` is presented
    /// below the fold until revealed.
    revealed: Vec<String>,
}

#[derive(Clone)]
struct Fake(Rc<RefCell<App>>);

impl Fake {
    fn new() -> Self {
        Self(Rc::new(RefCell::new(App {
            instance: "instance-1".into(),
            observation: 1,
            ..App::default()
        })))
    }
}

fn capability(id: &str, name: &str, arguments: ArgumentContract) -> CapabilityDescription {
    let target = ResolvedTarget {
        argument: None,
        id: SemanticUiId::new(id),
        capability: name.into(),
        pane: None,
        domain: None,
        meaning: name.into(),
        observation: 1,
    };
    CapabilityDescription {
        argument: None,
        id: target.id.clone(),
        capability: name.into(),
        pane: None,
        domain: None,
        label: name.into(),
        description: String::new(),
        scope: ActionScope::Application,
        shortcut: None,
        availability: Availability::Enabled,
        rendered: true,
        control_enabled: Some(true),
        semantic_invocable: true,
        arguments,
        target,
    }
}

fn capabilities() -> Vec<CapabilityDescription> {
    vec![
        capability(
            "desk.category",
            "desk.edit-category",
            ArgumentContract::Choice {
                options: vec![ArgumentOption {
                    id: "ideas".into(),
                    label: "Ideas".into(),
                }],
            },
        ),
        capability(
            "desk.notes",
            SET_TEXT_CAPABILITY,
            ArgumentContract::Text {
                max_length: Some(5),
            },
        ),
        // Open, presented on two rows that carry their IDs, and the action's
        // own binding, which takes any ID.
        row("people.open.1", Some(1)),
        row("people.open.2", Some(2)),
        row("people.open", None),
        // A field below the fold, observed only once revealed.
        capability(
            "desk.far",
            SET_TEXT_CAPABILITY,
            ArgumentContract::Text { max_length: None },
        ),
        capability(
            REVEAL_CAPABILITY,
            REVEAL_CAPABILITY,
            ArgumentContract::Text {
                max_length: Some(512),
            },
        ),
        // Archive, presented for person 2 in two lists: two controls that
        // perform the same thing.
        archive("people.list.archive.2"),
        archive("people.recent.archive.2"),
    ]
}

fn archive(id: &str) -> CapabilityDescription {
    let mut capability = row(id, Some(2));
    capability.capability = "people.archive".into();
    capability.target.capability = capability.capability.clone();
    capability.target.meaning = capability.capability.clone();
    capability
}

fn row(id: &str, carried: Option<u32>) -> CapabilityDescription {
    let mut capability = capability(
        id,
        "people.open",
        ArgumentContract::Number {
            minimum: 1.0,
            maximum: 99.0,
            integer: true,
        },
    );
    capability.argument = carried.map(|id| serde_json::json!(id));
    capability.target.argument = capability.argument.clone();
    capability
}

impl Transport for Fake {
    fn request(&mut self, body: &str, _deadline: Instant) -> Result<String, ClientError> {
        let request: InspectionRequest = serde_json::from_str(body).expect("a valid request");
        let mut app = self.0.borrow_mut();
        let read = matches!(
            request.operation,
            InspectionOperation::Observe
                | InspectionOperation::Query { .. }
                | InspectionOperation::Discover { .. }
        );
        if read && app.unpublished > 0 {
            app.unpublished -= 1;
            return Ok(serde_json::to_string(&InspectionReply {
                version: INSPECTION_VERSION,
                request_id: request.request_id,
                instance: app.instance.clone(),
                result: None,
                error: Some(InspectionError {
                    code: InspectionErrorCode::MissingObservation,
                    message: "no observation has been published".into(),
                }),
            })
            .unwrap());
        }
        let receipt = |state, after_observation| InspectionResult::Receipt {
            receipt: InvocationReceipt {
                request_id: "mutation".into(),
                state,
                route: Some("semantic".into()),
                before_observation: None,
                after_observation,
                target: None,
                error: None,
            },
        };
        let result = match request.operation {
            InspectionOperation::Hello => InspectionResult::Hello {
                application: ApplicationIdentity::default(),
                limits: InspectionLimits {
                    query: 64,
                    nodes: 1024,
                    text: 1024,
                    publication_bytes: 1 << 20,
                    request_bytes: 1 << 16,
                    mutations: 64,
                    pending: 8,
                },
                operations: Vec::new(),
                renderer: None,
            },
            InspectionOperation::Observe => {
                app.observation += 1;
                InspectionResult::Observe {
                    observation: CompletedObservation {
                        instance: app.instance.clone(),
                        application: ApplicationIdentity {
                            name: "Desk".into(),
                            build: BuildIdentity::default(),
                        },
                        id: app.observation,
                        snapshot: UiSnapshot {
                            frame: app.observation,
                            pixels_per_point: 1.,
                            root: SemanticUiId::new("root"),
                            nodes: app.nodes.clone(),
                            text: app.text.clone(),
                            text_audit: Vec::new(),
                            text_audit_coverage: None,
                            semantic_audit: Vec::new(),
                        },
                        collection: CollectionMetadata::default(),
                        facts: app.facts.iter().cloned().collect(),
                    },
                }
            }
            InspectionOperation::Discover { selector, .. } => {
                let capabilities: Vec<_> = capabilities()
                    .into_iter()
                    .filter(|capability| {
                        (capability.id.0 != "desk.far"
                            || app.revealed.iter().any(|id| id == "desk.far"))
                            && selector.id.as_ref().is_none_or(|id| *id == capability.id)
                            && selector
                                .capability
                                .as_ref()
                                .is_none_or(|name| *name == capability.capability)
                            && selector.argument.as_ref().is_none_or(|argument| {
                                capability.argument.as_ref().is_some_and(|carried| {
                                    stobie_protocol::same_argument(argument, carried)
                                })
                            })
                    })
                    .collect();
                InspectionResult::Discover {
                    observation: app.observation,
                    total: capabilities.len(),
                    capabilities,
                    next_cursor: None,
                }
            }
            InspectionOperation::Invoke {
                expected,
                arguments,
                ..
            } => {
                app.targets.push(expected.id.0.clone());
                app.invocations
                    .push((expected.capability.clone(), arguments.clone()));
                if app.lose_invoke_reply {
                    return Err(ClientError::connection("the socket closed", "retry"));
                }
                if expected.capability == REVEAL_CAPABILITY {
                    let id = arguments["value"].as_str().unwrap_or_default().to_owned();
                    let mut receipt = receipt(ReceiptState::Completed, Some(app.observation + 1));
                    if id == "desk.far" {
                        app.revealed.push(id);
                    } else if let InspectionResult::Receipt { receipt } = &mut receipt {
                        receipt.state = ReceiptState::Rejected;
                        receipt.after_observation = None;
                        receipt.error = Some(InspectionError {
                            code: InspectionErrorCode::ValidationFailed,
                            message: format!("{id} is not presented"),
                        });
                    }
                    return Ok(serde_json::to_string(&InspectionReply {
                        version: INSPECTION_VERSION,
                        request_id: request.request_id,
                        instance: app.instance.clone(),
                        result: Some(receipt),
                        error: None,
                    })
                    .unwrap());
                }
                app.after = app.observation + 3;
                receipt(ReceiptState::Queued, None)
            }
            InspectionOperation::Receipt { .. } => {
                app.receipt_reads += 1;
                if app.receipt_reads < 2 {
                    receipt(ReceiptState::Queued, None)
                } else {
                    receipt(ReceiptState::Completed, Some(app.after))
                }
            }
            InspectionOperation::Cancel { .. } => {
                return Ok(serde_json::to_string(&InspectionReply {
                    version: INSPECTION_VERSION,
                    request_id: request.request_id,
                    instance: app.instance.clone(),
                    result: None,
                    error: Some(InspectionError {
                        code: InspectionErrorCode::UnknownRequest,
                        message: "no such request".into(),
                    }),
                })
                .unwrap());
            }
            InspectionOperation::Query { .. } => unimplemented!("not scripted"),
        };
        Ok(serde_json::to_string(&InspectionReply {
            version: INSPECTION_VERSION,
            request_id: request.request_id,
            instance: app.instance.clone(),
            result: Some(result),
            error: None,
        })
        .unwrap())
    }

    fn describe(&self) -> String {
        "fake".into()
    }
}

fn client() -> (Client<Fake>, Fake) {
    let fake = Fake::new();
    let mut client = Client::new(fake.clone()).with_timeout(Duration::from_secs(5));
    client.hello().unwrap();
    (client, fake)
}

fn select(capability: &str) -> InspectionSelector {
    InspectionSelector {
        capability: Some(capability.into()),
        ..InspectionSelector::default()
    }
}

#[test]
fn invoke_returns_once_the_completed_effect_is_observed() {
    let (mut client, fake) = client();
    let invoked = client
        .invoke(
            select("desk.edit-category"),
            InvokeOptions {
                arguments: Some(serde_json::json!({ "value": "ideas" })),
                ..InvokeOptions::default()
            },
        )
        .unwrap();
    assert_eq!(invoked.receipt.state, ReceiptState::Completed);
    let after = invoked.receipt.after_observation.unwrap();
    assert!(invoked.observation.unwrap().id >= after);
    let app = fake.0.borrow();
    assert_eq!(
        app.invocations,
        [(
            "desk.edit-category".to_owned(),
            serde_json::json!({ "value": "ideas" })
        )]
    );
    assert_eq!(app.receipt_reads, 2);
}

#[test]
fn arguments_outside_the_contract_are_rejected_before_sending() {
    let (mut client, fake) = client();
    for arguments in [
        serde_json::json!({ "value": "archive" }),
        serde_json::Value::Null,
    ] {
        let error = client
            .invoke(
                select("desk.edit-category"),
                InvokeOptions {
                    arguments: Some(arguments),
                    ..InvokeOptions::default()
                },
            )
            .unwrap_err();
        assert_eq!(error.code, "invalid_arguments", "{error}");
    }
    let error = client
        .set_text(
            InspectionSelector {
                id: Some(SemanticUiId::new("desk.notes")),
                ..InspectionSelector::default()
            },
            "longer than five",
            InvokeOptions::default(),
        )
        .unwrap_err();
    assert_eq!(error.code, "invalid_arguments", "{error}");
    assert!(fake.0.borrow().invocations.is_empty());

    client
        .set_text(
            select(SET_TEXT_CAPABILITY),
            "short",
            InvokeOptions::default(),
        )
        .unwrap();
    assert_eq!(
        fake.0.borrow().invocations,
        [(
            SET_TEXT_CAPABILITY.to_owned(),
            serde_json::json!({ "value": "short" })
        )]
    );
}

#[test]
fn a_lost_invocation_reply_is_uncertain_and_never_resent() {
    let (mut client, fake) = client();
    fake.0.borrow_mut().lose_invoke_reply = true;
    let error = client
        .invoke(
            select("desk.edit-category"),
            InvokeOptions {
                arguments: Some(serde_json::json!({ "value": "ideas" })),
                request_id: Some("mutation-1".into()),
                ..InvokeOptions::default()
            },
        )
        .unwrap_err();
    assert_eq!(error.code, "uncertain_completion");
    assert_eq!(error.class(), ErrorClass::Deadline);
    assert_eq!(error.request_id.as_deref(), Some("mutation-1"));
    assert!(error.hint.contains("stobie receipt mutation-1"));
    assert_eq!(fake.0.borrow().invocations.len(), 1);
}

#[test]
fn an_application_error_keeps_its_protocol_code() {
    let (mut client, _) = client();
    let error = client.cancel("nothing").unwrap_err();
    assert_eq!(error.code, "unknown_request");
    assert_eq!(error.class(), ErrorClass::Application);
}

#[test]
fn a_restarted_application_is_reported_rather_than_addressed() {
    let (mut client, fake) = client();
    fake.0.borrow_mut().instance = "instance-2".into();
    let error = client.observe().unwrap_err();
    assert_eq!(error.code, "wrong_instance");
    assert_eq!(error.class(), ErrorClass::Connection);
}

#[test]
fn waits_return_the_observation_that_satisfies_them_or_keep_the_last_on_timeout() {
    let (client, fake) = client();
    let mut client = client.with_timeout(Duration::from_millis(300));
    let condition = Condition::Fact {
        key: "undo_entries".into(),
        value: serde_json::json!(1),
    };
    let error = client.wait(&condition).unwrap_err();
    assert_eq!(error.code, "timeout");
    assert!(error.observation.is_some(), "a timeout keeps what it saw");

    fake.0.borrow_mut().facts = vec![("undo_entries".into(), ObservationValue::UnsignedInteger(1))];
    let waited = client.wait(&condition).unwrap();
    assert_eq!(
        serde_json::to_value(&waited.observation.facts["undo_entries"]).unwrap(),
        serde_json::json!(1)
    );
    assert_eq!(
        Condition::parse(serde_json::json!({ "condition": "audits_clear" })).unwrap(),
        Condition::AuditsClear {}
    );
}

#[test]
fn content_waits_match_published_excerpts_and_never_withheld_text() {
    let (client, fake) = client();
    let mut client = client.with_timeout(Duration::from_millis(200));
    let text = |content: serde_json::Value| -> TextLayoutObservation {
        let rect = serde_json::json!({ "min_x": 0., "min_y": 0., "max_x": 10., "max_y": 10. });
        serde_json::from_value(serde_json::json!({
            "component_id": { "kind": "reading_text", "instance": 1 },
            "parent_id": null,
            "role": "body",
            "horizontal_alignment": "start",
            "vertical_alignment": "top",
            "allocated_rect": rect,
            "painted_rect": rect,
            "clip_rect": rect,
            "baseline": null,
            "overflow": "wrap_unbounded",
            "declared_max_lines": 0,
            "line_count": 1,
            "truncated": false,
            "content": content,
        }))
        .unwrap()
    };
    let condition = Condition::parse(serde_json::json!({
        "condition": "content",
        "value": "today's pick"
    }))
    .unwrap();
    fake.0.borrow_mut().text = vec![text(serde_json::json!({ "kind": "withheld" }))];
    assert_eq!(client.wait(&condition).unwrap_err().code, "timeout");
    fake.0.borrow_mut().text.push(text(serde_json::json!({
        "kind": "excerpt",
        "text": "Here is today's pick: a paper on reasoning",
        "truncated": false
    })));
    let waited = client.wait(&condition).unwrap();
    assert!(
        waited
            .observation
            .snapshot
            .content_text()
            .any(|text| text.contains("today's pick"))
    );
}

#[test]
fn a_value_wait_that_times_out_says_what_the_node_presents() {
    let (client, fake) = client();
    let mut client = client.with_timeout(Duration::from_millis(200));
    let node = |value: Option<&str>| {
        let mut node = UiNode::container(
            SemanticUiId::new("desk.category"),
            None,
            UiRole::ComboBox,
            UiRect {
                min_x: 0.,
                min_y: 0.,
                max_x: 10.,
                max_y: 10.,
            },
        );
        node.value = value.map(ToOwned::to_owned);
        node
    };
    let condition = Condition::Value {
        selector: InspectionSelector {
            id: Some(SemanticUiId::new("desk.category")),
            ..InspectionSelector::default()
        },
        value: "Ideas".into(),
    };
    fake.0.borrow_mut().nodes = vec![node(None)];
    let error = client.wait(&condition).unwrap_err();
    assert!(
        error.message.contains("#desk.category has no value"),
        "{error}"
    );
    fake.0.borrow_mut().nodes = vec![node(Some("Research"))];
    let error = client.wait(&condition).unwrap_err();
    assert!(
        error.message.contains(r#"latest value: "Research""#),
        "{error}"
    );
    fake.0.borrow_mut().nodes = vec![node(Some("Ideas"))];
    assert!(client.wait(&condition).is_ok());
}

#[test]
fn a_value_invokes_the_rows_control_that_carries_it() {
    let (mut client, fake) = client();
    let invoke = |client: &mut Client<Fake>, selector, arguments| {
        client.invoke(
            selector,
            InvokeOptions {
                arguments,
                ..InvokeOptions::default()
            },
        )
    };
    invoke(
        &mut client,
        select("people.open"),
        Some(serde_json::json!({ "value": 2 })),
    )
    .unwrap();
    // A value no row carries goes to the action's own binding.
    invoke(
        &mut client,
        select("people.open"),
        Some(serde_json::json!({ "value": 7 })),
    )
    .unwrap();
    // A selector's argument names the row, which needs no value.
    invoke(
        &mut client,
        InspectionSelector {
            argument: Some(serde_json::json!(1)),
            ..select("people.open")
        },
        None,
    )
    .unwrap();
    assert_eq!(
        fake.0.borrow().targets,
        ["people.open.2", "people.open", "people.open.1"]
    );
    // Without a value, the rows and the action's binding are ambiguous.
    let error = invoke(&mut client, select("people.open"), None).unwrap_err();
    assert_eq!(error.code, "ambiguous", "{error}");
    assert!(
        error.message.contains("people.open.2 carrying 2"),
        "{error}"
    );
    assert_eq!(fake.0.borrow().invocations.len(), 3);
}

#[test]
fn reads_wait_for_the_first_publication_within_the_deadline() {
    let (mut client, fake) = client();
    fake.0.borrow_mut().unpublished = 3;
    assert!(client.observe().is_ok());
    fake.0.borrow_mut().unpublished = 3;
    let page = client
        .discover(select("desk.edit-category"), crate::Page::default())
        .unwrap();
    assert_eq!(page.total, 1);
    // An invocation discovers its target after the first publication too.
    fake.0.borrow_mut().unpublished = 3;
    client
        .invoke(
            select("desk.edit-category"),
            InvokeOptions {
                arguments: Some(serde_json::json!({ "value": "ideas" })),
                ..InvokeOptions::default()
            },
        )
        .unwrap();
    // One that never publishes fails at the deadline with its code.
    let mut client = client.with_timeout(Duration::from_millis(200));
    fake.0.borrow_mut().unpublished = usize::MAX;
    let started = Instant::now();
    let error = client.observe().unwrap_err();
    assert_eq!(error.code, "missing_observation", "{error}");
    assert!(started.elapsed() >= Duration::from_millis(100));
}

#[test]
fn set_text_on_a_field_out_of_view_reveals_it_first() {
    let (mut client, fake) = client();
    let far = InspectionSelector {
        id: Some(SemanticUiId::new("desk.far")),
        ..InspectionSelector::default()
    };
    let invoked = client
        .set_text(far, "a note", InvokeOptions::default())
        .unwrap();
    assert_eq!(invoked.receipt.state, ReceiptState::Completed);
    let app = fake.0.borrow();
    assert_eq!(app.targets, [REVEAL_CAPABILITY, "desk.far"]);
    assert_eq!(
        app.invocations[0],
        (
            REVEAL_CAPABILITY.to_owned(),
            serde_json::json!({ "value": "desk.far" })
        )
    );
}

#[test]
fn an_id_no_pass_presents_fails_with_the_reveals_reason() {
    let (mut client, fake) = client();
    let error = client
        .set_text(
            InspectionSelector {
                id: Some(SemanticUiId::new("desk.gone")),
                ..InspectionSelector::default()
            },
            "text",
            InvokeOptions::default(),
        )
        .unwrap_err();
    assert_eq!(error.code, "no_match", "{error}");
    assert!(
        error.message.contains("desk.gone is not presented"),
        "{error}"
    );
    // Only the reveal ran; nothing was set.
    assert_eq!(fake.0.borrow().targets, [REVEAL_CAPABILITY]);
    // A direct reveal reports the same failure on its receipt.
    let revealed = client.reveal(&SemanticUiId::new("desk.gone")).unwrap();
    assert_eq!(revealed.receipt.state, ReceiptState::Rejected);
}

#[test]
fn controls_that_perform_the_same_thing_resolve_to_the_least_id() {
    let (mut client, fake) = client();
    client
        .invoke(select("people.archive"), InvokeOptions::default())
        .unwrap();
    client
        .invoke(
            select("people.archive"),
            InvokeOptions {
                arguments: Some(serde_json::json!({ "value": 2 })),
                ..InvokeOptions::default()
            },
        )
        .unwrap();
    assert_eq!(
        fake.0.borrow().targets,
        ["people.list.archive.2", "people.list.archive.2"]
    );
    assert_eq!(
        client.discover_one(select("people.archive")).unwrap().id.0,
        "people.list.archive.2"
    );
    let waited = client
        .wait(&Condition::CapabilityEnabled {
            selector: select("people.archive"),
        })
        .unwrap();
    assert_eq!(waited.capability.unwrap().id.0, "people.list.archive.2");
    // Rows carrying different values remain several choices.
    let error = client.discover_one(select("people.open")).unwrap_err();
    assert_eq!(error.code, "ambiguous", "{error}");
}
