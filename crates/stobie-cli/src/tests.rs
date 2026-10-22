use std::collections::BTreeSet;

use stobie_client::protocol::{
    ActionScope, ApplicationIdentity, ArgumentOption, Availability, BuildIdentity,
    CollectionMetadata, SemanticUiId, TextLayoutObservation, UiNode, UiRect, UiRole, UiSnapshot,
};

use super::*;

fn parse(command: &str, arguments: &[&str]) -> Result<Args, ClientError> {
    let arguments: Vec<String> = arguments.iter().map(|&argument| argument.into()).collect();
    Args::parse(spec::find(command).expect("registered"), &arguments)
}

fn usage_message(result: Result<impl std::fmt::Debug, ClientError>) -> String {
    let error = result.expect_err("a usage error");
    assert_eq!(error.code, "usage", "{error}");
    error.message
}

fn capability(id: &str, name: &str, arguments: ArgumentContract) -> CapabilityDescription {
    CapabilityDescription {
        argument: None,
        id: SemanticUiId::new(id),
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
        target: ResolvedTarget {
            argument: None,
            id: SemanticUiId::new(id),
            capability: name.into(),
            pane: None,
            domain: None,
            meaning: String::new(),
            observation: 1,
        },
    }
}

fn converted(
    contract: ArgumentContract,
    value: &str,
) -> Result<Option<serde_json::Value>, ClientError> {
    let args = parse("invoke", &["app.action", "--value", value])?;
    argument(&args, &capability("app.control", "app.action", contract))
}

#[test]
fn flags_take_values_inline_or_next_and_rest_follows_the_separator() {
    let args = parse(
        "launch",
        &[
            "desk",
            "--bin=record-desk",
            "--debug",
            "--timeout",
            "5000",
            "--",
            "/bin/app",
            "--flag",
        ],
    )
    .unwrap();
    assert_eq!(args.positional, ["desk"]);
    assert_eq!(args.value("--bin"), Some("record-desk"));
    assert!(args.has("--debug"));
    assert_eq!(args.number::<u64>("--timeout").unwrap(), Some(5000));
    assert_eq!(args.rest, ["/bin/app", "--flag"]);
}

#[test]
fn malformed_command_lines_are_rejected_with_the_fix() {
    assert!(
        usage_message(parse("observe", &["--bogus"])).contains("`observe` does not accept --bogus")
    );
    assert!(
        usage_message(parse("observe", &["--limit"])).contains("--limit needs a value: --limit N")
    );
    assert!(
        usage_message(parse("launch", &["desk", "--debug=yes"])).contains("--debug takes no value")
    );
    assert!(
        usage_message(parse("observe", &["--limit", "1", "--limit", "2"])).contains("given twice")
    );
    // Commands that do not connect take no target or selector flags.
    assert!(
        usage_message(parse("sessions", &["--socket", "/tmp/x.sock"])).contains("does not accept")
    );
    let args = parse("observe", &["--limit", "many"]).unwrap();
    assert!(usage_message(args.number::<usize>("--limit")).contains("needs a number"));
}

#[test]
fn selectors_combine_the_positional_field_with_selector_flags() {
    let args = parse(
        "invoke",
        &[
            "record-desk.apply",
            "--role",
            "button",
            "--pane",
            "3",
            "--name",
            "Apply",
        ],
    )
    .unwrap();
    let selector = args
        .selector(Some(("capability", "record-desk.apply")))
        .unwrap();
    assert_eq!(selector.capability.as_deref(), Some("record-desk.apply"));
    assert_eq!(selector.role, Some(UiRole::Button));
    assert_eq!(selector.name.as_deref(), Some("Apply"));
    assert_eq!(selector.pane.map(|pane| pane.0), Some(3));

    let json = parse(
        "query",
        &["--selector", r#"{"role":"text_input"}"#, "--id", "a.b"],
    )
    .unwrap();
    let selector = json.selector(None).unwrap();
    assert_eq!(selector.role, Some(UiRole::TextInput));
    assert_eq!(selector.id, Some(SemanticUiId::new("a.b")));

    let role = parse("query", &["--role", "Button"]).unwrap();
    assert!(usage_message(role.selector(None)).contains("roles are snake_case"));
    let pane = parse("query", &["--pane", "left"]).unwrap();
    assert!(usage_message(pane.selector(None)).contains("--pane needs a number"));
    let unknown = parse("query", &["--selector", r#"{"label":"Apply"}"#]).unwrap();
    assert!(usage_message(unknown.selector(None)).contains("fields are id, role"));
}

#[test]
fn values_are_converted_by_the_argument_contract() {
    let wrapped = |value: serde_json::Value| Some(serde_json::json!({ "value": value }));
    assert_eq!(
        converted(ArgumentContract::Text { max_length: None }, "true").unwrap(),
        wrapped("true".into())
    );
    assert_eq!(
        converted(ArgumentContract::Boolean, "false").unwrap(),
        wrapped(false.into())
    );
    assert!(usage_message(converted(ArgumentContract::Boolean, "yes")).contains("true or false"));
    let number = ArgumentContract::Number {
        minimum: 0.,
        maximum: 100.,
        integer: false,
    };
    assert_eq!(converted(number.clone(), "42").unwrap(), wrapped(42.into()));
    assert_eq!(
        converted(number.clone(), "0.5").unwrap(),
        wrapped(0.5.into())
    );
    assert!(usage_message(converted(number, "lots")).contains("takes a number"));
    let choice = ArgumentContract::Choice {
        options: vec![ArgumentOption {
            id: "ideas".into(),
            label: "Ideas".into(),
        }],
    };
    // A choice is sent by ID; a label or ID in any case names its option, and
    // anything else is sent as given for the application to reject.
    assert_eq!(
        converted(choice.clone(), "Ideas").unwrap(),
        wrapped("ideas".into())
    );
    assert_eq!(
        converted(choice.clone(), "IDEAS").unwrap(),
        wrapped("ideas".into())
    );
    assert_eq!(
        converted(choice.clone(), "archive").unwrap(),
        wrapped("archive".into())
    );
    assert!(usage_message(converted(ArgumentContract::None, "1")).contains("takes no value"));

    // A value-taking action without --value says what it takes.
    let args = parse("invoke", &["app.action"]).unwrap();
    let message = usage_message(argument(
        &args,
        &capability("app.control", "app.action", choice),
    ));
    assert!(
        message.contains("needs --value (choice ideas)"),
        "{message}"
    );
    assert_eq!(
        argument(
            &args,
            &capability("app.control", "app.action", ArgumentContract::None)
        )
        .unwrap(),
        None
    );

    // --value-json passes any JSON through for the application to check.
    let raw = parse("invoke", &["app.action", "--value-json", "[1]"]).unwrap();
    assert_eq!(
        argument(
            &raw,
            &capability("app.control", "app.action", ArgumentContract::Boolean)
        )
        .unwrap(),
        wrapped(serde_json::json!([1]))
    );
    let both = parse(
        "invoke",
        &["app.action", "--value", "1", "--value-json", "1"],
    )
    .unwrap();
    assert!(usage_message(json_value(&both)).contains("not both"));
}

#[test]
fn wait_conditions_are_built_from_flags_or_json() {
    let fact =
        condition(&parse("wait", &["fact", "--key", "undo_entries", "--value", "1"]).unwrap());
    assert_eq!(
        fact.unwrap(),
        Condition::Fact {
            key: "undo_entries".into(),
            value: serde_json::json!(1)
        }
    );
    let text = condition(&parse("wait", &["fact", "--key", "status", "--value", "Saved"]).unwrap());
    assert_eq!(
        text.unwrap(),
        Condition::Fact {
            key: "status".into(),
            value: serde_json::json!("Saved")
        }
    );
    let enabled =
        condition(&parse("wait", &["enabled", "--capability", "app.apply"]).unwrap()).unwrap();
    let Condition::Enabled { selector } = enabled else {
        panic!("enabled: {enabled:?}")
    };
    assert_eq!(selector.capability.as_deref(), Some("app.apply"));
    // A numeric-looking value stays text for a text condition.
    let value =
        condition(&parse("wait", &["value", "--id", "a.b", "--value", "12"]).unwrap()).unwrap();
    assert!(matches!(value, Condition::Value { value, .. } if value == "12"));
    let content =
        condition(&parse("wait", &["content", "--value", "Today's pick"]).unwrap()).unwrap();
    assert_eq!(
        content,
        Condition::Content {
            value: "Today's pick".into()
        }
    );
    assert!(
        usage_message(condition(
            &parse("wait", &["checked", "--id", "a", "--value", "yes"]).unwrap()
        ))
        .contains("true or false")
    );
    assert!(
        usage_message(condition(&parse("wait", &["glowing"]).unwrap()))
            .contains("conditions are present")
    );
    assert!(
        usage_message(condition(&parse("wait", &[]).unwrap())).contains("wait needs a condition")
    );
    // Misspelt fields fail rather than being ignored.
    let misspelt = parse(
        "wait",
        &["--condition", r#"{"condition":"fact","key":"k","vaule":1}"#],
    )
    .unwrap();
    assert!(usage_message(condition(&misspelt)).contains("unknown field"));
    for name in Condition::NAMES {
        let error =
            Condition::parse(serde_json::json!({ "condition": name, "bogus": 1 })).unwrap_err();
        assert!(
            error.message.contains("unknown field") || error.message.contains("missing field"),
            "{name}: {error}"
        );
    }
}

#[test]
fn every_error_class_has_its_documented_exit_status() {
    let documented: BTreeSet<i32> = spec::EXIT_CODES.iter().map(|(code, ..)| *code).collect();
    assert_eq!(documented, (0..=5).collect());
    for (code, expected) in [
        ("usage", 2),
        ("invalid_request", 2),
        ("connection_failed", 3),
        ("wrong_instance", 3),
        ("launch_failed", 3),
        ("timeout", 4),
        ("uncertain_completion", 4),
        ("partial", 5),
        ("unavailable", 1),
        ("ambiguous", 1),
        ("not_found", 1),
    ] {
        assert_eq!(status(&ClientError::new(code, "", "")), expected, "{code}");
    }
    let receipt = |state| InvocationReceipt {
        request_id: "r".into(),
        state,
        route: None,
        before_observation: None,
        after_observation: None,
        target: None,
        error: None,
    };
    assert_eq!(receipt_status(&receipt(ReceiptState::Completed)), 0);
    assert_eq!(receipt_status(&receipt(ReceiptState::Rejected)), 1);
    assert_eq!(receipt_status(&receipt(ReceiptState::Uncertain)), 4);
}

#[test]
fn the_registry_is_consistent_with_parsing_and_help() {
    let shared: BTreeSet<&str> = spec::TARGET_FLAGS
        .iter()
        .chain(spec::SELECTOR_FLAGS)
        .map(|flag| flag.name)
        .collect();
    for command in spec::COMMANDS {
        let own: Vec<&str> = command.flags.iter().map(|flag| flag.name).collect();
        let unique: BTreeSet<&str> = own.iter().copied().collect();
        assert_eq!(unique.len(), own.len(), "{} repeats a flag", command.name);
        if command.connects {
            assert!(
                unique.is_disjoint(&shared),
                "{} redefines a shared flag",
                command.name
            );
        }
        assert!(
            command
                .usage
                .starts_with(&format!("stobie {}", command.name)),
            "{}",
            command.name
        );
        assert!(
            spec::help().contains(command.name),
            "help omits {}",
            command.name
        );
        assert!(spec::command_help(command).contains(command.summary));
        for flag in command.flags {
            assert!(
                spec::command_help(command).contains(flag.name),
                "{} help omits {}",
                command.name,
                flag.name
            );
        }
    }
    let described = capabilities();
    let names: Vec<&str> = described["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|command| command["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        spec::COMMANDS
            .iter()
            .map(|command| command.name)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        described["wait_conditions"].as_array().unwrap().len(),
        Condition::NAMES.len()
    );
}

fn node(id: &str, parent: Option<&str>, role: UiRole, name: &str) -> UiNode {
    let mut node = UiNode::container(
        SemanticUiId::new(id),
        parent.map(SemanticUiId::new),
        role,
        UiRect {
            min_x: 0.,
            min_y: 0.,
            max_x: 10.,
            max_y: 10.,
        },
    );
    node.name = name.into();
    node
}

fn observation(nodes: Vec<UiNode>, facts: &[(&str, ObservationValue)]) -> CompletedObservation {
    CompletedObservation {
        instance: "instance-1".into(),
        application: ApplicationIdentity {
            name: "Desk".into(),
            build: BuildIdentity {
                version: "1.0".into(),
                source_revision: None,
            },
        },
        id: 7,
        snapshot: UiSnapshot {
            frame: 7,
            pixels_per_point: 1.,
            root: SemanticUiId::new("root"),
            nodes,
            text: Vec::new(),
            text_audit: Vec::new(),
            text_audit_coverage: None,
            semantic_audit: Vec::new(),
        },
        collection: CollectionMetadata::default(),
        facts: facts
            .iter()
            .map(|(key, value)| ((*key).into(), value.clone()))
            .collect(),
    }
}

fn text(kind: &str, instance: u64, content: Option<serde_json::Value>) -> TextLayoutObservation {
    let rect = serde_json::json!({ "min_x": 0., "min_y": 0., "max_x": 10., "max_y": 10. });
    let mut text = serde_json::json!({
        "component_id": { "kind": kind, "instance": instance },
        "parent_id": null,
        "role": "body",
        "horizontal_alignment": "start",
        "vertical_alignment": "top",
        "allocated_rect": rect,
        "painted_rect": rect,
        "clip_rect": rect,
        "baseline": null,
        "overflow": "wrap",
        "declared_max_lines": 2,
        "line_count": 1,
        "truncated": false,
    });
    if let Some(content) = content {
        text["content"] = content;
    }
    serde_json::from_value(text).unwrap()
}

#[test]
fn observe_shows_presented_content_only_when_asked() {
    let mut observed = observation(vec![node("root", None, UiRole::Application, "")], &[]);
    observed.snapshot.text = vec![
        text(
            "section_heading",
            1,
            Some(serde_json::json!({ "kind": "excerpt", "text": "Inbox", "truncated": false })),
        ),
        text(
            "reading_text",
            2,
            Some(serde_json::json!({
                "kind": "excerpt",
                "text": "First line\nsecond line…",
                "truncated": true
            })),
        ),
        text(
            "content_label",
            3,
            Some(serde_json::json!({ "kind": "withheld" })),
        ),
        text("action_button", 4, None),
    ];
    let content = output::content_outline(&observed);
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(
        lines,
        [
            "content: 3 texts",
            r#"- section_heading "Inbox""#,
            r#"- reading_text "First line⏎second line…""#,
            "- content_label withheld",
        ]
    );
    // The plain outline and JSON leave content out and say so.
    assert!(!output::outline(&observed, &[], 0, 64).contains("Inbox"));
    let plain =
        |extras| serde_json::to_value(output::observe_json(&observed, &[], 0, 64, extras)).unwrap();
    let json = plain(output::ObserveExtras::default());
    assert!(json.get("content").is_none());
    assert!(
        json["omitted"]
            .as_array()
            .unwrap()
            .contains(&"content".into())
    );
    // Measurements alone carry no words.
    let measured = plain(output::ObserveExtras {
        text: true,
        content: false,
    });
    assert!(!measured.to_string().contains("Inbox"), "{measured}");
    assert_eq!(measured["text_observations"].as_array().unwrap().len(), 4);
    let json = plain(output::ObserveExtras {
        text: false,
        content: true,
    });
    assert!(
        !json["omitted"]
            .as_array()
            .unwrap()
            .contains(&"content".into())
    );
    assert_eq!(
        json["content"],
        serde_json::json!([
            { "component": "section_heading", "role": "body",
              "content": { "kind": "excerpt", "text": "Inbox", "truncated": false } },
            { "component": "reading_text", "role": "body",
              "content": { "kind": "excerpt", "text": "First line\nsecond line…", "truncated": true } },
            { "component": "content_label", "role": "body", "content": { "kind": "withheld" } },
        ])
    );
    let both = plain(output::ObserveExtras {
        text: true,
        content: true,
    });
    assert_eq!(both["text_observations"][0]["content"]["text"], "Inbox");
}

#[test]
fn the_outline_shows_state_values_ids_and_capabilities_compactly() {
    let mut notes = node("desk.notes", Some("desk.pane"), UiRole::TextInput, "Notes");
    notes.value = Some(format!("first line\n{}", "x".repeat(100)));
    let mut apply = node("desk.apply", Some("desk.pane"), UiRole::Button, "Apply");
    apply.enabled = false;
    apply.disabled_reason = Some("Nothing to apply".into());
    let mut reviewed = node(
        "desk.reviewed",
        Some("desk.pane"),
        UiRole::Button,
        "Reviewed",
    );
    reviewed.checked = Some(true);
    let observed = observation(
        vec![
            node("root", None, UiRole::Application, ""),
            node("desk.pane", Some("root"), UiRole::Pane, "Editor"),
            notes,
            apply,
            reviewed,
        ],
        &[
            ("undo_entries", ObservationValue::UnsignedInteger(2)),
            ("status", ObservationValue::Text("Saved".into())),
        ],
    );
    let mut disabled = capability("desk.apply", "desk.apply", ArgumentContract::None);
    disabled.availability = Availability::Disabled {
        reason: "Nothing to apply".into(),
    };
    let capabilities = [
        capability(
            "desk.notes",
            SET_TEXT,
            ArgumentContract::Text { max_length: None },
        ),
        disabled,
        capability(
            "desk.category",
            "desk.edit-category",
            ArgumentContract::Choice {
                options: ["ideas", "operations"]
                    .map(|id| ArgumentOption {
                        id: id.into(),
                        label: id.into(),
                    })
                    .to_vec(),
            },
        ),
    ];
    let text = output::outline(&observed, &capabilities, 0, 64);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "Desk 1.0 · observation 7 · instance instance-1");
    assert_eq!(lines[1], r#"facts: status="Saved" undo_entries=2"#);
    assert!(text.contains("audits: clear"));
    let notes_line = lines
        .iter()
        .find(|line| line.contains("#desk.notes"))
        .unwrap();
    assert!(notes_line.contains(r#"text_input "Notes""#), "{notes_line}");
    assert!(
        notes_line.contains("first line⏎x") && notes_line.contains("…"),
        "{notes_line}"
    );
    assert!(notes_line.ends_with("[set-text]"), "{notes_line}");
    let apply_line = lines
        .iter()
        .find(|line| line.contains("#desk.apply"))
        .unwrap();
    assert!(
        apply_line.contains("disabled(Nothing to apply)"),
        "{apply_line}"
    );
    assert!(
        apply_line.contains("[desk.apply (disabled: Nothing to apply)]"),
        "{apply_line}"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.contains(r#"button "Reviewed" checked"#))
    );
    // The root is implied; children are indented beneath their pane.
    assert!(!text.contains("#root"));
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with(r#"- pane "Editor""#))
    );
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("  - button \"Apply\""))
    );
    // A capability without a rendered control is still listed.
    assert_eq!(
        *lines.last().unwrap(),
        "other capabilities: desk.edit-category: choice ideas|operations"
    );

    let paged = output::outline(&observed, &capabilities, 1, 2);
    assert!(
        paged.contains("… 1 more nodes; continue with --offset 3"),
        "{paged}"
    );
    assert!(
        !paged.contains("#desk.pane") && paged.contains("#desk.apply"),
        "{paged}"
    );
}

#[test]
fn changed_facts_report_additions_removals_and_changes() {
    let before = observation(
        Vec::new(),
        &[
            ("same", ObservationValue::Boolean(true)),
            ("undo_entries", ObservationValue::UnsignedInteger(0)),
            ("gone", ObservationValue::Boolean(true)),
        ],
    );
    let after = observation(
        Vec::new(),
        &[
            ("same", ObservationValue::Boolean(true)),
            ("undo_entries", ObservationValue::UnsignedInteger(1)),
            ("new", ObservationValue::Boolean(false)),
        ],
    );
    let changed = changed_facts(&before, &after);
    assert_eq!(
        changed.keys().copied().collect::<Vec<_>>(),
        ["gone", "new", "undo_entries"]
    );
    assert_eq!(changed["gone"].1, None);
    assert_eq!(changed["new"].0, None);
}

#[test]
fn cursors_round_trip_through_their_printed_token() {
    let cursor = stobie_client::protocol::InspectionCursor {
        observation: 12,
        offset: 64,
    };
    let args = parse(
        "query",
        &["--cursor", &cursor_token(cursor), "--limit", "5"],
    )
    .unwrap();
    let page = page(&args).unwrap();
    assert_eq!(page.cursor, Some(cursor));
    assert_eq!(page.limit, 5);
    let bad = parse("query", &["--cursor", "twelve"]).unwrap();
    assert!(usage_message(super::page(&bad)).contains("--cursor needs a cursor"));
}

const SET_TEXT: &str = stobie_client::protocol::SET_TEXT_CAPABILITY;

#[test]
fn the_envelope_leads_and_the_payload_keeps_its_field_order() {
    #[derive(serde::Serialize)]
    struct Payload {
        zeta: u8,
        alpha: u8,
    }
    let payload = serde_json::to_string(&Payload { zeta: 1, alpha: 2 }).unwrap();
    assert_eq!(
        envelope("observe", true, &payload),
        r#"{"schema_version":1,"command":"observe","ok":true,"zeta":1,"alpha":2}"#
    );
    let empty = serde_json::to_string(&serde_json::json!({})).unwrap();
    assert_eq!(
        envelope("stop", false, &empty),
        r#"{"schema_version":1,"command":"stop","ok":false}"#
    );
}

#[test]
fn a_rows_control_is_selected_by_the_argument_it_carries() {
    let selector = |value: &str| {
        parse("invoke", &["people.open", "--argument", value])
            .unwrap()
            .selector(Some(("capability", "people.open")))
            .unwrap()
            .argument
    };
    assert_eq!(selector("42"), Some(serde_json::json!(42)));
    assert_eq!(selector("true"), Some(serde_json::json!(true)));
    assert_eq!(selector("\"42\""), Some(serde_json::json!("42")));
    assert_eq!(selector("ada"), Some(serde_json::json!("ada")));
    assert_eq!(selector("[1]"), Some(serde_json::json!("[1]")));
    let json = parse("query", &["--selector", r#"{"argument":7}"#])
        .unwrap()
        .selector(None)
        .unwrap();
    assert_eq!(json.argument, Some(serde_json::json!(7)));

    // A row's control carries its value, so it needs none and shows it.
    let mut row = capability(
        "people.open.2",
        "people.open",
        ArgumentContract::Number {
            minimum: 1.0,
            maximum: 9.0,
            integer: true,
        },
    );
    let args = parse("invoke", &["people.open"]).unwrap();
    assert!(
        argument(&args, &row).is_err(),
        "the action's binding needs a value"
    );
    row.argument = Some(serde_json::json!(2));
    assert_eq!(argument(&args, &row).unwrap(), None);
    assert_eq!(output::capability_tag(&row), "people.open = 2");
    let mut node = UiNode::container(
        SemanticUiId::new("people.open.2"),
        None,
        UiRole::Button,
        UiRect::default(),
    );
    node.name = "Open Grace".into();
    node.argument = Some(serde_json::json!(2));
    assert_eq!(
        node_text(&node, &[]),
        "button \"Open Grace\" carrying 2 #people.open.2"
    );
}

#[test]
fn reveal_takes_one_id_and_is_tagged_without_its_contract() {
    let args = parse("reveal", &["desk.notes", "--request-id", "r1"]).unwrap();
    assert_eq!(args.positional, ["desk.notes"]);
    assert_eq!(args.value("--request-id"), Some("r1"));
    let mut reveal = capability(
        stobie_client::protocol::REVEAL_CAPABILITY,
        stobie_client::protocol::REVEAL_CAPABILITY,
        ArgumentContract::Text {
            max_length: Some(512),
        },
    );
    reveal.rendered = false;
    assert_eq!(output::capability_tag(&reveal), "reveal");
}
