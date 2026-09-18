//! Production browser builds keep their bindgen staging separate from development output.
use std::{env, fs, path::Path, process::Command};

use anyhow::{Context, Result, bail};

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
    if !["none", "Oz", "O3"].contains(&variant.as_str()) {
        bail!("browser variant must be none, Oz or O3");
    }
    if variant != "none" {
        super::run("node", &["tools/browser-package.mjs", "--check-optimiser"])?;
    }
    super::ensure_wasm_bindgen_version("0.2.127")?;
    let rustc = Command::new("rustc").arg("--version").output()?;
    let rustc = String::from_utf8(rustc.stdout)?;
    if !rustc.starts_with("rustc 1.97.1 ") {
        bail!("production browser builds require repository Rust 1.97.1; observed {rustc:?}");
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
            "polyorama-gallery",
            "-p",
            "polyorama-tile-worker",
            "-p",
            "emuella-viewer",
        ],
    )?;
    for (name, app, binary) in [
        (
            "lab",
            "analytical-workspace-lab",
            "analytical_workspace_lab",
        ),
        ("gallery", "polyorama-gallery", "polyorama_gallery"),
        ("viewer", "emuella-viewer", "emuella_viewer"),
    ] {
        let destination = staging.join(name);
        fs::create_dir_all(&destination)?;
        for entry in fs::read_dir(format!("apps/{app}/web"))? {
            let entry = entry?;
            if entry.file_type()?.is_file()
                && matches!(
                    entry.path().extension().and_then(|v| v.to_str()),
                    Some("html" | "js" | "css")
                )
            {
                fs::copy(entry.path(), destination.join(entry.file_name()))?;
            }
        }
        fs::copy(
            "tools/browser-startup.js",
            destination.join("browser-startup.js"),
        )?;
        bindgen(binary, &destination.join("pkg"))?;
        if name == "lab" {
            bindgen("polyorama_tile_worker", &destination.join("worker-pkg"))?;
        }
    }
    let revision = Command::new("git").args(["rev-parse", "HEAD"]).output()?;
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()?;
    let identity = serde_json::json!({
        "revision": String::from_utf8(revision.stdout)?.trim(),
        "dirty": !dirty.stdout.is_empty(), "rustc": rustc.trim(), "wasmBindgen": "0.2.127",
        "profile": "release", "workspaceManifest": fs::read_to_string("Cargo.toml")?,
        "profileEnvironment": env::vars().filter(|(key, _)| key.starts_with("CARGO_PROFILE_RELEASE_")).collect::<std::collections::BTreeMap<_, _>>(),
        "target":"wasm32-unknown-unknown", "browserTestApi": true,
        "rustflags":env::var("RUSTFLAGS").ok(), "cargoEncodedRustflags":env::var("CARGO_ENCODED_RUSTFLAGS").ok(),
    });
    let identity_path = staging.join("build-identity.json");
    fs::write(&identity_path, serde_json::to_vec_pretty(&identity)?)?;
    super::run(
        "node",
        &[
            "tools/browser-package.mjs",
            "--app",
            "lab=target/browser-staging/lab",
            "--app",
            "gallery=target/browser-staging/gallery",
            "--app",
            "viewer=target/browser-staging/viewer",
            "--output",
            &output,
            "--variant",
            &variant,
            "--source-identity-file",
            identity_path.to_str().context("UTF-8 identity path")?,
        ],
    )?;
    // Validation targets the post-processed artifact, never a pre-optimisation build.
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(Path::new(&output).join("manifest.json"))?)?;
    let viewer = manifest["apps"]
        .as_array()
        .context("package apps")?
        .iter()
        .find(|app| app["name"] == "viewer")
        .context("packaged viewer")?;
    let viewer_root =
        Path::new(&output).join(viewer["assetDirectory"].as_str().context("viewer assets")?);
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
    super::run(
        "wasm-bindgen",
        &[
            "--target",
            "web",
            "--out-dir",
            destination.to_str().context("UTF-8 bindgen path")?,
            &format!("target/browser-cargo/wasm32-unknown-unknown/release/{binary}.wasm"),
        ],
    )
}
