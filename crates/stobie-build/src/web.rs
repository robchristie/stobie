//! Building one application's web root: cargo for `wasm32-unknown-unknown`,
//! the `wasm-bindgen` version check and bindgen, the startup loader, and the
//! optional production package.

use std::fs;
use std::io::BufRead as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

use crate::package::{self, Manifest, Variant};
use crate::{Error, write_startup_loader};

/// Names inside a web root that builds generate. Staging for a production
/// package skips them and writes fresh ones.
pub const GENERATED: &[&str] = &["pkg", crate::STARTUP_LOADER_FILE];

#[derive(Clone, Debug, Default)]
pub struct BuildOptions {
    /// The application's `Cargo.toml`; the current directory's when absent.
    pub manifest_path: Option<PathBuf>,
    /// The package in a multi-package workspace.
    pub package: Option<String>,
    /// The web root holding `index.html`; `web/` beside the package manifest
    /// when absent.
    pub web_dir: Option<PathBuf>,
    /// Build with the dev profile instead of release.
    pub debug: bool,
    /// Pass `--locked` to cargo.
    pub locked: bool,
    pub production: Option<ProductionOptions>,
}

#[derive(Clone, Debug)]
pub struct ProductionOptions {
    /// The package directory; `<target>/web-production` when absent.
    pub output: Option<PathBuf>,
    pub variant: Variant,
    /// `wasm-opt` for an optimised variant; `$WASM_OPT` or `wasm-opt`.
    pub wasm_opt: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct BuildReport {
    pub package: String,
    pub web_dir: PathBuf,
    pub wasm: PathBuf,
    pub wasm_bindgen: String,
    pub production: Option<(PathBuf, Manifest)>,
}

fn io(context: impl std::fmt::Display) -> impl FnOnce(std::io::Error) -> Error {
    move |error| Error::new(format!("{context}: {error}"), "")
}

fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
}

/// `cargo metadata` for the wasm target, with the dependency graph.
pub fn cargo_metadata(manifest_path: Option<&Path>) -> Result<Value, Error> {
    let mut command = cargo();
    command.args([
        "metadata",
        "--format-version",
        "1",
        "--filter-platform",
        "wasm32-unknown-unknown",
    ]);
    if let Some(path) = manifest_path {
        command.arg("--manifest-path").arg(path);
    }
    let output = command
        .stderr(Stdio::inherit())
        .output()
        .map_err(io("run cargo metadata"))?;
    if !output.status.success() {
        return Err(Error::new(
            "cargo metadata failed",
            "run it in the application's directory or pass --manifest-path",
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| Error::new(format!("read cargo metadata: {error}"), ""))
}

/// The application package: `--package`, the workspace root package, or the
/// only member.
fn application<'a>(metadata: &'a Value, name: Option<&str>) -> Result<&'a Value, Error> {
    let members: Vec<&str> = metadata["workspace_members"]
        .as_array()
        .map(|members| members.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let find = |id: &str| {
        metadata["packages"]
            .as_array()
            .and_then(|packages| packages.iter().find(|package| package["id"] == id))
    };
    let chosen = match name {
        Some(name) => members
            .iter()
            .filter_map(|id| find(id))
            .find(|package| package["name"] == name),
        None => match metadata["resolve"]["root"].as_str() {
            Some(root) => find(root),
            None if members.len() == 1 => find(members[0]),
            None => None,
        },
    };
    chosen.ok_or_else(|| {
        Error::new(
            match name {
                Some(name) => format!("no workspace member is named {name:?}"),
                None => "the workspace has several packages".to_owned(),
            },
            "pass --package NAME",
        )
    })
}

/// The `wasm-bindgen` version the project's lockfile resolves for the wasm
/// target, if it depends on it.
pub fn locked_wasm_bindgen(metadata: &Value) -> Option<String> {
    let mut versions: Vec<&str> = metadata["packages"]
        .as_array()?
        .iter()
        .filter(|package| package["name"] == "wasm-bindgen")
        .filter_map(|package| package["version"].as_str())
        .collect();
    versions.sort_unstable();
    versions.dedup();
    match versions.as_slice() {
        [version] => Some((*version).to_owned()),
        _ => None,
    }
}

/// The installed `wasm-bindgen` CLI's version, such as `0.2.127`.
pub fn wasm_bindgen_cli_version() -> Option<String> {
    let output = Command::new("wasm-bindgen")
        .arg("--version")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()?
        .trim()
        .strip_prefix("wasm-bindgen ")
        .map(|version| {
            version
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_owned()
        })
}

/// Fail unless the `wasm-bindgen` CLI is exactly `expected`, the version of
/// the `wasm-bindgen` crate the project compiles against.
pub fn ensure_wasm_bindgen(expected: &str) -> Result<(), Error> {
    let install = format!("cargo install --locked --force wasm-bindgen-cli --version {expected}");
    match wasm_bindgen_cli_version() {
        Some(version) if version == expected => Ok(()),
        Some(version) => Err(Error::new(
            format!(
                "the wasm-bindgen CLI is {version}, but the project's wasm-bindgen crate is {expected}; they must match exactly"
            ),
            format!("run `{install}`"),
        )),
        None => Err(Error::new(
            format!("the wasm-bindgen CLI {expected} is required and was not found on PATH"),
            format!("run `rustup target add wasm32-unknown-unknown` and `{install}`"),
        )),
    }
}

/// Run `wasm-bindgen --target web` for `wasm` into a fresh `output`.
pub fn bindgen(wasm: &Path, output: &Path) -> Result<(), Error> {
    if output.exists() {
        fs::remove_dir_all(output).map_err(io(format!("clear {}", output.display())))?;
    }
    let status = Command::new("wasm-bindgen")
        .args(["--target", "web", "--out-dir"])
        .arg(output)
        .arg(wasm)
        .status()
        .map_err(io("run wasm-bindgen"))?;
    if !status.success() {
        return Err(Error::new(
            format!("wasm-bindgen failed for {}", wasm.display()),
            "",
        ));
    }
    Ok(())
}

/// Copy a web root's source files into `destination`, skipping hidden files,
/// `tests`, the names builds generate and `exclude`.
pub fn stage_web(source: &Path, destination: &Path, exclude: &[&str]) -> Result<(), Error> {
    fs::create_dir_all(destination).map_err(io(format!("create {}", destination.display())))?;
    let entries = fs::read_dir(source).map_err(io(format!("read {}", source.display())))?;
    for entry in entries {
        let entry = entry.map_err(io(format!("read {}", source.display())))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.')
            || name == "tests"
            || GENERATED.contains(&name.as_ref())
            || exclude.contains(&name.as_ref())
        {
            continue;
        }
        let kind = entry.file_type().map_err(io(format!("inspect {name}")))?;
        let target = destination.join(entry.file_name());
        if kind.is_dir() {
            stage_web(&entry.path(), &target, &[])?;
        } else if kind.is_file() {
            fs::copy(entry.path(), &target).map_err(io(format!("copy {name}")))?;
        } else {
            return Err(Error::new(
                format!("{} is not a regular file", entry.path().display()),
                "web roots hold regular files and directories only",
            ));
        }
    }
    Ok(())
}

/// Build the library for the wasm target and return the `.wasm` it produced.
fn build_wasm(options: &BuildOptions, package: &Value) -> Result<PathBuf, Error> {
    let mut command = cargo();
    command.args([
        "build",
        "--lib",
        "--target",
        "wasm32-unknown-unknown",
        "--message-format",
        "json-render-diagnostics",
    ]);
    if !options.debug {
        command.arg("--release");
    }
    if options.locked {
        command.arg("--locked");
    }
    if let Some(path) = &options.manifest_path {
        command.arg("--manifest-path").arg(path);
    }
    let name = package["name"].as_str().unwrap_or_default();
    command.args(["--package", name]);
    let mut child = command
        .stdout(Stdio::piped())
        .spawn()
        .map_err(io("run cargo build"))?;
    let mut wasm = None;
    let stdout = child.stdout.take().expect("piped stdout");
    for line in std::io::BufReader::new(stdout).lines() {
        let line = line.map_err(io("read cargo output"))?;
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if message["reason"] == "compiler-artifact" && message["package_id"] == package["id"] {
            let files = message["filenames"].as_array().cloned().unwrap_or_default();
            if let Some(file) = files
                .iter()
                .filter_map(Value::as_str)
                .find(|file| file.ends_with(".wasm"))
            {
                wasm = Some(PathBuf::from(file));
            }
        }
    }
    let status = child.wait().map_err(io("wait for cargo build"))?;
    if !status.success() {
        return Err(Error::new(
            "cargo build for wasm32-unknown-unknown failed",
            "fix the errors above; `rustup target add wasm32-unknown-unknown` adds a missing target",
        ));
    }
    wasm.ok_or_else(|| {
        Error::new(
            format!("{name} produced no .wasm"),
            "give the library `crate-type = [\"cdylib\", \"rlib\"]`",
        )
    })
}

fn git(directory: &Path, arguments: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(directory)
        .args(arguments)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Build one application's web root in place: `pkg/` from wasm-bindgen and
/// the startup loader beside `index.html`. With production options, also
/// package a clean copy of it.
pub fn build(options: &BuildOptions) -> Result<BuildReport, Error> {
    let metadata = cargo_metadata(options.manifest_path.as_deref())?;
    let package = application(&metadata, options.package.as_deref())?.clone();
    let name = package["name"].as_str().unwrap_or_default().to_owned();
    let manifest = PathBuf::from(package["manifest_path"].as_str().unwrap_or_default());
    let root = manifest.parent().unwrap_or(Path::new(".")).to_path_buf();
    let web_dir = options.web_dir.clone().unwrap_or_else(|| root.join("web"));
    if !web_dir.join("index.html").is_file() {
        return Err(Error::new(
            format!("{} has no index.html", web_dir.display()),
            "pass --web-dir DIR, the directory holding index.html and bootstrap.js",
        ));
    }
    let version = locked_wasm_bindgen(&metadata).ok_or_else(|| {
        Error::new(
            format!("{name} does not depend on exactly one wasm-bindgen version"),
            "add `wasm-bindgen = \"=0.2.127\"` (your wasm-bindgen CLI's version) under [target.'cfg(target_arch = \"wasm32\")'.dependencies]",
        )
    })?;
    ensure_wasm_bindgen(&version)?;
    let wasm = build_wasm(options, &package)?;
    bindgen(&wasm, &web_dir.join("pkg"))?;
    write_startup_loader(&web_dir)?;
    let production = match &options.production {
        None => None,
        Some(production) => {
            let target = PathBuf::from(metadata["target_directory"].as_str().unwrap_or("target"));
            let output = production
                .output
                .clone()
                .unwrap_or_else(|| target.join("web-production"));
            let staging = target.join("web-staging").join(&name);
            if staging.exists() {
                fs::remove_dir_all(&staging).map_err(io("clear web staging"))?;
            }
            stage_web(&web_dir, &staging, &[])?;
            write_startup_loader(&staging)?;
            bindgen(&wasm, &staging.join("pkg"))?;
            let rustc = Command::new("rustc")
                .arg("--version")
                .output()
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
                .unwrap_or_default();
            let identity = serde_json::json!({
                "revision": git(&root, &["rev-parse", "HEAD"]),
                "dirty": git(&root, &["status", "--porcelain"]).map(|status| !status.is_empty()),
                "rustc": rustc,
                "wasmBindgen": version,
                "profile": if options.debug { "dev" } else { "release" },
                "packageManifest": fs::read_to_string(&manifest).unwrap_or_default(),
                "target": "wasm32-unknown-unknown",
                "rustflags": std::env::var("RUSTFLAGS").ok(),
                "cargoEncodedRustflags": std::env::var("CARGO_ENCODED_RUSTFLAGS").ok(),
            });
            let manifest = package::package(&package::Options {
                apps: vec![package::App {
                    name: name.clone(),
                    directory: staging,
                }],
                output: output.clone(),
                variant: production.variant,
                wasm_opt: production
                    .wasm_opt
                    .clone()
                    .or_else(|| std::env::var_os("WASM_OPT").map(PathBuf::from))
                    .unwrap_or_else(|| "wasm-opt".into()),
                source_identity: identity,
            })?;
            Some((output, manifest))
        }
    };
    Ok(BuildReport {
        package: name,
        web_dir,
        wasm,
        wasm_bindgen: version,
        production,
    })
}
