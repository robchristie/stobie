//! Production packaging of already built wasm-bindgen web directories:
//! optional `wasm-opt`, Brotli and gzip representations and immutable,
//! content-addressed asset directories. No build, install or publication
//! happens here.

use std::collections::BTreeSet;
use std::fs;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{Error, pwa};

/// The Binaryen release whose `wasm-opt` the optimised variants require.
pub const WASM_OPT_VERSION: &str = "131";

/// Post-bindgen WASM processing: none, or `wasm-opt -Oz` or `-O3`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Variant {
    #[default]
    None,
    Oz,
    O3,
}

impl Variant {
    pub fn parse(value: &str) -> Result<Self, Error> {
        match value {
            "none" => Ok(Self::None),
            "Oz" => Ok(Self::Oz),
            "O3" => Ok(Self::O3),
            other => Err(Error::new(
                format!("Unknown variant {other}; expected none, Oz, O3"),
                "choose --variant none, Oz or O3",
            )),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Oz => "Oz",
            Self::O3 => "O3",
        }
    }
}

/// One application's complete bindgen web root, packaged under `name/`.
#[derive(Clone, Debug)]
pub struct App {
    pub name: String,
    pub directory: PathBuf,
}

#[derive(Clone, Debug)]
pub struct Options {
    pub apps: Vec<App>,
    pub output: PathBuf,
    pub variant: Variant,
    /// The `wasm-opt` executable for an optimised variant.
    pub wasm_opt: PathBuf,
    /// Build configuration and provenance. `revision` and `dirty` are
    /// recorded but do not change asset identity.
    pub source_identity: Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Describe {
    pub bytes: usize,
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InputRecord {
    pub path: String,
    pub bytes: usize,
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Representations {
    pub path: String,
    pub raw: Describe,
    pub br: Describe,
    pub gzip: Describe,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagedApp {
    pub name: String,
    pub input_identity: String,
    pub inputs: Vec<InputRecord>,
    pub content_id: String,
    pub asset_directory: String,
    pub entry: Representations,
    pub assets: Vec<Representations>,
    /// The manifest and service worker at the web root, beside the entry,
    /// for a page that links `manifest.webmanifest`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pwa: Option<PwaFiles>,
}

/// An installable application's mutable files at its web root.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PwaFiles {
    pub manifest: Representations,
    pub service_worker: Representations,
    /// Whether the service worker handles Web Push; recorded only when it does.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub web_push: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Compression {
    pub brotli_quality: u32,
    pub gzip_level: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub variant: &'static str,
    pub wasm_opt_args: Vec<String>,
    pub compression: Compression,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolVersions {
    /// The packager, which fixes the Brotli and gzip encoders.
    pub packager: String,
    pub wasm_opt: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub schema_version: u32,
    pub settings: Settings,
    pub tool_versions: ToolVersions,
    pub source_identity: Value,
    pub apps: Vec<PackagedApp>,
}

const BROTLI_QUALITY: u32 = 11;
const BROTLI_WINDOW: u32 = 22;
const GZIP_LEVEL: u32 = 9;
const MARKER: &str = ".browser-package";

pub fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn describe(bytes: &[u8]) -> Describe {
    Describe {
        bytes: bytes.len(),
        sha256: sha256(bytes),
    }
}

/// Two-space indented JSON with a trailing newline, the form identities hash.
fn serialise(value: &impl Serialize) -> String {
    let mut text = serde_json::to_string_pretty(value).expect("package records serialise");
    text.push('\n');
    text
}

fn io(context: impl std::fmt::Display) -> impl FnOnce(std::io::Error) -> Error {
    move |error| Error::new(format!("{context}: {error}"), "")
}

fn files_in(directory: &Path, prefix: &str, output: &mut Vec<String>) -> Result<(), Error> {
    let mut entries = fs::read_dir(directory)
        .map_err(io(format!("read {}", directory.display())))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(io(format!("read {}", directory.display())))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name == "tests" {
            continue;
        }
        let relative = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let full = entry.path();
        let kind = entry
            .file_type()
            .map_err(io(format!("inspect {}", full.display())))?;
        if kind.is_symlink() {
            return Err(Error::new(
                format!("Symlink is not a package input: {}", full.display()),
                "",
            ));
        }
        if kind.is_dir() {
            files_in(&full, &relative, output)?;
        } else if !kind.is_file() {
            return Err(Error::new(
                format!("Not a regular file: {}", full.display()),
                "",
            ));
        } else if name.ends_with(".br") || name.ends_with(".gz") {
            return Err(Error::new(
                format!("Precompressed input is not supported: {}", full.display()),
                "",
            ));
        } else {
            output.push(relative);
        }
    }
    Ok(())
}

/// The `wasm-opt` version line, which must be Binaryen 131.
pub fn optimiser_version(executable: &Path) -> Result<String, Error> {
    let output = Command::new(executable)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .ok_or_else(|| {
            Error::new(
                format!(
                    "wasm-opt {WASM_OPT_VERSION} is required: {} did not run",
                    executable.display()
                ),
                format!(
                    "install Binaryen {WASM_OPT_VERSION} and set WASM_OPT=/absolute/path/to/wasm-opt"
                ),
            )
        })?;
    let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let expected = format!("wasm-opt version {WASM_OPT_VERSION}");
    let matches = version
        .strip_prefix(&expected)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace));
    if !matches {
        return Err(Error::new(
            format!("Incompatible wasm-opt: expected {WASM_OPT_VERSION}, observed {version:?}"),
            format!("install Binaryen {WASM_OPT_VERSION}"),
        ));
    }
    Ok(version)
}

fn brotli(bytes: &[u8]) -> Vec<u8> {
    let parameters = brotli::enc::BrotliEncoderParams {
        quality: BROTLI_QUALITY as i32,
        lgwin: BROTLI_WINDOW as i32,
        ..Default::default()
    };
    let mut output = Vec::new();
    brotli::BrotliCompress(&mut &bytes[..], &mut output, &parameters)
        .expect("in-memory Brotli compression");
    output
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder =
        flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(GZIP_LEVEL));
    encoder.write_all(bytes).expect("in-memory gzip");
    encoder.finish().expect("in-memory gzip")
}

fn write_representations(
    directory: &Path,
    relative: &str,
    bytes: &[u8],
) -> Result<Representations, Error> {
    let destination = directory.join(relative);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(io(format!("create {}", parent.display())))?;
    }
    let br = brotli(bytes);
    let gz = gzip(bytes);
    let mut unbr = Vec::new();
    brotli::BrotliDecompress(&mut &br[..], &mut unbr)
        .map_err(io(format!("decompress {relative}")))?;
    let mut ungz = Vec::new();
    flate2::read::GzDecoder::new(&gz[..])
        .read_to_end(&mut ungz)
        .map_err(io(format!("decompress {relative}")))?;
    if unbr != bytes || ungz != bytes {
        return Err(Error::new(
            format!("Compression round-trip failed: {relative}"),
            "",
        ));
    }
    let suffixed = |suffix: &str| {
        let mut path = destination.clone().into_os_string();
        path.push(suffix);
        PathBuf::from(path)
    };
    fs::write(&destination, bytes).map_err(io(format!("write {}", destination.display())))?;
    fs::write(suffixed(".br"), &br).map_err(io(format!("write {relative}.br")))?;
    fs::write(suffixed(".gz"), &gz).map_err(io(format!("write {relative}.gz")))?;
    Ok(Representations {
        path: relative.to_owned(),
        raw: describe(bytes),
        br: describe(&br),
        gzip: describe(&gz),
    })
}

fn valid_name(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && characters.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn absolute(path: &Path) -> Result<PathBuf, Error> {
    std::path::absolute(path).map_err(io(format!("resolve {}", path.display())))
}

/// Insert `<base href="./assets/ID/">` after the head tag.
fn add_base(name: &str, html: &str, content_id: &str) -> Result<String, Error> {
    let lower = html.to_ascii_lowercase();
    let has_base = lower
        .match_indices("<base")
        .any(|(index, _)| !lower[index + 5..].starts_with(|c: char| c.is_ascii_alphanumeric()));
    let head = lower.match_indices("<head").find(|(index, _)| {
        lower[index + 5..].starts_with(|c: char| c == '>' || c.is_ascii_whitespace())
    });
    let (Some((start, _)), false) = (head, has_base) else {
        return Err(Error::new(
            format!("{name}: expected HTML head without an existing base"),
            "give index.html a <head> and no <base>",
        ));
    };
    let end = start + lower[start..].find('>').expect("a head tag closes") + 1;
    Ok(format!(
        "{}\n<base href=\"./assets/{content_id}/\">{}",
        &html[..end],
        &html[end..]
    ))
}

struct Staging(PathBuf);

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn staging_directory(parent: &Path) -> Result<Staging, Error> {
    for attempt in 0..100u32 {
        let candidate = parent.join(format!(".browser-package-{}-{attempt}", std::process::id()));
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(Staging(candidate)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io("create package staging")(error)),
        }
    }
    Err(Error::new("could not create package staging", ""))
}

/// Package complete bindgen web roots into `options.output`, replacing a
/// previous package there only once every step has succeeded.
pub fn package(options: &Options) -> Result<Manifest, Error> {
    if options.apps.is_empty() {
        return Err(Error::new("At least one app is required", ""));
    }
    let output = absolute(&options.output)?;
    let mut names = BTreeSet::new();
    for app in &options.apps {
        if !valid_name(&app.name) || !names.insert(app.name.clone()) {
            return Err(Error::new(
                format!("Invalid or duplicate app name: {}", app.name),
                "app names are lower-case letters, digits and hyphens",
            ));
        }
        let input = absolute(&app.directory)?;
        if input == output || input.starts_with(&output) || output.starts_with(&input) {
            return Err(Error::new(
                "Output must be isolated from inputs",
                "choose an output directory outside every web root",
            ));
        }
        let metadata =
            fs::symlink_metadata(&input).map_err(io(format!("read {}", input.display())))?;
        if metadata.file_type().is_symlink() {
            return Err(Error::new(
                format!("Symlink is not a package input: {}", input.display()),
                "",
            ));
        }
    }
    if let Ok(metadata) = fs::symlink_metadata(&output) {
        if metadata.file_type().is_symlink() {
            return Err(Error::new("Output must not be a symlink", ""));
        }
        let occupied = fs::read_dir(&output)
            .map_err(io(format!("read {}", output.display())))?
            .next()
            .is_some();
        if occupied && !output.join(MARKER).exists() {
            return Err(Error::new(
                format!("Refusing to replace unrelated output: {}", output.display()),
                "choose an empty directory or a previous package",
            ));
        }
    }
    let variant = options.variant;
    let wasm_opt_args: Vec<String> = match variant {
        Variant::None => Vec::new(),
        other => vec![format!("-{}", other.name())],
    };
    let settings = Settings {
        variant: variant.name(),
        wasm_opt_args: wasm_opt_args.clone(),
        compression: Compression {
            brotli_quality: BROTLI_QUALITY,
            gzip_level: GZIP_LEVEL,
        },
    };
    let tool_versions = ToolVersions {
        packager: format!("stobie-build {}", env!("CARGO_PKG_VERSION")),
        wasm_opt: match variant {
            Variant::None => None,
            _ => Some(optimiser_version(&options.wasm_opt)?),
        },
    };
    let parent = output
        .parent()
        .ok_or_else(|| Error::new("Output needs a parent", ""))?;
    fs::create_dir_all(parent).map_err(io(format!("create {}", parent.display())))?;
    let staging = staging_directory(parent)?;
    // Provenance changes alone must not invalidate otherwise identical public assets.
    let content_configuration = match &options.source_identity {
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(key, _)| !matches!(key.as_str(), "revision" | "dirty"))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        ),
        other => other.clone(),
    };
    let mut apps = options.apps.clone();
    apps.sort_by(|a, b| a.name.cmp(&b.name));
    let mut packaged = Vec::new();
    for app in apps {
        let mut inputs = Vec::new();
        files_in(&app.directory, "", &mut inputs)?;
        if !inputs.iter().any(|path| path == "index.html")
            || !inputs.iter().any(|path| path.ends_with(".wasm"))
        {
            return Err(Error::new(
                format!("{} requires index.html and built .wasm input", app.name),
                "build the application with wasm-bindgen first",
            ));
        }
        let html_path = app.directory.join("index.html");
        let html =
            fs::read_to_string(&html_path).map_err(io(format!("read {}", html_path.display())))?;
        let declaration = pwa::declaration(&html)
            .map_err(|error| Error::new(format!("{}: {}", app.name, error.message), error.hint))?;
        if let Some(declaration) = &declaration {
            declaration.check_icons(&app.directory)?;
        }
        let mut input_records = Vec::new();
        let mut assets: Vec<(String, Vec<u8>)> = Vec::new();
        for relative in &inputs {
            let path = app.directory.join(relative);
            let bytes = fs::read(&path).map_err(io(format!("read {}", path.display())))?;
            let described = describe(&bytes);
            input_records.push(InputRecord {
                path: relative.clone(),
                bytes: described.bytes,
                sha256: described.sha256,
            });
            // The manifest and service worker are written afresh at the web
            // root, never as hashed assets.
            let generated = [pwa::MANIFEST_FILE, pwa::SERVICE_WORKER_FILE];
            if relative == "index.html"
                || (declaration.is_some() && generated.contains(&relative.as_str()))
            {
                continue;
            }
            let bytes = if relative.ends_with(".wasm") && variant != Variant::None {
                optimise(
                    &options.wasm_opt,
                    &wasm_opt_args,
                    &staging.0,
                    &app.name,
                    relative,
                    &bytes,
                )?
            } else {
                bytes
            };
            assets.push((relative.clone(), bytes));
        }
        let input_identity = sha256(serialise(&input_records).as_bytes());
        let asset_records: Vec<InputRecord> = assets
            .iter()
            .map(|(path, bytes)| {
                let described = describe(bytes);
                InputRecord {
                    path: path.clone(),
                    bytes: described.bytes,
                    sha256: described.sha256,
                }
            })
            .collect();
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct ContentIdentity<'a> {
            schema_version: u32,
            input_identity: &'a str,
            content_configuration: &'a Value,
            settings: &'a Settings,
            tool_versions: &'a ToolVersions,
            assets: &'a [InputRecord],
        }
        let content_id = sha256(
            serialise(&ContentIdentity {
                schema_version: 1,
                input_identity: &input_identity,
                content_configuration: &content_configuration,
                settings: &settings,
                tool_versions: &tool_versions,
                assets: &asset_records,
            })
            .as_bytes(),
        );
        let asset_directory = format!("{}/assets/{content_id}", app.name);
        let records = assets
            .iter()
            .map(|(path, bytes)| {
                write_representations(&staging.0, &format!("{asset_directory}/{path}"), bytes)
            })
            .collect::<Result<Vec<_>, _>>()?;
        // The base points relative URLs into the asset directory, so the
        // manifest link climbs back to the web root.
        let html = match &declaration {
            Some(declaration) => {
                declaration.with_manifest_href(&html, &format!("../../{}", pwa::MANIFEST_FILE))
            }
            None => html,
        };
        let html = add_base(&app.name, &html, &content_id)?;
        let entry = write_representations(
            &staging.0,
            &format!("{}/index.html", app.name),
            html.as_bytes(),
        )?;
        let pwa = match &declaration {
            None => None,
            Some(declaration) => {
                let prefix = format!("assets/{content_id}/");
                let precache: Vec<String> = ["./".to_owned(), pwa::MANIFEST_FILE.to_owned()]
                    .into_iter()
                    .chain(assets.iter().map(|(path, _)| format!("{prefix}{path}")))
                    .collect();
                Some(PwaFiles {
                    manifest: write_representations(
                        &staging.0,
                        &format!("{}/{}", app.name, pwa::MANIFEST_FILE),
                        declaration.manifest(&prefix).as_bytes(),
                    )?,
                    service_worker: write_representations(
                        &staging.0,
                        &format!("{}/{}", app.name, pwa::SERVICE_WORKER_FILE),
                        declaration
                            .service_worker(
                                &app.name,
                                &content_id,
                                &precache,
                                &[pwa::MANIFEST_FILE.to_owned()],
                            )
                            .as_bytes(),
                    )?,
                    web_push: declaration.web_push,
                })
            }
        };
        packaged.push(PackagedApp {
            name: app.name,
            input_identity,
            inputs: input_records,
            content_id,
            asset_directory,
            entry,
            assets: records,
            pwa,
        });
    }
    let manifest = Manifest {
        schema_version: 1,
        settings,
        tool_versions,
        source_identity: options.source_identity.clone(),
        apps: packaged,
    };
    fs::write(staging.0.join("manifest.json"), serialise(&manifest))
        .map_err(io("write manifest.json"))?;
    fs::write(staging.0.join(MARKER), "1\n").map_err(io("write package marker"))?;
    // Keep the previous coherent package until all optimisation and
    // compression has succeeded.
    let mut previous = staging.0.clone().into_os_string();
    previous.push("-previous");
    let previous = PathBuf::from(previous);
    let had_previous = output.exists();
    if had_previous {
        fs::rename(&output, &previous).map_err(io("set aside the previous package"))?;
    }
    if let Err(error) = fs::rename(&staging.0, &output) {
        if had_previous {
            let _ = fs::rename(&previous, &output);
        }
        return Err(io("install the package")(error));
    }
    if had_previous {
        let _ = fs::remove_dir_all(&previous);
    }
    Ok(manifest)
}

fn optimise(
    wasm_opt: &Path,
    arguments: &[String],
    staging: &Path,
    app: &str,
    relative: &str,
    bytes: &[u8],
) -> Result<Vec<u8>, Error> {
    let work = staging.join(format!("{app}-work"));
    fs::create_dir_all(&work).map_err(io("create optimiser work"))?;
    let input = work.join("input.wasm");
    let output = work.join("output.wasm");
    fs::write(&input, bytes).map_err(io("write optimiser input"))?;
    let result = Command::new(wasm_opt)
        .arg(&input)
        .args(arguments)
        .arg("-o")
        .arg(&output)
        .output();
    let failed = |detail: String| {
        Error::new(
            format!("wasm-opt failed for {app}/{relative}: {detail}"),
            "",
        )
    };
    match result {
        Ok(result) if result.status.success() => {}
        Ok(result) => return Err(failed(String::from_utf8_lossy(&result.stderr).into_owned())),
        Err(error) => return Err(failed(error.to_string())),
    }
    let optimised = fs::read(&output).map_err(io("read optimiser output"))?;
    let _ = fs::remove_dir_all(&work);
    Ok(optimised)
}

#[cfg(test)]
mod tests;
