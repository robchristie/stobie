//! `stobie`: launch or attach to a Stobie application, read its UI compactly
//! and operate it. See `stobie --help` and `stobie capabilities --json`.

mod args;
mod capture;
mod output;
mod session;
mod spec;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{Read as _, Write as _};
use std::process::ExitCode;
use std::time::Duration;

use serde::Serialize;
use stobie_client::protocol::{
    ArgumentContract, CapabilityDescription, CompletedObservation, InvocationReceipt,
    ObservationValue, ReceiptState, ResolvedTarget,
};
use stobie_client::{
    BrowserTransport, Client, ClientError, Condition, ErrorClass, InvokeOptions, Invoked, Page,
    PageSelector, SocketTransport, Transport,
};

use args::Args;
use session::{Session, Target};
use spec::SCHEMA_VERSION;

/// What a command prints: text, and the JSON payload after the envelope's
/// `schema_version`, `command` and `ok`.
struct Output {
    text: String,
    /// Serialised once, so a struct payload keeps its field order.
    json: String,
    /// Exit status for a successful exchange whose outcome is not success,
    /// such as a rejected invocation.
    status: u8,
}

impl Output {
    fn new(text: String, json: impl Serialize) -> Self {
        Self {
            text,
            json: serde_json::to_string(&json).expect("outputs serialise"),
            status: 0,
        }
    }
}

#[derive(Serialize)]
struct EnvelopeHead<'a> {
    schema_version: u32,
    command: &'a str,
    ok: bool,
}

/// One JSON object: the envelope's fields, then the payload object's fields
/// in the order the payload serialised them.
fn envelope(command: &str, ok: bool, payload: &str) -> String {
    let head = serde_json::to_string(&EnvelopeHead {
        schema_version: SCHEMA_VERSION,
        command,
        ok,
    })
    .expect("envelopes serialise");
    let body = payload.trim();
    let fields = body
        .strip_prefix('{')
        .and_then(|body| body.strip_suffix('}'))
        .expect("payloads are objects")
        .trim();
    let head = head.strip_suffix('}').expect("an object");
    if fields.is_empty() {
        format!("{head}}}")
    } else {
        format!("{head},{fields}}}")
    }
}

/// Write to standard output, ignoring a reader that has gone away, such as
/// `stobie observe | head`.
fn emit(text: &str) {
    use std::io::Write as _;
    let mut stdout = std::io::stdout().lock();
    let _ = stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush());
}

#[derive(Serialize)]
struct ErrorEnvelope<'a> {
    schema_version: u32,
    command: &'a str,
    ok: bool,
    error: ErrorBody<'a>,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
    hint: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<&'a str>,
    /// The latest observation a failed wait saw.
    #[serde(skip_serializing_if = "Option::is_none")]
    observation: Option<u64>,
}

fn status(error: &ClientError) -> u8 {
    match (error.code.as_str(), error.class()) {
        ("partial", _) => 5,
        ("launch_failed" | "session_failed", _) => 3,
        (_, ErrorClass::Application) => 1,
        (_, ErrorClass::Usage) => 2,
        (_, ErrorClass::Connection) => 3,
        (_, ErrorClass::Deadline) => 4,
    }
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.first().map(String::as_str) == Some("__supervise") {
        return match arguments.as_slice() {
            [_, name, directory, program, rest @ ..] => {
                ExitCode::from(
                    session::supervise(name, directory.as_ref(), program.as_ref(), rest) as u8,
                )
            }
            _ => ExitCode::from(2),
        };
    }
    let Some(name) = arguments.first().cloned() else {
        print!("{}", spec::help());
        return ExitCode::from(2);
    };
    if matches!(name.as_str(), "--help" | "-h" | "help") {
        print!("{}", spec::help());
        return ExitCode::SUCCESS;
    }
    if name == "--version" {
        println!("stobie {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    // A program's own arguments after `--` are not stobie's.
    let json = arguments
        .iter()
        .take_while(|argument| *argument != "--")
        .any(|argument| argument == "--json");
    let Some(command) = spec::find(&name) else {
        report(
            &name,
            json,
            &ClientError::usage(format!("unknown command {name:?}; see `stobie --help`")),
        );
        return ExitCode::from(2);
    };
    if arguments[1..]
        .iter()
        .take_while(|argument| *argument != "--")
        .any(|argument| argument == "--help" || argument == "-h")
    {
        print!("{}", spec::command_help(command));
        return ExitCode::SUCCESS;
    }
    let result = Args::parse(command, &arguments[1..]).and_then(|args| run(command, &args));
    match result {
        Ok(output) => {
            if json || command.name == "capabilities" {
                let mut text = envelope(command.name, output.status == 0, &output.json);
                // The command registry is read by people as well as parsed.
                if command.name == "capabilities" {
                    let value: serde_json::Value =
                        serde_json::from_str(&text).expect("envelopes parse");
                    text = serde_json::to_string_pretty(&value).expect("values serialise");
                }
                emit(&format!("{text}\n"));
            } else {
                emit(&output.text);
            }
            ExitCode::from(output.status)
        }
        Err(error) => {
            report(command.name, json, &error);
            ExitCode::from(status(&error))
        }
    }
}

fn report(command: &str, json: bool, error: &ClientError) {
    if json {
        let envelope = ErrorEnvelope {
            schema_version: SCHEMA_VERSION,
            command,
            ok: false,
            error: ErrorBody {
                code: &error.code,
                message: &error.message,
                hint: &error.hint,
                request_id: error.request_id.as_deref(),
                observation: error.observation.as_ref().map(|observation| observation.id),
            },
        };
        eprintln!(
            "{}",
            serde_json::to_string(&envelope).expect("errors serialise")
        );
    } else {
        eprintln!("error: {}: {}", error.code, error.message);
        if let Some(request_id) = &error.request_id {
            eprintln!("request: {request_id}");
        }
        eprintln!("next: {}", error.hint);
    }
}

fn timeout(args: &Args, default: Duration) -> Result<Duration, ClientError> {
    Ok(args
        .number::<u64>("--timeout")?
        .map_or(default, Duration::from_millis))
}

type AnyClient = Client<Box<dyn Transport>>;

/// A browser page selector from `--page` or `--target`.
pub(crate) fn page_selector(
    page: Option<&str>,
    target: Option<&str>,
) -> Result<PageSelector, ClientError> {
    match (page, target) {
        (Some(_), Some(_)) => Err(ClientError::usage("give --page or --target, not both")),
        (Some(url), None) => Ok(PageSelector::Url(url.into())),
        (None, Some(id)) => Ok(PageSelector::Target(id.into())),
        (None, None) => Ok(PageSelector::Only),
    }
}

/// Where a command's application is: `--socket`, `--cdp`, or a session.
fn target(args: &Args) -> Result<(Target, Option<Session>), ClientError> {
    match (args.value("--socket"), args.value("--cdp")) {
        (Some(_), Some(_)) => Err(ClientError::usage("give --socket or --cdp, not both")),
        (Some(socket), None) => Ok((
            Target::Native {
                socket: socket.into(),
                pid: None,
                directory: None,
            },
            None,
        )),
        (None, Some(endpoint)) => Ok((
            Target::Browser {
                endpoint: endpoint.into(),
                page: args.value("--page").map(ToOwned::to_owned),
                target: args.value("--target").map(ToOwned::to_owned),
            },
            None,
        )),
        (None, None) => {
            let session = session::resolve(args.value("--session"))?;
            Ok((session.target.clone(), Some(session)))
        }
    }
}

fn transport(target: &Target) -> Result<Box<dyn Transport>, ClientError> {
    Ok(match target {
        Target::Native { socket, .. } => Box::new(SocketTransport::new(socket.clone())),
        Target::Browser {
            endpoint,
            page,
            target,
        } => Box::new(BrowserTransport::new(
            endpoint,
            page_selector(page.as_deref(), target.as_deref())?,
        )?),
    })
}

/// The application a command addresses, bound with `hello`.
fn connect(args: &Args) -> Result<(AnyClient, Target, Option<Session>), ClientError> {
    let (target, session) = target(args)?;
    let mut client = Client::new(transport(&target)?)
        .with_timeout(timeout(args, stobie_client::DEFAULT_TIMEOUT)?);
    client.hello()?;
    Ok((client, target, session))
}

/// Every capability, across pages.
fn all_capabilities(client: &mut AnyClient) -> Result<Vec<CapabilityDescription>, ClientError> {
    let mut capabilities = Vec::new();
    let mut cursor = None;
    loop {
        let page = client.discover(Default::default(), Page { limit: 256, cursor })?;
        capabilities.extend(page.capabilities);
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => return Ok(capabilities),
        }
    }
}

fn run(command: &spec::Command, args: &Args) -> Result<Output, ClientError> {
    let positional = |index: usize, what: &str| {
        args.positional
            .get(index)
            .map(String::as_str)
            .ok_or_else(|| {
                ClientError::usage(format!(
                    "{} needs {what}; usage: {}",
                    command.name, command.usage
                ))
            })
    };
    if args.positional.len() > expected_positionals(command.name) {
        return Err(ClientError::usage(format!(
            "unexpected argument {:?}; usage: {}",
            args.positional[expected_positionals(command.name)],
            command.usage
        )));
    }
    match command.name {
        "capabilities" => Ok(Output::new(String::new(), capabilities())),
        "sessions" => sessions(),
        "launch" => launch(args, positional(0, "a session NAME")?),
        "stop" => {
            let name = positional(0, "a session NAME")?;
            let stopped = session::stop(name, Duration::from_secs(10))?;
            let text = match &stopped.target {
                Target::Browser { .. } => format!("detached {name}; the page keeps running\n"),
                Target::Native { .. } => format!("stopped {name}\n"),
            };
            Ok(Output::new(text, serde_json::json!({ "session": stopped })))
        }
        "hello" => {
            let (mut client, _, _) = connect(args)?;
            let hello = client.hello()?;
            let text = format!(
                "{} {} · instance {} · operations {}\n",
                hello.application.name,
                hello.application.build.version,
                hello.instance,
                hello
                    .operations
                    .iter()
                    .map(|operation| serde_json::to_value(operation)
                        .ok()
                        .and_then(|value| value.as_str().map(ToOwned::to_owned))
                        .unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            Ok(Output::new(text, hello))
        }
        "observe" => {
            let (mut client, _, _) = connect(args)?;
            let observation = client.observe()?;
            let capabilities = all_capabilities(&mut client)?;
            let offset = args.number::<usize>("--offset")?.unwrap_or(0);
            let limit = args.number::<usize>("--limit")?.unwrap_or(200);
            Ok(Output::new(
                output::outline(&observation, &capabilities, offset, limit),
                output::observe_json(
                    &observation,
                    &capabilities,
                    offset,
                    limit,
                    args.has("--include-text"),
                ),
            ))
        }
        "query" => {
            let (mut client, _, _) = connect(args)?;
            let selector = args.selector(args.positional.first().map(|id| ("id", id.as_str())))?;
            let page = client.query(selector, page(args)?)?;
            let capabilities = all_capabilities(&mut client)?;
            let mut text = String::new();
            for observed in &page.nodes {
                let tags: Vec<String> = capabilities
                    .iter()
                    .filter(|capability| capability.id == observed.node.id)
                    .map(output::capability_tag)
                    .collect();
                let _ = writeln!(text, "- {}", node_text(&observed.node, &tags));
            }
            let _ = writeln!(
                text,
                "{} of {} nodes · observation {}{}",
                page.nodes.len(),
                page.total,
                page.observation,
                cursor_text(page.next_cursor)
            );
            let nodes: Vec<output::NodeOutput> = page
                .nodes
                .iter()
                .map(|observed| output::NodeOutput::new(&observed.node, &capabilities))
                .collect();
            Ok(Output::new(
                text,
                serde_json::json!({
                    "observation": page.observation,
                    "total": page.total,
                    "nodes": nodes,
                    "truncated": page.next_cursor.is_some(),
                    "next_cursor": page.next_cursor.map(cursor_token),
                    "omitted": ["geometry"],
                }),
            ))
        }
        "discover" => {
            let (mut client, _, _) = connect(args)?;
            let selector = args.selector(
                args.positional
                    .first()
                    .map(|capability| ("capability", capability.as_str())),
            )?;
            let page = client.discover(selector, page(args)?)?;
            let mut text = String::new();
            for capability in &page.capabilities {
                let mut line = format!("- {}", output::capability_tag(capability));
                let _ = write!(line, " · {:?}", capability.label);
                if let Some(shortcut) = capability.shortcut {
                    let _ = write!(line, " · {}", shortcut.display());
                }
                let _ = write!(
                    line,
                    " · on #{}{}",
                    capability.id.0,
                    if capability.rendered {
                        ""
                    } else {
                        " (not rendered)"
                    }
                );
                let _ = writeln!(text, "{line}");
                // What a value means is in the description, such as which ID
                // to give.
                if capability.arguments != ArgumentContract::None
                    && !capability.description.is_empty()
                {
                    let _ = writeln!(text, "  {}", output::clip(&capability.description));
                }
            }
            let _ = writeln!(
                text,
                "{} of {} capabilities · observation {}{}",
                page.capabilities.len(),
                page.total,
                page.observation,
                cursor_text(page.next_cursor)
            );
            Ok(Output::new(
                text,
                serde_json::json!({
                    "observation": page.observation,
                    "total": page.total,
                    "capabilities": page.capabilities,
                    "truncated": page.next_cursor.is_some(),
                    "next_cursor": page.next_cursor.map(cursor_token),
                }),
            ))
        }
        "invoke" => {
            let (mut client, _, _) = connect(args)?;
            let selector = args.selector(
                args.positional
                    .first()
                    .map(|capability| ("capability", capability.as_str())),
            )?;
            if selector == Default::default() {
                return Err(ClientError::usage(format!(
                    "invoke needs a capability; usage: {}",
                    command.usage
                )));
            }
            let expected = args
                .value("--expected")
                .map(|json| {
                    serde_json::from_str::<ResolvedTarget>(json).map_err(|error| {
                        ClientError::usage(format!("--expected is not a target: {error}"))
                    })
                })
                .transpose()?;
            let (expected, arguments) = match expected {
                Some(expected) => (
                    Some(expected),
                    json_value(args)?.map(|value| serde_json::json!({ "value": value })),
                ),
                None => {
                    let capability = client.discover_one(selector.clone())?;
                    let arguments = argument(args, &capability)?;
                    (Some(capability.target), arguments)
                }
            };
            let before = client.observe()?;
            let invoked = client.invoke(
                selector,
                InvokeOptions {
                    expected,
                    arguments,
                    request_id: args.value("--request-id").map(ToOwned::to_owned),
                    no_wait: args.has("--no-wait"),
                },
            )?;
            Ok(invoked_output(&before, invoked))
        }
        "set-text" => {
            let (mut client, _, _) = connect(args)?;
            let id = positional(0, "a text field ID")?;
            let text = match (args.value("--text-file"), args.positional.get(1)) {
                (Some(path), None) => std::fs::read_to_string(path)
                    .map_err(|error| ClientError::usage(format!("cannot read {path}: {error}")))?,
                (None, Some(text)) if text == "-" => {
                    let mut text = String::new();
                    std::io::stdin()
                        .read_to_string(&mut text)
                        .map_err(|error| {
                            ClientError::usage(format!("cannot read stdin: {error}"))
                        })?;
                    text
                }
                (None, Some(text)) => text.clone(),
                _ => {
                    return Err(ClientError::usage(format!(
                        "set-text needs TEXT or --text-file PATH; usage: {}",
                        command.usage
                    )));
                }
            };
            let selector = args.selector(Some(("id", id)))?;
            let before = client.observe()?;
            let invoked = client.set_text(
                selector,
                &text,
                InvokeOptions {
                    request_id: args.value("--request-id").map(ToOwned::to_owned),
                    ..InvokeOptions::default()
                },
            )?;
            Ok(invoked_output(&before, invoked))
        }
        "wait" => {
            let (mut client, _, _) = connect(args)?;
            let condition = condition(args)?;
            let waited = client.wait(&condition)?;
            let mut text = format!("holds at observation {}", waited.observation.id);
            if let Some(node) = &waited.node {
                let _ = write!(text, ": {}", node_text(node, &[]));
            }
            if let Some(capability) = &waited.capability {
                let _ = write!(text, ": {}", output::capability_tag(capability));
            }
            text.push('\n');
            Ok(Output::new(
                text,
                serde_json::json!({
                    "observation": waited.observation.id,
                    "node": waited.node.as_ref().map(|node| output::NodeOutput::new(node, &[])),
                    "capability": waited.capability,
                }),
            ))
        }
        "receipt" | "cancel" => {
            let (mut client, _, _) = connect(args)?;
            let id = positional(0, "a REQUEST_ID")?;
            let receipt = if command.name == "receipt" {
                client.receipt(id)?
            } else {
                client.cancel(id)?
            };
            let mut output = Output::new(format!("{}\n", receipt_text(&receipt)), &receipt);
            output.status = receipt_status(&receipt);
            Ok(output)
        }
        "capture" => {
            let (mut client, target, _) = connect(args)?;
            let path = args
                .value("--output")
                .ok_or_else(|| ClientError::usage("capture needs --output PNG"))?;
            capture::capture(&mut client, &target, path.as_ref())
        }
        "attach" => {
            let name = positional(0, "a session NAME")?;
            session::validate_name(name)?;
            let endpoint = args
                .value("--cdp")
                .ok_or_else(|| ClientError::usage("attach needs --cdp URL"))?;
            let target = Target::Browser {
                endpoint: endpoint.into(),
                page: args.value("--page").map(ToOwned::to_owned),
                target: args.value("--target").map(ToOwned::to_owned),
            };
            let mut client = Client::new(transport(&target)?)
                .with_timeout(timeout(args, stobie_client::DEFAULT_TIMEOUT)?);
            let hello = client.hello()?;
            let session = Session {
                name: name.into(),
                target,
            };
            session::save(&session)?;
            Ok(Output::new(
                format!(
                    "attached {name}: {} {} · {}\nnext: stobie observe --session {name}\n",
                    hello.application.name,
                    hello.application.build.version,
                    client.transport().describe()
                ),
                serde_json::json!({ "session": session, "application": hello.application }),
            ))
        }
        _ => unreachable!("every registered command runs"),
    }
}

fn expected_positionals(command: &str) -> usize {
    match command {
        "launch" | "attach" | "stop" | "query" | "discover" | "invoke" | "wait" | "receipt"
        | "cancel" => 1,
        "set-text" => 2,
        _ => 0,
    }
}

fn page(args: &Args) -> Result<Page, ClientError> {
    Ok(Page {
        limit: args.number::<usize>("--limit")?.unwrap_or(64),
        cursor: args
            .value("--cursor")
            .map(|token| {
                token
                    .split_once(':')
                    .and_then(|(observation, offset)| {
                        Some(stobie_client::protocol::InspectionCursor {
                            observation: observation.parse().ok()?,
                            offset: offset.parse().ok()?,
                        })
                    })
                    .ok_or_else(|| {
                        ClientError::usage("--cursor needs a cursor printed by a previous page")
                    })
            })
            .transpose()?,
    })
}

fn cursor_token(cursor: stobie_client::protocol::InspectionCursor) -> String {
    format!("{}:{}", cursor.observation, cursor.offset)
}

fn cursor_text(cursor: Option<stobie_client::protocol::InspectionCursor>) -> String {
    cursor.map_or_else(String::new, |cursor| {
        format!(" · more with --cursor {}", cursor_token(cursor))
    })
}

fn node_text(node: &stobie_client::protocol::UiNode, tags: &[String]) -> String {
    let mut text = output::role_name(node.role);
    if !node.name.is_empty() {
        let _ = write!(text, " {:?}", output::clip(&node.name));
    }
    if !node.enabled {
        text.push_str(" disabled");
    }
    if node.focused {
        text.push_str(" focused");
    }
    if node.selected {
        text.push_str(" selected");
    }
    if let Some(checked) = node.checked {
        text.push_str(if checked { " checked" } else { " unchecked" });
    }
    if let Some(value) = &node.value {
        let _ = write!(text, " = {:?}", output::clip(value));
    }
    let _ = write!(text, " #{}", node.id.0);
    if !tags.is_empty() {
        let _ = write!(text, " [{}]", tags.join("; "));
    }
    text
}

/// `--value-json`, or `--value` as JSON text when it parses, as a raw value.
fn json_value(args: &Args) -> Result<Option<serde_json::Value>, ClientError> {
    match (args.value("--value"), args.value("--value-json")) {
        (Some(_), Some(_)) => Err(ClientError::usage("give --value or --value-json, not both")),
        (None, Some(json)) => serde_json::from_str(json)
            .map(Some)
            .map_err(|error| ClientError::usage(format!("--value-json is not JSON: {error}"))),
        (Some(value), None) => Ok(Some(serde_json::Value::String(value.to_owned()))),
        (None, None) => Ok(None),
    }
}

/// The `arguments` object for a capability, converting `--value` by its
/// contract.
fn argument(
    args: &Args,
    capability: &CapabilityDescription,
) -> Result<Option<serde_json::Value>, ClientError> {
    let value = match (args.value("--value"), args.value("--value-json")) {
        (_, Some(_)) => json_value(args)?,
        (Some(value), None) => Some(match &capability.arguments {
            ArgumentContract::None => {
                return Err(ClientError::usage(format!(
                    "{} takes no value",
                    capability.capability
                )));
            }
            ArgumentContract::Text { .. } => serde_json::Value::String(value.to_owned()),
            // A choice is sent by ID; an option's label, in any case, names
            // its ID when no other option matches it. Anything else is sent as
            // given for the application to reject with the valid IDs.
            ArgumentContract::Choice { options } => {
                let matching: Vec<_> = options
                    .iter()
                    .filter(|option| {
                        option.id.eq_ignore_ascii_case(value)
                            || option.label.to_lowercase() == value.to_lowercase()
                    })
                    .collect();
                let id = match (
                    options.iter().find(|option| option.id == value),
                    matching.as_slice(),
                ) {
                    (Some(exact), _) => exact.id.as_str(),
                    (None, [only]) => only.id.as_str(),
                    _ => value,
                };
                serde_json::Value::String(id.to_owned())
            }
            ArgumentContract::Boolean => serde_json::Value::Bool(match value {
                "true" => true,
                "false" => false,
                _ => {
                    return Err(ClientError::usage(format!(
                        "{} takes true or false, not {value:?}",
                        capability.capability
                    )));
                }
            }),
            ArgumentContract::Number { .. } => {
                let number: f64 = value.parse().map_err(|_| {
                    ClientError::usage(format!(
                        "{} takes a number, not {value:?}",
                        capability.capability
                    ))
                })?;
                if number.fract() == 0.0 && number.abs() < 9.0e15 {
                    serde_json::json!(number as i64)
                } else {
                    serde_json::json!(number)
                }
            }
        }),
        (None, None) => {
            if let Some(summary) = output::argument_summary(&capability.arguments) {
                let mut message = format!("{} needs --value ({summary})", capability.capability);
                if !capability.description.is_empty() {
                    let _ = write!(message, ": {}", capability.description);
                }
                return Err(ClientError::usage(message));
            }
            None
        }
    };
    Ok(value.map(|value| serde_json::json!({ "value": value })))
}

fn condition(args: &Args) -> Result<Condition, ClientError> {
    if let Some(json) = args.value("--condition") {
        let value = serde_json::from_str(json)
            .map_err(|error| ClientError::usage(format!("--condition is not JSON: {error}")))?;
        return Condition::parse(value);
    }
    let name = args.positional.first().ok_or_else(|| {
        ClientError::usage(format!(
            "wait needs a condition: {}",
            Condition::NAMES.join(", ")
        ))
    })?;
    let mut object = serde_json::Map::new();
    object.insert("condition".into(), serde_json::Value::String(name.clone()));
    let selector = args.selector(None)?;
    if selector != Default::default() {
        object.insert(
            "selector".into(),
            serde_json::to_value(selector).expect("selectors serialise"),
        );
    }
    if let Some(key) = args.value("--key") {
        object.insert("key".into(), serde_json::Value::String(key.into()));
    }
    if let Some(value) = args.value("--value") {
        let value = match name.as_str() {
            "checked" => serde_json::Value::Bool(match value {
                "true" => true,
                "false" => false,
                _ => {
                    return Err(ClientError::usage(format!(
                        "checked takes --value true or false, not {value:?}"
                    )));
                }
            }),
            // A fact compares a primitive: parse numbers and booleans.
            "fact" => serde_json::from_str(value)
                .unwrap_or_else(|_| serde_json::Value::String(value.into())),
            _ => serde_json::Value::String(value.into()),
        };
        object.insert("value".into(), value);
    }
    Condition::parse(serde_json::Value::Object(object))
}

fn receipt_text(receipt: &InvocationReceipt) -> String {
    let state = serde_json::to_value(receipt.state)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_default();
    let mut text = format!("{state} {}", receipt.request_id);
    if let Some(target) = &receipt.target {
        let _ = write!(text, " · {} on #{}", target.capability, target.id.0);
    }
    if let Some(error) = &receipt.error {
        let code = serde_json::to_value(error.code)
            .ok()
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
            .unwrap_or_default();
        let _ = write!(text, " · {code}: {}", error.message);
    }
    text
}

fn receipt_status(receipt: &InvocationReceipt) -> u8 {
    match receipt.state {
        ReceiptState::Completed | ReceiptState::Queued => 0,
        ReceiptState::Rejected | ReceiptState::Cancelled | ReceiptState::Unknown => 1,
        ReceiptState::Uncertain => 4,
    }
}

/// Facts whose value differs between two observations.
fn changed_facts<'a>(
    before: &'a CompletedObservation,
    after: &'a CompletedObservation,
) -> BTreeMap<&'a str, (Option<&'a ObservationValue>, Option<&'a ObservationValue>)> {
    before
        .facts
        .keys()
        .chain(after.facts.keys())
        .filter_map(|key| {
            let (old, new) = (before.facts.get(key), after.facts.get(key));
            (old != new).then_some((key.as_str(), (old, new)))
        })
        .collect()
}

fn invoked_output(before: &CompletedObservation, invoked: Invoked) -> Output {
    let mut text = receipt_text(&invoked.receipt);
    let changed = invoked
        .observation
        .as_ref()
        .map(|after| changed_facts(before, after));
    if let Some(after) = &invoked.observation {
        let _ = write!(text, " · observation {}", after.id);
    }
    text.push('\n');
    let nodes = invoked
        .observation
        .as_ref()
        .map(|after| output::changed_nodes(before, after));
    if let (Some(changed), Some(nodes)) = (&changed, &nodes) {
        if changed.is_empty() && nodes.is_empty() {
            text.push_str("no observed change in facts or nodes\n");
        } else if changed.is_empty() {
            text.push_str("facts unchanged\n");
        } else {
            let changes: Vec<String> = changed
                .iter()
                .map(|(key, (old, new))| {
                    let show = |value: &Option<&ObservationValue>| {
                        value.map_or_else(
                            || "absent".to_owned(),
                            |value| match value {
                                ObservationValue::Text(text) => format!("{:?}", output::clip(text)),
                                other => serde_json::to_string(other).unwrap_or_default(),
                            },
                        )
                    };
                    format!("{key}: {} → {}", show(old), show(new))
                })
                .collect();
            let _ = writeln!(text, "changed: {}", changes.join(", "));
        }
        if !nodes.is_empty() {
            let _ = writeln!(text, "nodes changed: {}", nodes.len());
            text.push_str(&output::changed_nodes_text(nodes));
        }
    }
    let status = receipt_status(&invoked.receipt);
    let mut output = Output::new(
        text,
        serde_json::json!({
            "receipt": invoked.receipt,
            "observation": invoked.observation.as_ref().map(|observation| observation.id),
            "changed_facts": changed.map(|changed| changed
                .into_iter()
                .map(|(key, (old, new))| (key, serde_json::json!({ "before": old, "after": new })))
                .collect::<BTreeMap<_, _>>()),
            "changed_nodes": nodes.as_ref().map(|nodes| nodes.iter().take(32).collect::<Vec<_>>()),
            "changed_nodes_total": nodes.as_ref().map(Vec::len),
        }),
    );
    output.status = status;
    output
}

fn sessions() -> Result<Output, ClientError> {
    let sessions = session::list()?;
    let mut text = String::new();
    let mut entries = Vec::new();
    for session in &sessions {
        let (place, running) = match &session.target {
            Target::Native { socket, .. } => (
                format!("native {}", socket.display()),
                session::running(session),
            ),
            Target::Browser {
                endpoint,
                page,
                target,
            } => (
                format!(
                    "browser {endpoint} {}",
                    page.as_deref()
                        .or(target.as_deref())
                        .unwrap_or("(only page)")
                ),
                None,
            ),
        };
        let _ = writeln!(
            text,
            "{} · {place}{}",
            session.name,
            match running {
                Some(true) => " · running",
                Some(false) => " · exited",
                None => "",
            }
        );
        entries.push(serde_json::json!({ "session": session, "running": running }));
    }
    if sessions.is_empty() {
        text.push_str(
            "no sessions; start one with `stobie launch NAME ...` or `stobie attach NAME --cdp URL --page URL`\n",
        );
    }
    Ok(Output::new(
        text,
        serde_json::json!({ "sessions": entries }),
    ))
}

fn launch(args: &Args, name: &str) -> Result<Output, ClientError> {
    let timeout = timeout(args, Duration::from_secs(120))?;
    let (program, rest) = match args.rest.split_first() {
        Some((program, rest)) => (program.into(), rest.to_vec()),
        None => {
            if !args.has("--manifest-path") && !args.has("--package") {
                return Err(ClientError::usage(
                    "launch needs --manifest-path PATH, --package PKG or -- PROGRAM [ARGS...]",
                ));
            }
            (
                session::build(
                    args.value("--manifest-path"),
                    args.value("--package"),
                    args.value("--bin"),
                    args.value("--example"),
                    !args.has("--debug"),
                )?,
                Vec::new(),
            )
        }
    };
    let program = std::fs::canonicalize(&program).map_err(|error| {
        ClientError::usage(format!("cannot find {}: {error}", program.display()))
    })?;
    let answered = |socket: &std::path::Path| {
        Client::new(SocketTransport::new(socket))
            .with_timeout(Duration::from_secs(2))
            .hello()
            .and_then(|_| {
                // The application has published its first observation.
                let mut client =
                    Client::new(SocketTransport::new(socket)).with_timeout(Duration::from_secs(2));
                client.hello()?;
                client.observe()
            })
            .is_ok()
    };
    let session = session::launch(name, &program, &rest, answered, timeout)?;
    let Target::Native { socket, pid, .. } = &session.target else {
        unreachable!("launch records a native session")
    };
    let mut client = Client::new(SocketTransport::new(socket.clone()));
    let hello = client.hello()?;
    let _ = std::io::stdout().flush();
    Ok(Output::new(
        format!(
            "launched {name}: {} {} · pid {} · socket {}\nnext: stobie observe --session {name}\n",
            hello.application.name,
            hello.application.build.version,
            pid.map_or_else(|| "unknown".into(), |pid| pid.to_string()),
            socket.display()
        ),
        serde_json::json!({
            "session": session,
            "application": hello.application,
            "log": session::log_path(name).ok(),
        }),
    ))
}

/// The registry, exit codes and conditions, without connecting.
fn capabilities() -> serde_json::Value {
    serde_json::json!({
        "package_version": env!("CARGO_PKG_VERSION"),
        "protocol_version": stobie_client::protocol::INSPECTION_VERSION,
        "commands": spec::COMMANDS,
        "target_flags": spec::TARGET_FLAGS,
        "selector_flags": spec::SELECTOR_FLAGS,
        "exit_codes": spec::EXIT_CODES
            .iter()
            .map(|(code, name, description)| serde_json::json!({
                "code": code, "name": name, "description": description,
            }))
            .collect::<Vec<_>>(),
        "wait_conditions": Condition::NAMES,
        "argument_kinds": ["text", "boolean", "number", "choice"],
        "error_output": "stderr: {schema_version, command, ok: false, error: {code, message, hint, request_id?, observation?}}",
        "sessions": "launch records a session; commands use --session NAME, $STOBIE_SESSION or the only session",
    })
}

#[cfg(test)]
mod tests;
