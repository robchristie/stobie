//! Creating and building applications: `new`, `build-web`, `serve` and
//! `package-web`. Building and packaging belong to `stobie-build`, which the
//! repository's own tooling shares.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use stobie_build::package::{self, Variant};
use stobie_client::ClientError;

use crate::Output;
use crate::args::Args;

/// The repository applications depend on through git.
pub const STOBIE_GIT: &str = "https://github.com/robchristie/stobie";
/// The Rust toolchain the template pins; the repository's own.
pub const RUST_TOOLCHAIN: &str = "1.99.0";
/// The wasm-bindgen version the template pins; the workspace's own.
pub const WASM_BINDGEN: &str = "0.2.127";

/// The revision this CLI was built from, when known.
pub fn source_revision() -> Option<&'static str> {
    option_env!("STOBIE_SOURCE_REVISION")
}

fn build_error(error: stobie_build::Error) -> ClientError {
    ClientError::new("build_failed", error.message, error.hint)
}

const TEMPLATE: &[(&str, &str)] = &[
    ("Cargo.toml", include_str!("../template/Cargo.toml.in")),
    ("README.md", include_str!("../template/README.md")),
    (
        "rust-toolchain.toml",
        include_str!("../template/rust-toolchain.toml"),
    ),
    (".gitignore", include_str!("../template/gitignore")),
    ("src/lib.rs", include_str!("../template/src/lib.rs")),
    ("src/main.rs", include_str!("../template/src/main.rs")),
    ("web/index.html", include_str!("../template/web/index.html")),
    (
        "web/bootstrap.js",
        include_str!("../template/web/bootstrap.js"),
    ),
];

/// Names Cargo or Rust reserve, which cannot name a package.
const RESERVED: &[&str] = &[
    "test",
    "core",
    "std",
    "alloc",
    "proc-macro",
    "proc_macro",
    "self",
    "crate",
    "super",
    "build",
    "deps",
    "examples",
    "incremental",
];

/// What `stobie new` generates.
#[derive(Clone, Debug)]
pub struct NewApp {
    pub name: String,
    pub revision: String,
    pub imaging: bool,
}

impl NewApp {
    pub fn crate_name(&self) -> String {
        self.name.replace('-', "_")
    }

    pub fn title(&self) -> String {
        self.name
            .split('-')
            .filter(|word| !word.is_empty())
            .map(|word| {
                let mut characters = word.chars();
                let first = characters.next().expect("non-empty").to_ascii_uppercase();
                std::iter::once(first).chain(characters).collect::<String>()
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The Stobie crates the application depends on directly.
    pub fn stobie_crates(&self) -> Vec<&'static str> {
        let mut crates = vec!["stobie-core", "stobie-ui-egui"];
        if self.imaging {
            crates.extend(["stobie-imaging", "stobie-runtime", "stobie-render-wgpu"]);
        }
        crates
    }

    fn dependencies(&self) -> String {
        let pin = format!("git = \"{STOBIE_GIT}\", rev = \"{}\"", self.revision);
        let features = if self.imaging {
            "\"eframe\", \"image-rendering\""
        } else {
            "\"eframe\""
        };
        let mut text = String::new();
        for name in self.stobie_crates() {
            if name == "stobie-ui-egui" {
                text.push_str(&format!(
                    "{name} = {{ {pin}, default-features = false, features = [{features}] }}\n"
                ));
            } else {
                text.push_str(&format!("{name} = {{ {pin} }}\n"));
            }
        }
        text
    }

    /// Every generated file, as a path relative to the application and its text.
    pub fn files(&self) -> Vec<(&'static str, String)> {
        TEMPLATE
            .iter()
            .map(|(path, text)| {
                let text = text
                    .replace("{{stobie_dependencies}}", &self.dependencies())
                    .replace("{{stobie_rev}}", &self.revision)
                    .replace("{{wasm_bindgen}}", WASM_BINDGEN)
                    .replace("{{rust_toolchain}}", RUST_TOOLCHAIN)
                    .replace("{{crate}}", &self.crate_name())
                    .replace("{{title}}", &self.title())
                    .replace("{{name}}", &self.name);
                (*path, text)
            })
            .collect()
    }

    /// `.cargo/config.toml` patching every Stobie dependency to `checkout`.
    pub fn local_patch(&self, checkout: &Path) -> String {
        let mut text = format!(
            "# Build against a local Stobie checkout instead of the pinned revision.\n\
             # Cargo still fetches the pinned revision to resolve this patch.\n\
             [patch.\"{STOBIE_GIT}\"]\n"
        );
        for name in self.stobie_crates() {
            let path = checkout.join("crates").join(name);
            text.push_str(&format!(
                "{name} = {{ path = {} }}\n",
                toml_string(&path.to_string_lossy())
            ));
        }
        text
    }
}

fn toml_string(value: &str) -> String {
    serde_json::to_string(value).expect("strings serialise")
}

pub fn valid_name(name: &str) -> Result<(), ClientError> {
    let mut characters = name.chars();
    let shaped = characters.next().is_some_and(|c| c.is_ascii_lowercase())
        && characters.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.ends_with('-')
        && !name.contains("--");
    if !shaped || RESERVED.contains(&name) || name.len() > 64 {
        return Err(ClientError::usage(format!(
            "{name:?} cannot name an application: use lower-case letters, digits and single hyphens, starting with a letter, such as my-notes"
        )));
    }
    Ok(())
}

fn full_revision(value: &str) -> Result<String, ClientError> {
    if value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(value.to_ascii_lowercase())
    } else {
        Err(ClientError::usage(format!(
            "--stobie-rev needs a full 40-character commit ID from {STOBIE_GIT}, not {value:?}"
        )))
    }
}

#[derive(Serialize)]
struct Created<'a> {
    path: &'a Path,
    name: &'a str,
    stobie_rev: &'a str,
    imaging: bool,
    stobie_path: Option<&'a Path>,
    files: Vec<&'a str>,
    next: Vec<String>,
}

pub fn new(args: &Args, name: &str) -> Result<Output, ClientError> {
    valid_name(name)?;
    let revision = match (args.value("--stobie-rev"), source_revision()) {
        (Some(value), _) => full_revision(value)?,
        (None, Some(revision)) => revision.to_owned(),
        (None, None) => {
            return Err(ClientError::usage(
                "this stobie was built outside a Git checkout, so it cannot pin a Stobie revision; pass --stobie-rev COMMIT",
            ));
        }
    };
    let app = NewApp {
        name: name.to_owned(),
        revision,
        imaging: args.has("--imaging"),
    };
    let directory = PathBuf::from(args.value("--path").unwrap_or(name));
    if directory.exists()
        && fs::read_dir(&directory)
            .map(|mut entries| entries.next().is_some())
            .unwrap_or(true)
    {
        return Err(ClientError::usage(format!(
            "{} already exists and is not empty; choose another NAME or --path",
            directory.display()
        )));
    }
    let checkout = match args.value("--stobie-path") {
        Some(path) => {
            let path = fs::canonicalize(path)
                .map_err(|error| ClientError::usage(format!("--stobie-path {path}: {error}")))?;
            if !path.join("crates/stobie-core/Cargo.toml").is_file() {
                return Err(ClientError::usage(format!(
                    "--stobie-path {} is not a Stobie checkout (no crates/stobie-core)",
                    path.display()
                )));
            }
            Some(path)
        }
        None => None,
    };
    let io = |path: &Path| {
        let path = path.display().to_string();
        move |error: std::io::Error| {
            ClientError::new("io_failed", format!("write {path}: {error}"), "")
        }
    };
    let mut written = Vec::new();
    let mut files = app.files();
    if let Some(checkout) = &checkout {
        files.push((".cargo/config.toml", app.local_patch(checkout)));
    }
    for (relative, text) in &files {
        let path = directory.join(relative);
        fs::create_dir_all(path.parent().expect("files have parents")).map_err(io(&path))?;
        fs::write(&path, text).map_err(io(&path))?;
        written.push(*relative);
    }
    let directory = fs::canonicalize(&directory).unwrap_or(directory);
    let next = vec![
        format!("cd {}", directory.display()),
        "cargo run --release".to_owned(),
        "stobie build-web && stobie serve".to_owned(),
        "stobie launch app --manifest-path Cargo.toml".to_owned(),
    ];
    let mut text = format!(
        "created {} in {}\n  Stobie {} (git, pinned)\n",
        app.title(),
        directory.display(),
        &app.revision[..12]
    );
    if let Some(checkout) = &checkout {
        text.push_str(&format!(
            "  .cargo/config.toml builds against {}\n",
            checkout.display()
        ));
    }
    text.push_str("next:\n");
    for line in &next {
        text.push_str(&format!("  {line}\n"));
    }
    Ok(Output::new(
        text,
        Created {
            path: &directory,
            name,
            stobie_rev: &app.revision,
            imaging: app.imaging,
            stobie_path: checkout.as_deref(),
            files: written,
            next,
        },
    ))
}

#[derive(Serialize)]
struct Built<'a> {
    package: &'a str,
    web_dir: &'a Path,
    wasm: &'a Path,
    wasm_bindgen: &'a str,
    production: Option<Production<'a>>,
}

#[derive(Serialize)]
struct Production<'a> {
    output: &'a Path,
    content_id: &'a str,
    entry: String,
}

fn variant(args: &Args) -> Result<Variant, ClientError> {
    args.value("--variant")
        .map_or(Ok(Variant::None), Variant::parse)
        .map_err(build_error)
}

pub fn build_web(args: &Args) -> Result<Output, ClientError> {
    let production = args.has("--production");
    for flag in ["--output", "--variant", "--wasm-opt"] {
        if args.has(flag) && !production {
            return Err(ClientError::usage(format!("{flag} needs --production")));
        }
    }
    let options = stobie_build::BuildOptions {
        manifest_path: args.value("--manifest-path").map(PathBuf::from),
        package: args.value("--package").map(str::to_owned),
        web_dir: args.value("--web-dir").map(PathBuf::from),
        debug: args.has("--debug"),
        locked: args.has("--locked"),
        production: production
            .then(|| -> Result<_, ClientError> {
                Ok(stobie_build::ProductionOptions {
                    output: args.value("--output").map(PathBuf::from),
                    variant: variant(args)?,
                    wasm_opt: args.value("--wasm-opt").map(PathBuf::from),
                })
            })
            .transpose()?,
    };
    let report = stobie_build::build(&options).map_err(build_error)?;
    let production = report.production.as_ref().map(|(output, manifest)| {
        let app = &manifest.apps[0];
        Production {
            output,
            content_id: &app.content_id,
            entry: format!("{}/", app.name),
        }
    });
    let mut text = format!(
        "built {} for the web in {} (wasm-bindgen {})\n",
        report.package,
        report.web_dir.display(),
        report.wasm_bindgen
    );
    match &production {
        Some(production) => text.push_str(&format!(
            "packaged {} under {} (assets {})\n",
            production.entry,
            production.output.display(),
            &production.content_id[..12]
        )),
        None => text.push_str(&format!(
            "next: stobie serve {}\n",
            report.web_dir.display()
        )),
    }
    Ok(Output::new(
        text,
        Built {
            package: &report.package,
            web_dir: &report.web_dir,
            wasm: &report.wasm,
            wasm_bindgen: &report.wasm_bindgen,
            production,
        },
    ))
}

pub fn package_web(args: &Args) -> Result<Output, ClientError> {
    if args.positional.is_empty() {
        return Err(ClientError::usage(
            "package-web needs at least one NAME=DIR web root",
        ));
    }
    let apps = args
        .positional
        .iter()
        .map(|app| {
            app.split_once('=')
                .filter(|(name, directory)| !name.is_empty() && !directory.is_empty())
                .map(|(name, directory)| package::App {
                    name: name.to_owned(),
                    directory: directory.into(),
                })
                .ok_or_else(|| ClientError::usage(format!("{app:?} is not NAME=DIR")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let output = args
        .value("--output")
        .ok_or_else(|| ClientError::usage("package-web needs --output DIR"))?;
    let source_identity = match args.value("--source-identity-file") {
        Some(path) => {
            let text = fs::read_to_string(path).map_err(|error| {
                ClientError::usage(format!("--source-identity-file {path}: {error}"))
            })?;
            match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(value @ serde_json::Value::Object(_)) => value,
                _ => {
                    return Err(ClientError::usage(
                        "--source-identity-file requires a JSON object",
                    ));
                }
            }
        }
        None => serde_json::Value::Null,
    };
    let manifest = package::package(&package::Options {
        apps,
        output: output.into(),
        variant: variant(args)?,
        wasm_opt: args
            .value("--wasm-opt")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("WASM_OPT").map(PathBuf::from))
            .unwrap_or_else(|| "wasm-opt".into()),
        source_identity,
    })
    .map_err(build_error)?;
    let text = manifest
        .apps
        .iter()
        .map(|app| {
            format!(
                "packaged {}/ under {output} (assets {})\n",
                app.name,
                &app.content_id[..12]
            )
        })
        .collect();
    Ok(Output::new(
        text,
        serde_json::json!({ "manifest": manifest }),
    ))
}

/// Serve a web root until interrupted. Prints one line, or one JSON
/// envelope with `--json`, once it listens.
pub fn serve(args: &Args, json: bool) -> Result<std::convert::Infallible, ClientError> {
    let root = match args.positional.first() {
        Some(path) => PathBuf::from(path),
        None if Path::new("web/index.html").is_file() => PathBuf::from("web"),
        None => PathBuf::from("."),
    };
    let port = args.number::<u16>("--port")?.unwrap_or(8080);
    let listener =
        stobie_build::serve::bind(&root, ([127, 0, 0, 1], port).into()).map_err(build_error)?;
    let address = listener.local_addr().map_err(|error| {
        ClientError::new(
            "io_failed",
            format!("read the listening address: {error}"),
            "",
        )
    })?;
    let url = format!("http://{address}/");
    if json {
        crate::emit(&format!(
            "{}\n",
            crate::envelope(
                "serve",
                true,
                &serde_json::json!({ "url": url, "root": root }).to_string()
            )
        ));
    } else {
        crate::emit(&format!(
            "serving {} at {url} (no-store; Ctrl+C stops)\nopen {url}?automation=1 for `stobie attach`; add &renderer=webgl2 to force WebGL2\n",
            root.display()
        ));
    }
    stobie_build::serve::run(&root, listener);
    unreachable!("the server runs until the process ends")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(imaging: bool) -> NewApp {
        NewApp {
            name: "field-notes".into(),
            revision: "0123456789abcdef0123456789abcdef01234567".into(),
            imaging,
        }
    }

    #[test]
    fn names_are_package_names_that_derive_crate_and_title() {
        for name in ["field-notes", "a", "app2"] {
            assert!(valid_name(name).is_ok(), "{name}");
        }
        for name in [
            "", "Notes", "2app", "my_app", "my--app", "app-", "test", "a b",
        ] {
            assert!(valid_name(name).is_err(), "{name}");
        }
        assert_eq!(app(false).crate_name(), "field_notes");
        assert_eq!(app(false).title(), "Field Notes");
    }

    #[test]
    fn every_placeholder_is_filled_and_every_dependency_is_pinned() {
        for imaging in [false, true] {
            let app = app(imaging);
            for (path, text) in app.files() {
                assert!(!text.contains("{{"), "{path} keeps a placeholder");
            }
            let files = app.files();
            let manifest = &files
                .iter()
                .find(|(path, _)| *path == "Cargo.toml")
                .unwrap()
                .1;
            for name in app.stobie_crates() {
                assert!(
                    manifest.contains(&format!(
                        "{name} = {{ git = \"{STOBIE_GIT}\", rev = \"{}\"",
                        app.revision
                    )),
                    "{name} is pinned"
                );
            }
            assert_eq!(manifest.contains("image-rendering"), imaging);
            assert_eq!(manifest.contains("stobie-imaging"), imaging);
            assert!(manifest.contains(&format!("wasm-bindgen = \"={WASM_BINDGEN}\"")));
            let patch = app.local_patch(Path::new("/src/stobie"));
            for name in app.stobie_crates() {
                assert!(patch.contains(&format!(
                    "{name} = {{ path = \"/src/stobie/crates/{name}\" }}"
                )));
            }
        }
        let files = app(false).files();
        let bootstrap = &files
            .iter()
            .find(|(path, _)| *path == "web/bootstrap.js")
            .unwrap()
            .1;
        assert!(bootstrap.contains("import('./pkg/field_notes.js')"));
        assert!(bootstrap.contains(&format!("./{}", stobie_build::STARTUP_LOADER_FILE)));
    }

    /// The template pins what the repository itself builds with.
    #[test]
    fn template_pins_match_the_repository() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let toolchain = fs::read_to_string(root.join("rust-toolchain.toml")).unwrap();
        assert!(toolchain.contains(&format!("channel = \"{RUST_TOOLCHAIN}\"")));
        let workspace = fs::read_to_string(root.join("Cargo.toml")).unwrap();
        assert!(workspace.contains(&format!("wasm-bindgen = \"{WASM_BINDGEN}\"")));
        assert!(workspace.contains("eframe = { version = \"0.36.1\""));
        assert!(workspace.contains("rust-version = \"1.97.1\""));
        assert_eq!(source_revision().map(str::len).unwrap_or(40), 40);
    }

    #[test]
    fn revisions_must_be_full_commit_ids() {
        assert!(full_revision("f79b477").is_err());
        assert_eq!(
            full_revision("0123456789ABCDEF0123456789abcdef01234567").unwrap(),
            "0123456789abcdef0123456789abcdef01234567"
        );
    }
}
