use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use stobie_protocol::{
    ActionScope, ApplicationIdentity, ArgumentContract, ArgumentOption, Availability,
    BuildIdentity, CapabilityDescription, CollectionMetadata, CompletedObservation,
    INSPECTION_VERSION, InspectionError, InspectionErrorCode, InspectionLimits,
    InspectionOperation, InspectionReply, InspectionRequest, InspectionResult, InspectionSelector,
    InvocationReceipt, ObservationValue, ReceiptState, ResolvedTarget, SET_TEXT_CAPABILITY,
    SemanticUiId, UiSnapshot,
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
    invocations: Vec<(String, serde_json::Value)>,
    receipt_reads: usize,
    after: u64,
    lose_invoke_reply: bool,
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
        id: SemanticUiId::new(id),
        capability: name.into(),
        pane: None,
        domain: None,
        meaning: name.into(),
        observation: 1,
    };
    CapabilityDescription {
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
    ]
}

impl Transport for Fake {
    fn request(&mut self, body: &str, _deadline: Instant) -> Result<String, ClientError> {
        let request: InspectionRequest = serde_json::from_str(body).expect("a valid request");
        let mut app = self.0.borrow_mut();
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
                            nodes: Vec::new(),
                            text: Vec::new(),
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
                        selector
                            .capability
                            .as_ref()
                            .is_none_or(|name| *name == capability.capability)
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
                app.invocations.push((expected.capability, arguments));
                if app.lose_invoke_reply {
                    return Err(ClientError::connection("the socket closed", "retry"));
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
