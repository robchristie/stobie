#![cfg(test)]

use super::*;

#[test]
fn negotiated_operations_identity_and_unsupported_operation_are_explicit() {
    let (inspection, snapshot, bindings) = fixture();
    publish(&inspection, &snapshot, &bindings);
    let reply = inspection.handle(request(
        &inspection,
        "hello-operations",
        InspectionOperation::Hello,
    ));
    let Some(InspectionResult::Hello { operations, .. }) = reply.result else {
        panic!("hello response");
    };
    assert_eq!(operations.len(), 7);
    assert!(operations.contains(&InspectionOperation::Observe.kind()));
    let response = inspection.handle(request(
        &inspection,
        "observation-identity",
        InspectionOperation::Observe,
    ));
    let Some(InspectionResult::Observe { observation }) = response.result else {
        panic!("observation response");
    };
    assert_eq!(observation.instance, inspection.instance());
    assert_eq!(observation.application.name, "test");
    let unknown = inspection
        .handle_json(r#"{"version":1,"request_id":"unknown-op","operation":{"op":"evaluate"}}"#);
    let reply: InspectionReply = serde_json::from_str(&unknown).unwrap();
    assert_eq!(reply.request_id, "unknown-op");
    assert_eq!(
        reply.error.unwrap().code,
        InspectionErrorCode::UnsupportedOperation
    );
    let invalid_selector = inspection.handle_json(r#"{"version":1,"request_id":"invalid-selector","operation":{"op":"query","selector":{"pane":-1}}}"#);
    let reply: InspectionReply = serde_json::from_str(&invalid_selector).unwrap();
    assert_eq!(reply.request_id, "invalid-selector");
    assert_eq!(
        reply.error.unwrap().code,
        InspectionErrorCode::InvalidRequest
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
enum Action {
    Fit,
}

impl ActionKey for Action {
    fn stable_id(self) -> &'static str {
        "fit"
    }
    fn specification(self) -> crate::ActionSpec<Self> {
        crate::ActionSpec {
            id: self,
            label: "Fit view",
            description: "Fit this pane to its content",
            compact_label: None,
            shortcut: None,
            scope: ActionScope::Pane,
        }
    }
}

fn rect(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> UiRect {
    UiRect {
        min_x,
        min_y,
        max_x,
        max_y,
    }
}

fn fixture() -> (Inspection, UiSnapshot, Vec<InspectionBinding<Action>>) {
    let inspection = Inspection::with_build_identity(
        "test",
        BuildIdentity {
            version: "test-version".into(),
            source_revision: Some("source".into()),
        },
    );
    let root = UiNode::container(
        SemanticUiId::root(),
        None,
        UiRole::Application,
        rect(0., 0., 100., 100.),
    );
    let mut node = UiNode::container(
        "fit-one".to_owned().into(),
        Some(root.id.clone()),
        UiRole::Button,
        rect(10., 20., 30., 40.),
    );
    node.name = "Fit view".into();
    node.pane = Some(PaneId(1));
    node.domain_reference = Some(DomainReference::External {
        namespace: "record".into(),
        id: "7".into(),
    });
    node.actions
        .push(crate::SemanticActionId::from_action(Action::Fit));
    let bindings = vec![
        InspectionBinding::new(
            ActionTarget::pane(Action::Fit, PaneId(1)),
            Availability::Enabled,
            true,
            "record:7",
        )
        .with_id(node.id.clone())
        .with_domain(node.domain_reference.clone().unwrap()),
    ];
    let snapshot = UiSnapshot {
        frame: 7,
        pixels_per_point: 2.,
        root: root.id.clone(),
        nodes: vec![root, node],
        ..UiSnapshot::default()
    };
    (inspection, snapshot, bindings)
}

fn publish(
    inspection: &Inspection,
    snapshot: &UiSnapshot,
    bindings: &[InspectionBinding<Action>],
) -> u64 {
    inspection
        .publish(
            snapshot.clone(),
            bindings,
            CollectionMetadata::default(),
            BTreeMap::new(),
        )
        .unwrap()
}

fn request(inspection: &Inspection, id: &str, operation: InspectionOperation) -> InspectionRequest {
    InspectionRequest {
        version: INSPECTION_VERSION,
        request_id: id.into(),
        instance: Some(inspection.instance()),
        operation,
    }
}

fn capability_selector() -> InspectionSelector {
    InspectionSelector {
        capability: Some("fit".into()),
        ..InspectionSelector::default()
    }
}

fn target(inspection: &Inspection) -> ResolvedTarget {
    let response = inspection.handle(request(
        inspection,
        "discovery",
        InspectionOperation::Discover {
            selector: capability_selector(),
            limit: 64,
            cursor: None,
        },
    ));
    let Some(InspectionResult::Discover { capabilities, .. }) = response.result else {
        panic!("expected discovery")
    };
    capabilities[0].target.clone()
}

fn invocation(inspection: &Inspection, id: &str, expected: ResolvedTarget) -> InspectionRequest {
    request(
        inspection,
        id,
        InspectionOperation::Invoke {
            selector: InspectionSelector {
                id: Some(expected.id.clone()),
                ..capability_selector()
            },
            expected,
            arguments: serde_json::Value::Null,
        },
    )
}

fn receipt(reply: InspectionReply) -> InvocationReceipt {
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("expected receipt, got {:?}", reply.error)
    };
    receipt
}

fn lookup(inspection: &Inspection, id: &str) -> InvocationReceipt {
    receipt(inspection.handle(request(
        inspection,
        "lookup",
        InspectionOperation::Receipt {
            request_id: id.into(),
        },
    )))
}

#[test]
fn protocol_version_instance_and_strict_envelope_are_enforced() {
    let (inspection, _, _) = fixture();
    let mut hello = request(&inspection, "hello", InspectionOperation::Hello);
    hello.instance = None;
    let response = inspection.handle(hello.clone());
    assert!(
        matches!(response.result, Some(InspectionResult::Hello { application, .. })
        if application.build.version == "test-version")
    );
    hello.version = 2;
    assert_eq!(
        inspection.handle(hello).error.unwrap().code,
        InspectionErrorCode::UnsupportedVersion
    );
    let mut observe = request(&inspection, "observe", InspectionOperation::Observe);
    observe.instance = None;
    assert_eq!(
        inspection.handle(observe).error.unwrap().code,
        InspectionErrorCode::WrongInstance
    );
    let malformed = r#"{"version":1,"request_id":"x","operation":{"op":"hello","eval":"bad"}}"#;
    let reply: InspectionReply = serde_json::from_str(&inspection.handle_json(malformed)).unwrap();
    assert_eq!(
        reply.error.unwrap().code,
        InspectionErrorCode::InvalidRequest
    );
}

#[test]
fn missing_observation_and_bounded_empty_query_are_explicit() {
    let (inspection, snapshot, bindings) = fixture();
    assert_eq!(
        inspection
            .handle(request(&inspection, "o", InspectionOperation::Observe))
            .error
            .unwrap()
            .code,
        InspectionErrorCode::MissingObservation
    );
    publish(&inspection, &snapshot, &bindings);
    let response = inspection.handle(request(
        &inspection,
        "q",
        InspectionOperation::Query {
            selector: InspectionSelector {
                name: Some("absent".into()),
                ..InspectionSelector::default()
            },
            limit: 1,
            cursor: None,
        },
    ));
    assert!(
        matches!(response.result, Some(InspectionResult::Query { total: 0, nodes, next_cursor: None, .. }) if nodes.is_empty())
    );
    for limit in [0, 257, usize::MAX] {
        let response = inspection.handle(request(
            &inspection,
            "limit",
            InspectionOperation::Query {
                selector: InspectionSelector::default(),
                limit,
                cursor: None,
            },
        ));
        assert_eq!(
            response.error.unwrap().code,
            InspectionErrorCode::InvalidRequest
        );
    }
}

#[test]
fn publication_is_atomic_bounded_and_cursor_is_observation_bound() {
    let (inspection, mut snapshot, bindings) = fixture();
    let first = publish(&inspection, &snapshot, &bindings);
    let response = inspection.handle(request(
        &inspection,
        "first",
        InspectionOperation::Query {
            selector: InspectionSelector::default(),
            limit: 1,
            cursor: None,
        },
    ));
    let Some(InspectionResult::Query {
        next_cursor: Some(cursor),
        ..
    }) = response.result
    else {
        panic!("expected page")
    };
    snapshot.frame += 1;
    let second = publish(&inspection, &snapshot, &bindings);
    assert!(second > first);
    let response = inspection.handle(request(
        &inspection,
        "stale",
        InspectionOperation::Query {
            selector: InspectionSelector::default(),
            limit: 1,
            cursor: Some(cursor),
        },
    ));
    assert_eq!(
        response.error.unwrap().code,
        InspectionErrorCode::StaleCursor
    );
    snapshot.nodes = (0..=INSPECTION_NODE_LIMIT)
        .map(|index| {
            UiNode::container(
                format!("node-{index}").into(),
                None,
                UiRole::Status,
                rect(0., 0., 1., 1.),
            )
        })
        .collect();
    assert_eq!(
        inspection
            .publish(
                snapshot,
                &bindings,
                CollectionMetadata::default(),
                BTreeMap::new()
            )
            .unwrap_err()
            .code,
        InspectionErrorCode::PublicationLimit
    );
    let Some(InspectionResult::Observe { observation }) = inspection
        .handle(request(
            &inspection,
            "retained",
            InspectionOperation::Observe,
        ))
        .result
    else {
        panic!("expected observation")
    };
    assert_eq!(observation.id, second);
    assert_eq!(observation.snapshot.frame, 8);
}

#[test]
fn oversized_text_fact_and_typed_request_are_rejected() {
    let (inspection, snapshot, bindings) = fixture();
    let facts = BTreeMap::from([(
        "large".into(),
        ObservationValue::Text("x".repeat(INSPECTION_PUBLICATION_BYTES)),
    )]);
    assert_eq!(
        inspection
            .publish(snapshot, &bindings, CollectionMetadata::default(), facts)
            .unwrap_err()
            .code,
        InspectionErrorCode::PublicationLimit
    );
    let large = request(
        &inspection,
        "large",
        InspectionOperation::Receipt {
            request_id: "x".repeat(INSPECTION_REQUEST_BYTES),
        },
    );
    assert_eq!(
        inspection.handle(large).error.unwrap().code,
        InspectionErrorCode::InvalidRequest
    );
}

#[test]
fn query_selectors_are_conjunctive_and_return_binding_tokens() {
    let (inspection, snapshot, bindings) = fixture();
    publish(&inspection, &snapshot, &bindings);
    let selector = InspectionSelector {
        role: Some(UiRole::Button),
        name: Some("Fit view".into()),
        pane: Some(PaneId(1)),
        domain: snapshot.nodes[1].domain_reference.clone(),
        ..capability_selector()
    };
    let response = inspection.handle(request(
        &inspection,
        "q",
        InspectionOperation::Query {
            selector: selector.clone(),
            limit: 1,
            cursor: None,
        },
    ));
    let Some(InspectionResult::Query {
        total: 1, nodes, ..
    }) = response.result
    else {
        panic!("expected one query match")
    };
    assert_eq!(nodes[0].targets[0], target(&inspection));
    let response = inspection.handle(request(
        &inspection,
        "wrong-pane",
        InspectionOperation::Query {
            selector: InspectionSelector {
                pane: Some(PaneId(2)),
                ..selector
            },
            limit: 1,
            cursor: None,
        },
    ));
    assert!(matches!(
        response.result,
        Some(InspectionResult::Query { total: 0, .. })
    ));
}

#[test]
fn repeated_capabilities_require_specific_identity_and_discovery_is_typed() {
    let (inspection, mut snapshot, mut bindings) = fixture();
    let mut second = snapshot.nodes[1].clone();
    second.id = "fit-two".to_owned().into();
    second.pane = Some(PaneId(2));
    second.domain_reference = Some(DomainReference::Pane(PaneId(2)));
    bindings.push(
        InspectionBinding::new(
            ActionTarget::pane(Action::Fit, PaneId(2)),
            Availability::Enabled,
            true,
            "pane:2",
        )
        .with_id(second.id.clone())
        .with_domain(second.domain_reference.clone().unwrap()),
    );
    snapshot.nodes.push(second);
    publish(&inspection, &snapshot, &bindings);
    let expected = target(&inspection);
    let response = receipt(inspection.handle(request(
        &inspection,
        "ambiguous",
        InspectionOperation::Invoke {
            selector: capability_selector(),
            expected,
            arguments: serde_json::Value::Null,
        },
    )));
    assert_eq!(response.state, ReceiptState::Rejected);
    assert_eq!(response.error.unwrap().code, InspectionErrorCode::Ambiguous);
    let response = inspection.handle(request(
        &inspection,
        "all",
        InspectionOperation::Discover {
            selector: capability_selector(),
            limit: 256,
            cursor: None,
        },
    ));
    let Some(InspectionResult::Discover { capabilities, .. }) = response.result else {
        panic!("expected capabilities")
    };
    assert_eq!(capabilities.len(), 2);
    assert!(capabilities.iter().all(|capability| capability.description
        == Action::Fit.specification().description
        && capability.capability == Action::Fit.stable_id()));
}

#[test]
fn hidden_disabled_and_physical_only_capabilities_remain_discoverable() {
    for (availability, semantic_invocable, code) in [
        (Availability::Hidden, true, InspectionErrorCode::Unavailable),
        (
            Availability::Disabled {
                reason: "no selection".into(),
            },
            true,
            InspectionErrorCode::Unavailable,
        ),
        (
            Availability::Enabled,
            false,
            InspectionErrorCode::NotInvocable,
        ),
    ] {
        let (inspection, snapshot, mut bindings) = fixture();
        bindings[0].availability = availability.clone();
        bindings[0].semantic_invocable = semantic_invocable;
        publish(&inspection, &snapshot, &bindings);
        let expected = target(&inspection);
        let response = receipt(inspection.handle(invocation(&inspection, "invoke", expected)));
        assert_eq!(response.state, ReceiptState::Rejected);
        assert_eq!(response.error.unwrap().code, code);
        assert!(!inspection.has_pending());
    }
}

#[test]
fn invocation_rechecks_current_availability_without_repainting() {
    let (inspection, snapshot, mut bindings) = fixture();
    publish(&inspection, &snapshot, &bindings);
    let expected = target(&inspection);
    assert_eq!(
        receipt(inspection.handle(invocation(&inspection, "invoke", expected))).state,
        ReceiptState::Queued
    );
    bindings[0].availability = Availability::Disabled {
        reason: "changed since observation".into(),
    };
    let mut executions = 0;
    assert!(inspection.drain(&bindings, |_| {
        executions += 1;
        Ok(())
    }));
    assert_eq!(executions, 0);
    let response = lookup(&inspection, "invoke");
    assert_eq!(response.state, ReceiptState::Rejected);
    assert_eq!(
        response.error.unwrap().code,
        InspectionErrorCode::Unavailable
    );
}

#[test]
fn target_disappearance_and_reused_id_meaning_are_rejected() {
    for disappeared in [false, true] {
        let (inspection, snapshot, mut bindings) = fixture();
        publish(&inspection, &snapshot, &bindings);
        inspection.handle(invocation(&inspection, "invoke", target(&inspection)));
        if disappeared {
            bindings.clear();
        } else {
            bindings[0].meaning = "different-record".into();
        }
        inspection.drain(&bindings, |_| panic!("stale target must not execute"));
        assert_eq!(
            lookup(&inspection, "invoke").error.unwrap().code,
            InspectionErrorCode::StaleTarget
        );
    }
}

#[test]
fn harmless_geometry_and_focus_frames_do_not_invalidate_meaning() {
    let (inspection, mut snapshot, bindings) = fixture();
    publish(&inspection, &snapshot, &bindings);
    let expected = target(&inspection);
    snapshot.frame += 1;
    snapshot.nodes[1].focused = true;
    snapshot.nodes[1].rect = rect(40., 40., 60., 60.);
    publish(&inspection, &snapshot, &bindings);
    let response = receipt(inspection.dispatch(
        invocation(&inspection, "invoke", expected),
        &bindings,
        |target| {
            assert_eq!(target, bindings[0].target);
            Ok(())
        },
    ));
    assert_eq!(response.state, ReceiptState::Completed);
    assert_eq!(response.route.as_deref(), Some("semantic"));
    assert_eq!(response.before_observation, Some(2));
}

#[test]
fn duplicate_request_ids_never_rerun_and_changed_bodies_conflict() {
    let (inspection, snapshot, bindings) = fixture();
    publish(&inspection, &snapshot, &bindings);
    let invocation = invocation(&inspection, "invoke", target(&inspection));
    inspection.handle(invocation.clone());
    inspection.handle(invocation.clone());
    assert_eq!(inspection.state.lock().unwrap().pending.len(), 1);
    let mut executions = 0;
    inspection.drain(&bindings, |_| {
        executions += 1;
        Ok(())
    });
    assert_eq!(
        receipt(inspection.dispatch(invocation.clone(), &bindings, |_| {
            executions += 1;
            Ok(())
        }))
        .state,
        ReceiptState::Completed
    );
    assert_eq!(executions, 1);
    let mut changed = invocation;
    let InspectionOperation::Invoke { expected, .. } = &mut changed.operation else {
        unreachable!()
    };
    expected.meaning = "changed".into();
    assert_eq!(
        inspection.handle(changed).error.unwrap().code,
        InspectionErrorCode::RequestIdConflict
    );
}

#[test]
fn replay_of_a_completed_request_does_not_drain_another_queued_action() {
    let (inspection, snapshot, bindings) = fixture();
    publish(&inspection, &snapshot, &bindings);
    let first = invocation(&inspection, "first", target(&inspection));
    inspection.dispatch(first.clone(), &bindings, |_| Ok(()));
    inspection.handle(invocation(&inspection, "second", target(&inspection)));
    let response = receipt(inspection.dispatch(first, &bindings, |_| {
        panic!("replay cannot dispatch other queued mutations")
    }));
    assert_eq!(response.state, ReceiptState::Completed);
    assert_eq!(lookup(&inspection, "second").state, ReceiptState::Queued);
}

#[test]
fn explicit_host_nonce_is_validated_without_process_or_clock_requirements() {
    assert!(Inspection::with_instance("browser", BuildIdentity::default(), "browser-uuid").is_ok());
    assert!(Inspection::with_instance("browser", BuildIdentity::default(), "").is_err());
    assert!(
        Inspection::with_instance("browser", BuildIdentity::default(), "x".repeat(129)).is_err()
    );
}

#[test]
fn invocation_no_match_and_future_observation_are_rejected_before_queueing() {
    let (inspection, snapshot, bindings) = fixture();
    publish(&inspection, &snapshot, &bindings);
    let expected = target(&inspection);
    let mut missing = invocation(&inspection, "missing", expected.clone());
    let InspectionOperation::Invoke { selector, .. } = &mut missing.operation else {
        unreachable!()
    };
    selector.id = Some("absent".to_owned().into());
    assert_eq!(
        receipt(inspection.handle(missing)).error.unwrap().code,
        InspectionErrorCode::NoMatch
    );
    let mut future = expected;
    future.observation += 1;
    assert_eq!(
        receipt(inspection.handle(invocation(&inspection, "future", future)))
            .error
            .unwrap()
            .code,
        InspectionErrorCode::StaleTarget
    );
    assert!(!inspection.has_pending());
}

#[test]
fn invalid_arguments_are_rejected_and_retained_without_dispatch() {
    let (inspection, snapshot, bindings) = fixture();
    publish(&inspection, &snapshot, &bindings);
    for (index, arguments) in [
        serde_json::json!({"value": 1}),
        serde_json::json!([1]),
        serde_json::json!(false),
    ]
    .into_iter()
    .enumerate()
    {
        let mut invocation = invocation(&inspection, &format!("args-{index}"), target(&inspection));
        let InspectionOperation::Invoke {
            arguments: current, ..
        } = &mut invocation.operation
        else {
            unreachable!()
        };
        *current = arguments;
        let response = receipt(inspection.handle(invocation.clone()));
        assert_eq!(
            response.error.unwrap().code,
            InspectionErrorCode::InvalidArguments
        );
        assert_eq!(
            receipt(inspection.handle(invocation)).state,
            ReceiptState::Rejected
        );
    }
    assert!(!inspection.has_pending());
}

#[test]
fn drain_is_one_mutation_so_host_can_recompute_between_actions() {
    let (inspection, snapshot, mut bindings) = fixture();
    publish(&inspection, &snapshot, &bindings);
    let expected = target(&inspection);
    inspection.handle(invocation(&inspection, "one", expected.clone()));
    inspection.handle(invocation(&inspection, "two", expected));
    let mut executions = 0;
    inspection.drain(&bindings, |_| {
        executions += 1;
        Ok(())
    });
    assert!(inspection.has_pending());
    bindings[0].availability = Availability::Hidden;
    inspection.drain(&bindings, |_| {
        executions += 1;
        Ok(())
    });
    assert_eq!(executions, 1);
    assert_eq!(lookup(&inspection, "one").state, ReceiptState::Completed);
    assert_eq!(lookup(&inspection, "two").state, ReceiptState::Rejected);
}

#[test]
fn ledger_never_evicts_and_history_full_does_not_admit_mutation() {
    let (inspection, snapshot, bindings) = fixture();
    publish(&inspection, &snapshot, &bindings);
    let expected = target(&inspection);
    let first = invocation(&inspection, "mutation-0", expected.clone());
    for index in 0..INSPECTION_HISTORY_LIMIT {
        inspection.handle(invocation(
            &inspection,
            &format!("mutation-{index}"),
            expected.clone(),
        ));
        inspection.drain(&bindings, |_| Ok(()));
    }
    assert_eq!(
        receipt(inspection.handle(first)).state,
        ReceiptState::Completed
    );
    let overflow = inspection.handle(invocation(&inspection, "overflow", expected));
    assert_eq!(
        overflow.error.unwrap().code,
        InspectionErrorCode::HistoryFull
    );
    assert_eq!(lookup(&inspection, "overflow").state, ReceiptState::Unknown);
    assert!(!inspection.has_pending());
}

#[test]
fn queue_full_is_a_retained_rejection_and_reads_do_not_wake_ui() {
    let (inspection, snapshot, bindings) = fixture();
    publish(&inspection, &snapshot, &bindings);
    let wakes = Arc::new(AtomicU64::new(0));
    let count = Arc::clone(&wakes);
    inspection.state.lock().unwrap().waker = Some(Arc::new(move || {
        count.fetch_add(1, Ordering::Relaxed);
    }));
    inspection.handle(request(&inspection, "read", InspectionOperation::Observe));
    target(&inspection);
    assert_eq!(wakes.load(Ordering::Relaxed), 0);
    let expected = target(&inspection);
    for index in 0..INSPECTION_QUEUE_LIMIT {
        inspection.handle(invocation(
            &inspection,
            &format!("queue-{index}"),
            expected.clone(),
        ));
    }
    assert_eq!(wakes.load(Ordering::Relaxed), INSPECTION_QUEUE_LIMIT as u64);
    let overflow = receipt(inspection.handle(invocation(&inspection, "overflow", expected)));
    assert_eq!(overflow.error.unwrap().code, InspectionErrorCode::QueueFull);
    assert_eq!(
        lookup(&inspection, "overflow").state,
        ReceiptState::Rejected
    );
    assert_eq!(wakes.load(Ordering::Relaxed), INSPECTION_QUEUE_LIMIT as u64);
}

#[test]
fn receipt_restart_uncertainty_and_queued_only_cancellation_are_explicit() {
    let (inspection, snapshot, bindings) = fixture();
    publish(&inspection, &snapshot, &bindings);
    let queued = invocation(&inspection, "queued", target(&inspection));
    inspection.handle(queued.clone());
    let response = receipt(inspection.handle(request(
        &inspection,
        "cancel",
        InspectionOperation::Cancel {
            request_id: "queued".into(),
        },
    )));
    assert_eq!(response.state, ReceiptState::Cancelled);
    assert!(!inspection.has_pending());
    assert_eq!(
        receipt(inspection.handle(queued)).state,
        ReceiptState::Cancelled
    );
    assert_eq!(
        inspection
            .handle(request(
                &inspection,
                "cancel-again",
                InspectionOperation::Cancel {
                    request_id: "queued".into()
                }
            ))
            .error
            .unwrap()
            .code,
        InspectionErrorCode::TooLate
    );
    assert_eq!(lookup(&inspection, "missing").state, ReceiptState::Unknown);
    let restarted = Inspection::new("test");
    assert_ne!(inspection.instance(), restarted.instance());
    let old_lookup = request(
        &inspection,
        "old-lookup",
        InspectionOperation::Receipt {
            request_id: "queued".into(),
        },
    );
    assert_eq!(
        receipt(restarted.handle(old_lookup)).state,
        ReceiptState::Uncertain
    );
    let old_mutation = invocation(&inspection, "old-mutation", target(&inspection));
    assert_eq!(
        restarted.handle(old_mutation).error.unwrap().code,
        InspectionErrorCode::WrongInstance
    );
}

#[test]
fn completion_hook_excludes_discarded_passes_and_retains_whole_publication() {
    let (inspection, snapshot, bindings) = fixture();
    let context = egui::Context::default();
    inspection.install_completion_hook(&context);
    inspection.install_completion_hook(&context);
    let mut passes = 0;
    let output = context.run_ui(egui::RawInput::default(), |ui| {
        assert!(inspection.state.lock().unwrap().publication.is_none());
        let mut current = snapshot.clone();
        current.frame = passes;
        inspection
            .stage_completed(
                ui.ctx(),
                current,
                &bindings,
                CollectionMetadata::default(),
                BTreeMap::from([("pass".into(), ObservationValue::UnsignedInteger(passes))]),
            )
            .unwrap();
        if passes == 0 {
            ui.ctx().request_discard("inspection completed-pass test");
        }
        passes += 1;
    });
    output.drop_without_applying_deltas();
    assert_eq!(passes, 2);
    let state = inspection.state.lock().unwrap();
    let observation = &state.publication.as_ref().unwrap().observation;
    assert_eq!(observation.id, 1);
    assert_eq!(observation.snapshot.frame, 1);
    assert_eq!(
        observation.facts["pass"],
        ObservationValue::UnsignedInteger(1)
    );
}

#[test]
fn physical_target_checks_geometry_enabled_state_containment_and_scale() {
    let (_, snapshot, _) = fixture();
    let mut observation = CompletedObservation {
        instance: "physical-test".into(),
        application: ApplicationIdentity::default(),
        id: 1,
        snapshot,
        collection: CollectionMetadata::default(),
        facts: BTreeMap::new(),
    };
    let selector = capability_selector();
    let resolved = resolve_physical_target(&observation, &selector).unwrap();
    assert_eq!(resolved.centre, [40., 60.]);
    assert_eq!(resolved.physical_rect, rect(20., 40., 60., 80.));
    observation.snapshot.nodes[1].enabled = false;
    assert_eq!(
        resolve_physical_target(&observation, &selector)
            .unwrap_err()
            .code,
        InspectionErrorCode::Unavailable
    );
    observation.snapshot.nodes[1].enabled = true;
    for bad in [
        rect(-1., 20., 30., 40.),
        rect(10., 20., 10., 40.),
        rect(f32::NAN, 20., 30., 40.),
    ] {
        observation.snapshot.nodes[1].rect = bad;
        assert_eq!(
            resolve_physical_target(&observation, &selector)
                .unwrap_err()
                .code,
            InspectionErrorCode::ValidationFailed
        );
    }
    observation.snapshot.nodes[1].rect = rect(10., 20., 30., 40.);
    for scale in [0., -1., f32::INFINITY, f32::NAN, f32::MAX] {
        observation.snapshot.pixels_per_point = scale;
        assert_eq!(
            resolve_physical_target(&observation, &selector)
                .unwrap_err()
                .code,
            InspectionErrorCode::ValidationFailed
        );
    }
}

#[cfg(all(unix, not(target_arch = "wasm32")))]
#[test]
fn native_socket_is_private_read_without_ui_and_owned_cleanup() {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixStream;
    use std::time::Duration;
    let (inspection, snapshot, bindings) = fixture();
    publish(&inspection, &snapshot, &bindings);
    let path = std::env::temp_dir().join(format!("pi-{}.sock", inspection.instance()));
    let wakes = Arc::new(AtomicU64::new(0));
    let count = Arc::clone(&wakes);
    let host = NativeInspectionHost::start(
        inspection.clone(),
        path.clone(),
        Arc::new(move || {
            count.fetch_add(1, Ordering::Relaxed);
        }),
    )
    .unwrap();
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(
        NativeInspectionHost::start(Inspection::new("other"), path.clone(), Arc::new(|| {}))
            .is_err()
    );
    let exchange = |request: InspectionRequest| {
        let mut stream = UnixStream::connect(&path).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        writeln!(stream, "{}", serde_json::to_string(&request).unwrap()).unwrap();
        let mut reply = String::new();
        BufReader::new(stream).read_line(&mut reply).unwrap();
        serde_json::from_str::<InspectionReply>(&reply).unwrap()
    };
    assert!(matches!(
        exchange(request(
            &inspection,
            "native-observe",
            InspectionOperation::Observe
        ))
        .result,
        Some(InspectionResult::Observe { .. })
    ));
    assert_eq!(wakes.load(Ordering::Relaxed), 0);
    assert_eq!(
        receipt(exchange(invocation(
            &inspection,
            "native-invoke",
            target(&inspection)
        )))
        .state,
        ReceiptState::Queued
    );
    assert_eq!(wakes.load(Ordering::Relaxed), 1);
    inspection.drain(&bindings, |_| Ok(()));
    assert_eq!(
        receipt(exchange(request(
            &inspection,
            "native-receipt",
            InspectionOperation::Receipt {
                request_id: "native-invoke".into(),
            }
        )))
        .state,
        ReceiptState::Completed
    );
    drop(host);
    assert!(!path.exists());
}
