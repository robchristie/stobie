use std::{env, fs, path::Path, process::Command};

use anyhow::{Context, Result, bail};

fn main() -> Result<()> {
    let command = env::args().nth(1).unwrap_or_else(|| "help".into());
    match command.as_str() {
        "verify" => verify(),
        "build-web" => build_web(),
        "architecture" => architecture(),
        _ => {
            println!(
                "cargo xtask verify      run the complete native/browser verification surface"
            );
            println!("cargo xtask build-web   build release WASM application and Worker packages");
            println!("cargo xtask architecture check dependency boundaries");
            Ok(())
        }
    }
}

fn verify() -> Result<()> {
    run("cargo", &["fmt", "--all", "--check"])?;
    run(
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run(
        "cargo",
        &[
            "clippy",
            "--target",
            "wasm32-unknown-unknown",
            "-p",
            "analytical-workspace-lab",
            "-p",
            "polyorama-tile-worker",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run("cargo", &["test", "--workspace"])?;
    architecture()?;
    run("cargo", &["build", "--workspace", "--release"])?;
    build_web()?;
    if cfg!(target_os = "linux") {
        run("bash", &["tools/bootstrap-linux-ui.sh"])?;
    }
    run("npm", &["ci"])?;
    run("npx", &["playwright", "install", "chromium"])?;
    run("npm", &["run", "browser-smoke"])?;
    if cfg!(target_os = "linux") {
        run("bash", &["tools/native-smoke.sh"])?;
    }
    println!(
        "Polyorama verification passed: format, lint, tests, architecture, release native, release WASM, browser and native runtime smoke"
    );
    Ok(())
}

fn build_web() -> Result<()> {
    ensure_wasm_bindgen_version("0.2.127")?;
    run(
        "cargo",
        &[
            "build",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "-p",
            "analytical-workspace-lab",
            "-p",
            "polyorama-tile-worker",
        ],
    )?;
    run(
        "wasm-bindgen",
        &[
            "--target",
            "web",
            "--out-dir",
            "apps/analytical-workspace-lab/web/pkg",
            "target/wasm32-unknown-unknown/release/analytical_workspace_lab.wasm",
        ],
    )?;
    run(
        "wasm-bindgen",
        &[
            "--target",
            "web",
            "--out-dir",
            "apps/analytical-workspace-lab/web/worker-pkg",
            "target/wasm32-unknown-unknown/release/polyorama_tile_worker.wasm",
        ],
    )?;
    Ok(())
}

fn architecture() -> Result<()> {
    assert_tree_excludes(
        "workspace-core",
        &["egui", "eframe", "wgpu", "web-sys", "winit"],
    )?;
    assert_tree_excludes("workspace-runtime", &["egui", "eframe", "wgpu"])?;
    let ui_source = fs::read_to_string("apps/analytical-workspace-lab/src/panes.rs")?;
    for forbidden in [
        "&mut AppModel",
        "&mut Runtime",
        "&mut Workspace",
        "&wgpu::Device",
        "&wgpu::Queue",
    ] {
        if ui_source.contains(forbidden) {
            bail!("pane API contains forbidden broad access: {forbidden}");
        }
    }
    let renderer_source = fs::read_to_string("crates/workspace-render-wgpu/src/lib.rs")?;
    if renderer_source.contains("create_device") {
        bail!("renderer or viewport creates an additional wgpu device");
    }
    let canonical_definitions = [
        "crates/workspace-core/src/dock.rs",
        "crates/workspace-ui-egui/src/lib.rs",
    ]
    .iter()
    .map(fs::read_to_string)
    .collect::<std::io::Result<Vec<_>>>()?
    .iter()
    .map(|source| source.matches("pub struct Workspace").count())
    .sum::<usize>();
    if canonical_definitions != 1 {
        bail!("expected exactly one canonical Workspace definition, found {canonical_definitions}");
    }
    println!(
        "architecture boundaries passed: GPU-free core/reducers, egui-free runtime, narrow panes, one workspace tree, no viewport device creation"
    );
    Ok(())
}

fn assert_tree_excludes(package: &str, forbidden: &[&str]) -> Result<()> {
    let output = Command::new("cargo")
        .args(["tree", "-p", package, "--prefix", "none"])
        .output()
        .with_context(|| format!("run cargo tree for {package}"))?;
    if !output.status.success() {
        bail!("cargo tree failed for {package}");
    }
    let tree = String::from_utf8(output.stdout)?;
    for dependency in forbidden {
        if tree
            .lines()
            .any(|line| line.split_whitespace().next() == Some(dependency))
        {
            bail!("{package} must not depend on {dependency}");
        }
    }
    Ok(())
}

fn ensure_wasm_bindgen_version(expected: &str) -> Result<()> {
    let output = Command::new("wasm-bindgen")
        .arg("--version")
        .output()
        .context("wasm-bindgen CLI is required")?;
    let version = String::from_utf8(output.stdout)?;
    if !output.status.success() || !version.contains(expected) {
        bail!(
            "wasm-bindgen CLI {expected} is required; run `cargo install -f wasm-bindgen-cli --version {expected}` (observed {version:?})"
        );
    }
    Ok(())
}

fn run(program: &str, arguments: &[&str]) -> Result<()> {
    if !Path::new("Cargo.toml").exists() {
        bail!("run xtask from the repository root");
    }
    println!("+ {program} {}", arguments.join(" "));
    let status = Command::new(program)
        .args(arguments)
        .status()
        .with_context(|| format!("run {program}"))?;
    if !status.success() {
        bail!(
            "command failed with {status}: {program} {}",
            arguments.join(" ")
        );
    }
    Ok(())
}
