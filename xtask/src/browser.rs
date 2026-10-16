//! Production browser builds keep their bindgen staging separate from development output.
use std::{env, fs, path::Path, path::PathBuf, process::Command};

use anyhow::{Context, Result, bail};
use stobie_build::package::{self, Variant};

/// `$WASM_OPT`, the repository's pinned Binaryen, or `wasm-opt` on `PATH`.
fn wasm_opt() -> PathBuf {
    let pinned = Path::new(".tools/browser/binaryen-version_131/bin/wasm-opt");
    env::var_os("WASM_OPT").map_or_else(
        || {
            if pinned.exists() {
                pinned.to_path_buf()
            } else {
                "wasm-opt".into()
            }
        },
        PathBuf::from,
    )
}

pub fn build(arguments: Vec<String>) -> Result<()> {
    let mut variant = "none".to_owned();
    let mut output = "target/browser-production".to_owned();
    let mut index = 0;
    while index < arguments.len() {
        let value = arguments
            .get(index + 1)
            .context("browser option requires a value")?;
        match arguments[index].as_str() {
            "--variant" => variant.clone_from(value),
            "--output" => output.clone_from(value),
            flag => bail!("unknown production browser option {flag}"),
        }
        index += 2;
    }
    let variant = Variant::parse(&variant)?;
    if variant != Variant::None {
        // Fail before the expensive build when the optimiser is unusable.
        package::optimiser_version(&wasm_opt())?;
    }
    let wasm_bindgen = super::ensure_wasm_bindgen()?;
    let rustc = Command::new("rustc").arg("--version").output()?;
    let rustc = String::from_utf8(rustc.stdout)?;
    if !rustc.starts_with("rustc 1.99.0 ") {
        bail!("production browser builds require repository Rust 1.99.0; observed {rustc:?}");
    }
    let staging = Path::new("target/browser-staging");
    if staging.exists() {
        fs::remove_dir_all(staging)?;
    }
    fs::create_dir_all(staging)?;
    super::run(
        "cargo",
        &[
            "build",
            "--locked",
            "--release",
            "--lib",
            "--target-dir",
            "target/browser-cargo",
            "--target",
            "wasm32-unknown-unknown",
            "-p",
            "analytical-workspace-lab",
            "-p",
            "stobie-gallery",
            "-p",
            "stobie-tile-worker",
            "-p",
            "emuella-viewer",
        ],
    )?;
    let mut apps = Vec::new();
    for (name, app, binary) in [
        (
            "lab",
            "analytical-workspace-lab",
            "analytical_workspace_lab",
        ),
        ("gallery", "stobie-gallery", "stobie_gallery"),
        ("viewer", "emuella-viewer", "emuella_viewer"),
    ] {
        let destination = staging.join(name);
        // The web root's sources only; a development build's generated
        // packages and loader are replaced by fresh ones.
        stobie_build::stage_web(
            Path::new(&format!("apps/{app}/web")),
            &destination,
            &["worker-pkg"],
        )?;
        stobie_build::write_startup_loader(&destination)?;
        bindgen(binary, &destination.join("pkg"))?;
        if name == "lab" {
            bindgen("stobie_tile_worker", &destination.join("worker-pkg"))?;
        }
        apps.push(package::App {
            name: name.into(),
            directory: destination,
        });
    }
    let revision = Command::new("git").args(["rev-parse", "HEAD"]).output()?;
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()?;
    let identity = serde_json::json!({
        "revision": String::from_utf8(revision.stdout)?.trim(),
        "dirty": !dirty.stdout.is_empty(), "rustc": rustc.trim(), "wasmBindgen": wasm_bindgen,
        "profile": "release", "workspaceManifest": fs::read_to_string("Cargo.toml")?,
        "profileEnvironment": env::vars().filter(|(key, _)| key.starts_with("CARGO_PROFILE_RELEASE_")).collect::<std::collections::BTreeMap<_, _>>(),
        "target":"wasm32-unknown-unknown", "browserTestApi": true,
        "rustflags":env::var("RUSTFLAGS").ok(), "cargoEncodedRustflags":env::var("CARGO_ENCODED_RUSTFLAGS").ok(),
    });
    fs::write(
        staging.join("build-identity.json"),
        serde_json::to_vec_pretty(&identity)?,
    )?;
    println!("+ package {} into {output}", staging.display());
    let manifest = package::package(&package::Options {
        apps,
        output: output.clone().into(),
        variant,
        wasm_opt: wasm_opt(),
        source_identity: identity,
    })?;
    // Validation targets the post-processed artifact, never a pre-optimisation build.
    let viewer = manifest
        .apps
        .iter()
        .find(|app| app.name == "viewer")
        .context("packaged viewer")?;
    let viewer_root = Path::new(&output).join(&viewer.asset_directory);
    super::run(
        "node",
        &[
            "tools/viewer-response-headers.mjs",
            viewer_root.to_str().context("UTF-8 viewer root")?,
        ],
    )?;
    Ok(())
}

fn bindgen(binary: &str, destination: &Path) -> Result<()> {
    super::bindgen(
        &format!("target/browser-cargo/wasm32-unknown-unknown/release/{binary}.wasm"),
        destination,
    )
}
