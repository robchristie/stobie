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

/// `stobie --version`: the package version and, when known, the commit it
/// was built from, which `stobie new` pins and consumers check.
pub fn version_line() -> String {
    match source_revision() {
        Some(revision) => format!("stobie {} (commit {revision})", env!("CARGO_PKG_VERSION")),
        None => format!("stobie {}", env!("CARGO_PKG_VERSION")),
    }
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
    /// Run only in the browser: natively the UI crate builds without eframe,
    /// for host tests, and there is no native entry point.
    pub browser_only: bool,
    /// Join the enclosing Cargo workspace instead of declaring one.
    pub member: bool,
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

    fn pin(&self) -> String {
        format!("git = \"{STOBIE_GIT}\", rev = \"{}\"", self.revision)
    }

    /// The UI crate's dependency line with `features`, without its defaults.
    fn ui_crate(&self, features: &[&str]) -> String {
        let pin = self.pin();
        if features.is_empty() {
            return format!("stobie-ui-egui = {{ {pin}, default-features = false }}\n");
        }
        let features = features
            .iter()
            .map(|feature| format!("\"{feature}\""))
            .collect::<Vec<_>>()
            .join(", ");
        format!("stobie-ui-egui = {{ {pin}, default-features = false, features = [{features}] }}\n")
    }

    /// The `[dependencies]` lines for Stobie and, unless the application is
    /// browser-only, eframe with its native window backends.
    fn dependencies(&self) -> String {
        let mut features = Vec::new();
        if !self.browser_only {
            features.push("eframe");
        }
        if self.imaging {
            features.push("image-rendering");
        }
        let mut text = String::new();
        for name in self.stobie_crates() {
            if name == "stobie-ui-egui" {
                text.push_str(&self.ui_crate(&features));
            } else {
                text.push_str(&format!("{name} = {{ {} }}\n", self.pin()));
            }
        }
        if !self.browser_only {
            text.push_str("eframe = { version = \"=0.36.1\", default-features = false, features = [\"accesskit\", \"default_fonts\", \"wgpu\", \"x11\", \"wayland\"] }\n");
        }
        text
    }

    /// The `wasm32` dependency lines after `wasm-bindgen`: for a browser-only
    /// application, the UI crate's eframe entry points and eframe itself,
    /// which the native build leaves out.
    fn web_dependencies(&self) -> String {
        if !self.browser_only {
            return String::new();
        }
        let mut features = vec!["eframe"];
        if self.imaging {
            features.push("image-rendering");
        }
        let mut text = self.ui_crate(&features);
        text.push_str("eframe = { version = \"=0.36.1\", default-features = false, features = [\"accesskit\", \"default_fonts\", \"wgpu\"] }\n");
        text
    }

    /// Whether a template section applies: `native` and `browser_only`
    /// select by target, `standalone` and `member` by workspace.
    fn section(&self, name: &str) -> bool {
        match name {
            "native" => !self.browser_only,
            "browser_only" => self.browser_only,
            "standalone" => !self.member,
            "member" => self.member,
            _ => panic!("unknown template section {name:?}"),
        }
    }

    /// Keep the lines of applicable `{{#name}}`…`{{/name}}` sections, each
    /// marker on a line of its own, and drop the markers.
    fn sections(&self, text: &str) -> String {
        let mut kept = String::new();
        let mut open: Vec<(&str, bool)> = Vec::new();
        for line in text.split_inclusive('\n') {
            let marker = line.trim_end();
            if let Some(name) = marker
                .strip_prefix("{{#")
                .and_then(|m| m.strip_suffix("}}"))
            {
                open.push((name, self.section(name)));
            } else if let Some(name) = marker
                .strip_prefix("{{/")
                .and_then(|m| m.strip_suffix("}}"))
            {
                let (opened, _) = open.pop().expect("a section closes only after it opens");
                assert_eq!(opened, name, "template sections nest");
            } else if open.iter().all(|(_, keep)| *keep) {
                kept.push_str(line);
            }
        }
        assert!(open.is_empty(), "every template section closes");
        kept
    }

    /// Every generated file, as a path relative to the application and its text.
    pub fn files(&self) -> Vec<(&'static str, String)> {
        TEMPLATE
            .iter()
            .filter(|(path, _)| match *path {
                "src/main.rs" => !self.browser_only,
                // A member builds with the workspace's toolchain.
                "rust-toolchain.toml" => !self.member,
                _ => true,
            })
            .map(|(path, text)| {
                let text = self
                    .sections(text)
                    .replace("{{stobie_dependencies}}", &self.dependencies())
                    .replace("{{web_dependencies}}\n", &self.web_dependencies())
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

    /// Placeholder PNG icons for the installable page, at the paths and sizes
    /// `web/index.html` names, for the application to replace.
    pub fn icons(&self) -> Vec<(&'static str, Vec<u8>)> {
        let (background, foreground) = ([0x0b, 0x0f, 0x12], [0x4f, 0xb3, 0xbf]);
        [
            ("web/icons/icon-192.png", 192),
            ("web/icons/icon-512.png", 512),
            ("web/icons/apple-touch-icon.png", 180),
        ]
        .into_iter()
        .map(|(path, size)| {
            (
                path,
                stobie_build::pwa::placeholder_icon(size, background, foreground),
            )
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
            "{name:?} cannot name an application: use at most 64 lower-case letters, digits and single hyphens, starting with a letter, such as my-notes"
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

/// The nearest ancestor of `directory` whose `Cargo.toml` declares a
/// `[workspace]` table (or one of its subtables, such as
/// `[workspace.dependencies]`), as Cargo finds a package's workspace.
pub fn enclosing_workspace(directory: &Path) -> Option<PathBuf> {
    directory.ancestors().skip(1).find_map(|ancestor| {
        let manifest = ancestor.join("Cargo.toml");
        let text = fs::read_to_string(&manifest).ok()?;
        text.lines()
            .map(str::trim)
            .any(|line| line == "[workspace]" || line.starts_with("[workspace."))
            .then_some(manifest)
    })
}

#[derive(Serialize)]
struct Created<'a> {
    path: &'a Path,
    name: &'a str,
    stobie_rev: &'a str,
    imaging: bool,
    browser_only: bool,
    /// The enclosing workspace's manifest, for `--member`.
    workspace: Option<&'a Path>,
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
        browser_only: args.has("--browser-only"),
        member: args.has("--member"),
    };
    let directory = PathBuf::from(args.value("--path").unwrap_or(name));
    let absolute = std::path::absolute(&directory)
        .map_err(|error| ClientError::usage(format!("--path {}: {error}", directory.display())))?;
    let workspace = match app.member {
        false => None,
        true => Some(enclosing_workspace(&absolute).ok_or_else(|| {
            ClientError::usage(format!(
                "--member needs a directory inside a Cargo workspace, but no Cargo.toml above {} declares [workspace]",
                absolute.display()
            ))
        })?),
    };
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
    let icons = app.icons();
    let contents = files
        .iter()
        .map(|(relative, text)| (*relative, text.as_bytes()))
        .chain(
            icons
                .iter()
                .map(|(relative, png)| (*relative, png.as_slice())),
        );
    for (relative, bytes) in contents {
        let path = directory.join(relative);
        fs::create_dir_all(path.parent().expect("files have parents")).map_err(io(&path))?;
        fs::write(&path, bytes).map_err(io(&path))?;
        written.push(relative);
    }
    let directory = fs::canonicalize(&directory).unwrap_or(directory);
    let mut next = Vec::new();
    if let Some(workspace) = &workspace {
        let root = workspace.parent().expect("a manifest has a directory");
        let root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let member = directory.strip_prefix(&root).unwrap_or(&directory);
        next.push(format!(
            "add \"{}\" to [workspace] members in {}",
            member.display(),
            workspace.display()
        ));
    }
    next.push(format!("cd {}", directory.display()));
    if app.browser_only {
        next.push("cargo test".to_owned());
        next.push("stobie build-web && stobie serve".to_owned());
    } else {
        next.push("cargo run --release".to_owned());
        next.push("stobie build-web && stobie serve".to_owned());
        next.push("stobie launch app --manifest-path Cargo.toml".to_owned());
    }
    let mut text = format!(
        "created {} in {}\n  Stobie {} (git, pinned)\n",
        app.title(),
        directory.display(),
        &app.revision[..12]
    );
    if app.browser_only {
        text.push_str("  browser-only: natively the library builds without eframe, for tests\n");
    }
    if let Some(workspace) = &workspace {
        text.push_str(&format!(
            "  a member of the workspace in {}; README.md suggests a [profile.web] for it\n",
            workspace.display()
        ));
    }
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
            browser_only: app.browser_only,
            workspace: workspace.as_deref(),
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
    /// The Cargo profile: `release`, `dev` or the `--profile` name.
    profile: &'a str,
    production: Option<Production<'a>>,
}

#[derive(Serialize)]
struct Production<'a> {
    output: &'a Path,
    content_id: &'a str,
    entry: String,
    /// The packaged application's transfer sizes.
    size: stobie_build::size::AppSize,
}

/// One line of a packaged application's sizes, in bytes.
fn size_line(size: &stobie_build::size::AppSize) -> String {
    format!(
        "  {}: wasm {} bytes ({} Brotli), {} files {} bytes ({} Brotli)\n",
        size.name, size.wasm.raw, size.wasm.br, size.files, size.total.raw, size.total.br
    )
}

fn sizes(manifest: &package::Manifest) -> Result<Vec<stobie_build::size::AppSize>, ClientError> {
    let manifest = serde_json::to_value(manifest).expect("a manifest serialises");
    stobie_build::size::of_manifest(&manifest).map_err(build_error)
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
    let profile = args.value("--profile");
    if profile.is_some() && args.has("--debug") {
        return Err(ClientError::usage(
            "--debug builds with the dev profile; pass --debug or --profile, not both",
        ));
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
    let report = stobie_build::build_with_profile(&options, profile).map_err(build_error)?;
    let profile = match (options.debug, profile) {
        (true, _) => "dev",
        (false, profile) => profile.unwrap_or("release"),
    };
    let production = report
        .production
        .as_ref()
        .map(|(output, manifest)| -> Result<_, ClientError> {
            let app = &manifest.apps[0];
            Ok(Production {
                output,
                content_id: &app.content_id,
                entry: format!("{}/", app.name),
                size: sizes(manifest)?.remove(0),
            })
        })
        .transpose()?;
    let mut text = format!(
        "built {} for the web in {} (profile {profile}, wasm-bindgen {})\n",
        report.package,
        report.web_dir.display(),
        report.wasm_bindgen
    );
    match &production {
        Some(production) => {
            text.push_str(&format!(
                "packaged {} under {} (assets {})\n",
                production.entry,
                production.output.display(),
                &production.content_id[..12]
            ));
            text.push_str(&size_line(&production.size));
        }
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
            profile,
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
    let sizes = sizes(&manifest)?;
    let text = manifest
        .apps
        .iter()
        .zip(&sizes)
        .map(|(app, size)| {
            format!(
                "packaged {}/ under {output} (assets {})\n{}",
                app.name,
                &app.content_id[..12],
                size_line(size)
            )
        })
        .collect();
    Ok(Output::new(
        text,
        serde_json::json!({ "manifest": manifest, "sizes": sizes }),
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
            browser_only: false,
            member: false,
        }
    }

    /// Every combination of `--imaging`, `--browser-only` and `--member`.
    fn shapes() -> Vec<NewApp> {
        let mut shapes = Vec::new();
        for imaging in [false, true] {
            for browser_only in [false, true] {
                for member in [false, true] {
                    shapes.push(NewApp {
                        browser_only,
                        member,
                        ..app(imaging)
                    });
                }
            }
        }
        shapes
    }

    fn file<'a>(files: &'a [(&'static str, String)], path: &str) -> Option<&'a str> {
        files
            .iter()
            .find(|(candidate, _)| *candidate == path)
            .map(|(_, text)| text.as_str())
    }

    fn test_directory(name: &str) -> PathBuf {
        // Generated manifests declare or join their own workspace, so an
        // enclosing one (with TMPDIR inside a checkout) does not count.
        let directory = std::env::temp_dir()
            .join("stobie-cli-tests")
            .join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        fs::canonicalize(directory).unwrap()
    }

    /// Cargo's reading of a generated manifest, without resolving or
    /// fetching its dependencies: each dependency's name, platform and
    /// whether it keeps its default features, with its features.
    fn cargo_dependencies(app: &NewApp) -> Vec<(String, Option<String>, bool, Vec<String>)> {
        let directory = test_directory(&format!(
            "manifest-{}-{}-{}",
            app.imaging, app.browser_only, app.member
        ));
        let package = if app.member {
            fs::write(
                directory.join("Cargo.toml"),
                "[workspace]\nmembers = [\"apps/field-notes\"]\nresolver = \"3\"\n",
            )
            .unwrap();
            directory.join("apps/field-notes")
        } else {
            directory.clone()
        };
        fs::create_dir_all(package.join("src")).unwrap();
        fs::write(package.join("src/lib.rs"), "").unwrap();
        fs::write(
            package.join("Cargo.toml"),
            file(&app.files(), "Cargo.toml").unwrap(),
        )
        .unwrap();
        let output = std::process::Command::new(env!("CARGO"))
            .args([
                "metadata",
                "--no-deps",
                "--offline",
                "--format-version",
                "1",
            ])
            .arg("--manifest-path")
            .arg(package.join("Cargo.toml"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            fs::canonicalize(metadata["workspace_root"].as_str().unwrap()).unwrap(),
            directory,
            "a member joins the enclosing workspace; a standalone application is its own"
        );
        let dependencies = metadata["packages"][0]["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dependency| {
                (
                    dependency["name"].as_str().unwrap().to_owned(),
                    dependency["target"].as_str().map(str::to_owned),
                    dependency["uses_default_features"].as_bool().unwrap(),
                    dependency["features"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|feature| feature.as_str().unwrap().to_owned())
                        .collect(),
                )
            })
            .collect();
        fs::remove_dir_all(&directory).unwrap();
        dependencies
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
        for app in shapes() {
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
            assert_eq!(manifest.contains("image-rendering"), app.imaging);
            assert_eq!(manifest.contains("stobie-imaging"), app.imaging);
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

    /// The generated page is installable: its head declares what
    /// `stobie build-web` needs, and the generated icons satisfy it.
    #[test]
    fn the_template_page_and_icons_are_installable() {
        let app = app(false);
        let files = app.files();
        let page = &files
            .iter()
            .find(|(path, _)| *path == "web/index.html")
            .unwrap()
            .1;
        let declared = stobie_build::pwa::declaration(page)
            .unwrap()
            .expect("the template links a manifest");
        assert_eq!(declared.short_name, "Field Notes");
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/stobie-cli-tests")
            .join(format!("icons-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        for (path, png) in app.icons() {
            let path = directory.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, png).unwrap();
        }
        declared.check_icons(&directory.join("web")).unwrap();
        fs::remove_dir_all(&directory).unwrap();
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
    fn the_version_names_the_build_commit() {
        let line = version_line();
        assert!(line.starts_with(&format!("stobie {}", env!("CARGO_PKG_VERSION"))));
        match source_revision() {
            Some(revision) => assert!(line.ends_with(&format!(" (commit {revision})"))),
            None => assert!(!line.contains("commit")),
        }
    }

    /// Generated sources pass `cargo fmt --check` whatever the name's length.
    /// Skipped where rustfmt is not installed; CI has it.
    /// A browser-only application has no native entry point, and its
    /// native build has no window system: eframe and the UI crate's
    /// `eframe` feature are declared only for `wasm32`.
    #[test]
    fn a_browser_only_application_builds_eframe_only_for_the_browser() {
        for imaging in [false, true] {
            let app = NewApp {
                browser_only: true,
                ..app(imaging)
            };
            let files = app.files();
            assert!(file(&files, "src/main.rs").is_none());
            let readme = file(&files, "README.md").unwrap();
            assert!(!readme.contains("cargo run"));
            assert!(readme.contains("stobie build-web"));
            let dependencies = cargo_dependencies(&app);
            let wasm = Some("cfg(target_arch = \"wasm32\")".to_owned());
            for (name, target, default_features, features) in &dependencies {
                if name == "eframe" {
                    assert_eq!(target, &wasm, "eframe is only for the browser");
                    assert!(!default_features);
                    for feature in ["accesskit", "default_fonts", "wgpu"] {
                        assert!(features.contains(&feature.to_owned()), "{feature}");
                    }
                    assert!(!features.iter().any(|f| f == "x11" || f == "wayland"));
                }
                if name == "stobie-ui-egui" && target.is_none() {
                    assert!(!default_features);
                    assert!(!features.iter().any(|f| f == "eframe"));
                    assert_eq!(features.iter().any(|f| f == "image-rendering"), imaging);
                }
                if name == "stobie-ui-egui" && target == &wasm {
                    assert!(features.iter().any(|f| f == "eframe"));
                    assert_eq!(features.iter().any(|f| f == "image-rendering"), imaging);
                }
            }
            for (name, target) in [("eframe", &wasm), ("stobie-ui-egui", &wasm)] {
                assert!(
                    dependencies.iter().any(|d| d.0 == name && &d.1 == target),
                    "{name} for {target:?}"
                );
            }
            assert!(
                !dependencies
                    .iter()
                    .any(|d| d.0 == "eframe" && d.1.is_none()),
                "no native eframe"
            );
        }
        // The ordinary application keeps its native window.
        let dependencies = cargo_dependencies(&app(false));
        assert!(
            dependencies
                .iter()
                .any(|d| d.0 == "eframe" && d.1.is_none() && d.3.contains(&"x11".to_owned()))
        );
    }

    /// A member of an existing workspace leaves the workspace, its
    /// profiles and its toolchain to the workspace.
    #[test]
    fn a_member_leaves_profiles_and_toolchain_to_its_workspace() {
        for app in shapes() {
            let files = app.files();
            let manifest = file(&files, "Cargo.toml").unwrap();
            let readme = file(&files, "README.md").unwrap();
            assert_eq!(manifest.contains("[workspace]"), !app.member);
            assert_eq!(manifest.contains("[profile."), !app.member);
            assert_eq!(file(&files, "rust-toolchain.toml").is_none(), app.member);
            assert_eq!(readme.contains("## In a Cargo workspace"), app.member);
            assert_eq!(file(&files, "src/main.rs").is_none(), app.browser_only);
            // Cargo reads the manifest in place: standalone, or as a
            // member of the enclosing workspace.
            let dependencies = cargo_dependencies(&app);
            assert!(dependencies.iter().any(|d| d.0 == "stobie-ui-egui"));
        }
    }

    #[test]
    fn the_enclosing_workspace_is_the_nearest_declared_one_above() {
        let root = test_directory("enclosing");
        let nested = root.join("apps/inner");
        fs::create_dir_all(&nested).unwrap();
        // Whatever encloses the temporary directory, such as a checkout
        // when verification points TMPDIR inside it.
        let outside = enclosing_workspace(&root);
        assert_eq!(enclosing_workspace(&nested), outside);
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        assert_eq!(
            enclosing_workspace(&nested),
            outside,
            "a package is not a workspace"
        );
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"x\"\n\n[workspace.dependencies]\nserde = \"1\"\n",
        )
        .unwrap();
        assert_eq!(enclosing_workspace(&nested), Some(root.join("Cargo.toml")));
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
        assert_eq!(enclosing_workspace(&nested), Some(root.join("Cargo.toml")));
        fs::write(nested.join("Cargo.toml"), "[workspace]\n").unwrap();
        assert_eq!(
            enclosing_workspace(&nested),
            Some(root.join("Cargo.toml")),
            "the directory itself is the new application, not its workspace"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn generated_sources_are_rustfmt_clean() {
        use std::io::Write as _;
        use std::process::{Command, Stdio};
        if Command::new("rustfmt").arg("--version").output().is_err() {
            return;
        }
        let names = ["a", "field-notes", &format!("a{}", "-b".repeat(31))];
        let apps = names
            .iter()
            .map(|name| NewApp {
                name: (*name).to_owned(),
                ..app(false)
            })
            .chain(shapes());
        for app in apps {
            let name = &app.name;
            assert!(valid_name(name).is_ok(), "{name}");
            for (path, text) in app.files() {
                if !path.ends_with(".rs") {
                    continue;
                }
                // `--check` on standard input always succeeds, so compare.
                let mut child = Command::new("rustfmt")
                    .args(["--edition", "2024", "--emit", "stdout"])
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                child
                    .stdin
                    .take()
                    .unwrap()
                    .write_all(text.as_bytes())
                    .unwrap();
                let output = child.wait_with_output().unwrap();
                assert!(output.status.success(), "{name}: rustfmt failed on {path}");
                assert_eq!(
                    String::from_utf8_lossy(&output.stdout),
                    text,
                    "{name}: {path} is not rustfmt-clean"
                );
            }
        }
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
