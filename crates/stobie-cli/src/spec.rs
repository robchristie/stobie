//! The command registry: one description drives `--help`, per-command help
//! and `capabilities --json`.

use serde::Serialize;

/// The JSON output schema version every command emits.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Flag {
    pub name: &'static str,
    /// The value's placeholder, or `None` for a switch.
    pub value: Option<&'static str>,
    pub description: &'static str,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Command {
    pub name: &'static str,
    pub usage: &'static str,
    pub summary: &'static str,
    /// Whether the command addresses an application (needs a target).
    pub connects: bool,
    pub flags: &'static [Flag],
    pub output_schema_version: u32,
}

const fn flag(name: &'static str, value: Option<&'static str>, description: &'static str) -> Flag {
    Flag {
        name,
        value,
        description,
    }
}

/// Flags every connecting command accepts.
pub const TARGET_FLAGS: &[Flag] = &[
    flag(
        "--session",
        Some("NAME"),
        "Use a session from `launch` or `attach` (default: $STOBIE_SESSION, or the only session)",
    ),
    flag(
        "--socket",
        Some("PATH"),
        "Attach to a native application's automation socket",
    ),
    flag(
        "--cdp",
        Some("URL"),
        "Attach to a browser page through Chromium's DevTools endpoint, such as http://127.0.0.1:9222",
    ),
    flag(
        "--page",
        Some("URL"),
        "With --cdp: the page's URL, which must include ?automation=1 (stobie does not add it; a unique prefix is enough)",
    ),
    flag(
        "--target",
        Some("ID"),
        "With --cdp: the page's DevTools target ID",
    ),
    flag(
        "--timeout",
        Some("MS"),
        "Deadline for the operation, including waits (default 15000)",
    ),
    flag("--json", None, "Emit one JSON object instead of text"),
];

/// Selector flags: all given properties must match (exact AND).
pub const SELECTOR_FLAGS: &[Flag] = &[
    flag(
        "--id",
        Some("ID"),
        "Stable semantic ID, such as record-desk.apply",
    ),
    flag(
        "--role",
        Some("ROLE"),
        "Semantic role, such as button or text_input",
    ),
    flag("--name", Some("NAME"), "Exact accessible name"),
    flag(
        "--capability",
        Some("ID"),
        "Capability ID, such as record-desk.apply",
    ),
    flag(
        "--pane",
        Some("N"),
        "Pane ID carried by the node or capability itself (controls inside a pane may carry none)",
    ),
    flag(
        "--domain",
        Some("JSON"),
        "Domain reference, such as {\"kind\":\"pane\",\"value\":1}",
    ),
    flag(
        "--selector",
        Some("JSON"),
        "The whole selector as JSON, for example {\"role\":\"tab\",\"pane\":2}",
    ),
];

const PAGE_FLAGS: &[Flag] = &[
    flag(
        "--limit",
        Some("N"),
        "Entries per page, 1 to 256 (default 64)",
    ),
    flag(
        "--cursor",
        Some("CURSOR"),
        "Continue from a previous page's next cursor",
    ),
];

pub const COMMANDS: &[Command] = &[
    Command {
        name: "capabilities",
        usage: "stobie capabilities [--json]",
        summary: "Describe commands, flags, exit codes, wait conditions and the output envelope as JSON, without connecting",
        connects: false,
        flags: &[flag(
            "--json",
            None,
            "Accepted for consistency; the output is always JSON",
        )],
        output_schema_version: SCHEMA_VERSION,
    },
    Command {
        name: "new",
        usage: "stobie new NAME [--path DIR] [--imaging] [--stobie-rev COMMIT] [--stobie-path CHECKOUT] [--json]",
        summary: "Create a standalone application in its own Cargo workspace, native and web, depending on Stobie through git pinned to this CLI's revision",
        connects: false,
        flags: &[
            flag(
                "--path",
                Some("DIR"),
                "Where to create it (default: ./NAME); must be new or empty",
            ),
            flag(
                "--imaging",
                None,
                "Add the image layer: stobie-imaging, stobie-runtime, stobie-render-wgpu and the UI crate's image-rendering feature",
            ),
            flag(
                "--stobie-rev",
                Some("COMMIT"),
                "Pin this full Stobie commit instead of the one this CLI was built from",
            ),
            flag(
                "--stobie-path",
                Some("CHECKOUT"),
                "Also write .cargo/config.toml patching Stobie to a local checkout, to test unmerged changes",
            ),
            flag("--json", None, "Emit one JSON object instead of text"),
        ],
        output_schema_version: SCHEMA_VERSION,
    },
    Command {
        name: "build-web",
        usage: "stobie build-web [--manifest-path PATH] [--package PKG] [--web-dir DIR] [--debug] [--locked] [--production [--output DIR] [--variant none|Oz|O3] [--wasm-opt PATH]] [--json]",
        summary: "Build an application for the browser: wasm32 release build, wasm-bindgen into WEB/pkg (the CLI must match the project's wasm-bindgen crate) and the startup loader; --production also packages compressed, immutable assets",
        connects: false,
        flags: &[
            flag(
                "--manifest-path",
                Some("PATH"),
                "The application's Cargo.toml (default: the current directory's)",
            ),
            flag(
                "--package",
                Some("PKG"),
                "The package, in a workspace with several",
            ),
            flag(
                "--web-dir",
                Some("DIR"),
                "The web root with index.html and bootstrap.js (default: web/ beside Cargo.toml)",
            ),
            flag(
                "--debug",
                None,
                "Build with the dev profile instead of release",
            ),
            flag("--locked", None, "Require Cargo.lock to be up to date"),
            flag(
                "--production",
                None,
                "Also package a clean copy with Brotli and gzip representations and content-addressed assets",
            ),
            flag(
                "--output",
                Some("DIR"),
                "With --production: the package directory (default: TARGET/web-production)",
            ),
            flag(
                "--variant",
                Some("VARIANT"),
                "With --production: none (default), Oz or O3, which run Binaryen 131's wasm-opt",
            ),
            flag(
                "--wasm-opt",
                Some("PATH"),
                "With --production: the wasm-opt executable (default: $WASM_OPT or wasm-opt)",
            ),
            flag("--json", None, "Emit one JSON object instead of text"),
        ],
        output_schema_version: SCHEMA_VERSION,
    },
    Command {
        name: "serve",
        usage: "stobie serve [DIR] [--port N] [--json]",
        summary: "Serve a built web root, or a --production package, on 127.0.0.1 for local preview, without caching, until interrupted (default DIR: web/)",
        connects: false,
        flags: &[
            flag(
                "--port",
                Some("N"),
                "The port, or 0 for any free one (default 8080)",
            ),
            flag(
                "--json",
                None,
                "Print the URL as one JSON object once listening",
            ),
        ],
        output_schema_version: SCHEMA_VERSION,
    },
    Command {
        name: "package-web",
        usage: "stobie package-web NAME=DIR... --output DIR [--variant none|Oz|O3] [--wasm-opt PATH] [--source-identity-file JSON] [--json]",
        summary: "Package already built web roots, such as an application and its worker, into one production directory with a manifest",
        connects: false,
        flags: &[
            flag(
                "--output",
                Some("DIR"),
                "The package directory; replaced only by a previous package",
            ),
            flag(
                "--variant",
                Some("VARIANT"),
                "none (default), Oz or O3, which run Binaryen 131's wasm-opt",
            ),
            flag(
                "--wasm-opt",
                Some("PATH"),
                "The wasm-opt executable (default: $WASM_OPT or wasm-opt)",
            ),
            flag(
                "--source-identity-file",
                Some("JSON"),
                "A JSON object recording the build configuration; all but revision and dirty change asset identity",
            ),
            flag("--json", None, "Emit one JSON object instead of text"),
        ],
        output_schema_version: SCHEMA_VERSION,
    },
    Command {
        name: "launch",
        usage: "stobie launch NAME (--manifest-path PATH | --package PKG | -- PROGRAM [ARGS...]) [--bin NAME | --example NAME] [--debug]",
        summary: "Build if needed and start a native application with a fresh private automation socket, record it as session NAME and wait until it answers",
        connects: false,
        flags: &[
            flag(
                "--manifest-path",
                Some("PATH"),
                "Cargo manifest of the application to build",
            ),
            flag(
                "--package",
                Some("PKG"),
                "Cargo package to build in the current workspace",
            ),
            flag(
                "--bin",
                Some("NAME"),
                "Binary to run when the package has several",
            ),
            flag(
                "--example",
                Some("NAME"),
                "Cargo example to build and run instead of a binary",
            ),
            flag("--debug", None, "Build without --release"),
            flag(
                "--timeout",
                Some("MS"),
                "How long to wait for the application to answer (default 120000)",
            ),
            flag("--json", None, "Emit one JSON object instead of text"),
        ],
        output_schema_version: SCHEMA_VERSION,
    },
    Command {
        name: "attach",
        usage: "stobie attach NAME --cdp URL [--page URL | --target ID]",
        summary: "Record session NAME for a browser page opened with ?automation=1, once its application has started and answers",
        connects: false,
        flags: &[
            flag(
                "--cdp",
                Some("URL"),
                "The browser's DevTools HTTP endpoint, such as http://127.0.0.1:9222 for a Chromium started with --remote-debugging-port=9222",
            ),
            flag(
                "--page",
                Some("URL"),
                "The page's URL, including ?automation=1 (a unique prefix is enough)",
            ),
            flag("--target", Some("ID"), "The page's DevTools target ID"),
            flag(
                "--timeout",
                Some("MS"),
                "How long to wait for the page to match and its application to start and answer (default 15000)",
            ),
            flag("--json", None, "Emit one JSON object instead of text"),
        ],
        output_schema_version: SCHEMA_VERSION,
    },
    Command {
        name: "stop",
        usage: "stobie stop NAME [--json]",
        summary: "Stop a launched application and remove its session and socket directory; for an attached browser session, forget the session and leave the page running",
        connects: false,
        flags: &[flag("--json", None, "Emit one JSON object instead of text")],
        output_schema_version: SCHEMA_VERSION,
    },
    Command {
        name: "sessions",
        usage: "stobie sessions [--json]",
        summary: "List recorded sessions, and whether each launched application is still running",
        connects: false,
        flags: &[flag("--json", None, "Emit one JSON object instead of text")],
        output_schema_version: SCHEMA_VERSION,
    },
    Command {
        name: "hello",
        usage: "stobie hello [TARGET FLAGS]",
        summary: "Show the application's identity, protocol limits and operations",
        connects: true,
        flags: &[],
        output_schema_version: SCHEMA_VERSION,
    },
    Command {
        name: "observe",
        usage: "stobie observe [--limit N] [--offset N] [--include-text] [TARGET FLAGS]",
        summary: "Outline the current UI: role, name, state, value, stable ID and capabilities of each node, with application facts",
        connects: true,
        flags: &[
            flag("--limit", Some("N"), "Nodes to show (default 200)"),
            flag("--offset", Some("N"), "Skip this many nodes"),
            flag(
                "--include-text",
                None,
                "Include text layout measurements: geometry, line counts and overflow, not the strings (JSON only)",
            ),
        ],
        output_schema_version: SCHEMA_VERSION,
    },
    Command {
        name: "query",
        usage: "stobie query [ID] [SELECTOR FLAGS] [--limit N] [--cursor CURSOR] [TARGET FLAGS]",
        summary: "List nodes matching a selector, with the capabilities bound to each",
        connects: true,
        flags: PAGE_FLAGS,
        output_schema_version: SCHEMA_VERSION,
    },
    Command {
        name: "discover",
        usage: "stobie discover [CAPABILITY] [SELECTOR FLAGS] [--limit N] [--cursor CURSOR] [TARGET FLAGS]",
        summary: "List capabilities: availability, argument contract, shortcut and target",
        connects: true,
        flags: PAGE_FLAGS,
        output_schema_version: SCHEMA_VERSION,
    },
    Command {
        name: "invoke",
        usage: "stobie invoke CAPABILITY [SELECTOR FLAGS] [--value VALUE | --value-json JSON] [--expected JSON] [--request-id ID] [--no-wait] [TARGET FLAGS]",
        summary: "Invoke one capability with an optional typed value, wait for its receipt, then for the observation that shows its effect",
        connects: true,
        flags: &[
            flag(
                "--value",
                Some("VALUE"),
                "The argument, converted by the capability's contract: text, true/false, a number, or a choice ID or label",
            ),
            flag("--value-json", Some("JSON"), "The argument as a JSON value"),
            flag(
                "--expected",
                Some("JSON"),
                "A previously discovered target, to fail if it changed",
            ),
            flag(
                "--request-id",
                Some("ID"),
                "The mutation's ID; retrying with it never runs the action twice",
            ),
            flag("--no-wait", None, "Return once the invocation is admitted"),
        ],
        output_schema_version: SCHEMA_VERSION,
    },
    Command {
        name: "set-text",
        usage: "stobie set-text ID (TEXT | - | --text-file PATH) [SELECTOR FLAGS] [--request-id ID] [TARGET FLAGS]",
        summary: "Replace a text field's text as typing would; `-` reads the text from standard input",
        connects: true,
        flags: &[
            flag("--text-file", Some("PATH"), "Read the text from a file"),
            flag(
                "--request-id",
                Some("ID"),
                "The mutation's ID; retrying with it never runs the edit twice",
            ),
        ],
        output_schema_version: SCHEMA_VERSION,
    },
    Command {
        name: "wait",
        usage: "stobie wait CONDITION [SELECTOR FLAGS] [--value VALUE] [--key KEY] [--condition JSON] [TARGET FLAGS]",
        summary: "Wait until a condition holds: present, absent, enabled, capability_enabled, focused, selected, checked, name, value, status, selection_changed, fact or audits_clear",
        connects: true,
        flags: &[
            flag(
                "--value",
                Some("VALUE"),
                "Expected text, or true/false for checked; for fact, a JSON value or text",
            ),
            flag("--key", Some("KEY"), "Fact name, for the fact condition"),
            flag(
                "--condition",
                Some("JSON"),
                "The whole condition as JSON, for example {\"condition\":\"fact\",\"key\":\"undo_entries\",\"value\":1}",
            ),
        ],
        output_schema_version: SCHEMA_VERSION,
    },
    Command {
        name: "capture",
        usage: "stobie capture --output PNG [TARGET FLAGS]",
        summary: "Screenshot the application, bracketed by observations before and after: a browser canvas through DevTools, or a launched native window on X11 (needs xdotool and ImageMagick's import)",
        connects: true,
        flags: &[flag(
            "--output",
            Some("PNG"),
            "Where to write the screenshot (an existing file is replaced)",
        )],
        output_schema_version: SCHEMA_VERSION,
    },
    Command {
        name: "receipt",
        usage: "stobie receipt REQUEST_ID [TARGET FLAGS]",
        summary: "Read an invocation's receipt; use it instead of repeating an uncertain mutation",
        connects: true,
        flags: &[],
        output_schema_version: SCHEMA_VERSION,
    },
    Command {
        name: "cancel",
        usage: "stobie cancel REQUEST_ID [TARGET FLAGS]",
        summary: "Cancel a queued invocation that has not started",
        connects: true,
        flags: &[],
        output_schema_version: SCHEMA_VERSION,
    },
];

/// Exit statuses, stable across versions.
pub const EXIT_CODES: &[(i32, &str, &str)] = &[
    (0, "ok", "The command succeeded"),
    (
        1,
        "rejected",
        "The application rejected or could not match the request, an invocation was rejected, or a build or capture failed",
    ),
    (2, "usage", "The command line or a request was malformed"),
    (
        3,
        "connection",
        "No application answered, or it restarted (wrong instance)",
    ),
    (
        4,
        "deadline",
        "A deadline passed, or a mutation's outcome is uncertain: read its receipt",
    ),
    (
        5,
        "partial",
        "Evidence is partial, for example a capture whose screenshot failed",
    ),
];

pub fn find(name: &str) -> Option<&'static Command> {
    COMMANDS.iter().find(|command| command.name == name)
}

fn flags_help(flags: &[Flag]) -> String {
    flags
        .iter()
        .map(|flag| {
            let left = match flag.value {
                Some(value) => format!("{} {value}", flag.name),
                None => flag.name.to_owned(),
            };
            format!("  {left:<26} {}\n", flag.description)
        })
        .collect()
}

pub fn help() -> String {
    let mut text = String::from(
        "stobie: create and build Stobie applications, then launch or attach to one, read its UI compactly and operate it.\n\n\
         Usage: stobie COMMAND [ARGS] [FLAGS]   (stobie COMMAND --help for details)\n\nCommands:\n",
    );
    for command in COMMANDS {
        text.push_str(&format!("  {:<13} {}\n", command.name, command.summary));
    }
    text.push_str("\nTarget flags (commands that address an application):\n");
    text.push_str(&flags_help(TARGET_FLAGS));
    text.push_str("\nSelector flags (exact AND match):\n");
    text.push_str(&flags_help(SELECTOR_FLAGS));
    text.push_str("\nExit statuses:\n");
    for (code, name, description) in EXIT_CODES {
        text.push_str(&format!("  {code} {name:<11} {description}\n"));
    }
    text.push_str(
        "\nTypical browser session (serve the app, open it with ?automation=1):\n  \
         stobie attach web --cdp http://127.0.0.1:9222 --page 'http://127.0.0.1:8080/?automation=1'\n\
         \nA new application:\n  \
         stobie new my-app && cd my-app\n  \
         cargo run --release                  # native\n  \
         stobie build-web && stobie serve     # browser, at http://127.0.0.1:8080/\n\
         \nTypical native session:\n  \
         stobie launch app --manifest-path path/to/Cargo.toml   (or: stobie launch app -- ./program)\n  \
         stobie observe                       # outline: role \"name\" state #ID [capabilities]\n  \
         stobie discover CAPABILITY           # availability and argument contract\n  \
         stobie set-text ID 'text'\n  \
         stobie invoke CAPABILITY [--value VALUE]\n  \
         stobie stop app\n",
    );
    text
}

pub fn command_help(command: &Command) -> String {
    let mut text = format!("{}\n\nUsage: {}\n", command.summary, command.usage);
    if !command.flags.is_empty() {
        text.push_str("\nFlags:\n");
        text.push_str(&flags_help(command.flags));
    }
    if command.connects {
        text.push_str("\nTarget flags:\n");
        text.push_str(&flags_help(TARGET_FLAGS));
        if matches!(
            command.name,
            "query" | "discover" | "invoke" | "set-text" | "wait"
        ) {
            text.push_str("\nSelector flags:\n");
            text.push_str(&flags_help(SELECTOR_FLAGS));
        }
    }
    text
}
