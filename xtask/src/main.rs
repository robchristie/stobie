use std::{env, fs, path::Path, process::Command};

use anyhow::{Context, Result, anyhow, bail};

mod tokens;
mod ui;

fn main() -> Result<()> {
    let command = env::args().nth(1).unwrap_or_else(|| "help".into());
    match command.as_str() {
        "verify" => verify(),
        "build-web" => build_web(),
        "architecture" => architecture(),
        "tokens" => match env::args().nth(2).as_deref() {
            Some("generate") => tokens::generate(Path::new(".")),
            Some("check") => tokens::check(Path::new(".")),
            Some(action) => bail!("unknown tokens action {action:?}; expected generate or check"),
            None => bail!("tokens requires an action: generate or check"),
        },
        "ui" => ui::run(Path::new("."), env::args().skip(2).collect()),
        _ => {
            println!(
                "cargo xtask verify      run the complete native/browser verification surface"
            );
            println!("cargo xtask build-web   build release WASM application and Worker packages");
            println!("cargo xtask architecture check dependency boundaries");
            println!("cargo xtask tokens generate generate typed Rust from the token source");
            println!("cargo xtask tokens check    validate tokens and check generated drift");
            println!("cargo xtask ui list|render|inspect|audit-text|verify --output-dir <path>");
            Ok(())
        }
    }
}

fn verify() -> Result<()> {
    let evidence_directory = env::current_dir()
        .context("resolve verification working directory")?
        .join(".tools/runtime/verification-evidence");
    fs::create_dir_all(&evidence_directory)
        .context("create ignored verification evidence directory")?;
    let evidence_environment = [("POLYORAMA_EVIDENCE_DIR", evidence_directory.as_path())];
    tokens::check(Path::new("."))?;
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
            "polyorama-gallery",
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
    run_with_environment("npm", &["run", "browser-smoke"], &evidence_environment)?;
    run_with_environment(
        "npm",
        &["run", "gallery-browser-smoke"],
        &evidence_environment,
    )?;
    ui::verify(Path::new("."), &evidence_directory.join("ui-snapshots"))?;
    if cfg!(target_os = "linux") {
        run_with_environment("bash", &["tools/native-smoke.sh"], &evidence_environment)?;
        run_with_environment(
            "bash",
            &["tools/gallery-native-smoke.sh"],
            &evidence_environment,
        )?;
    }
    println!(
        "Polyorama verification passed: format, lint, tests, architecture, release native, release WASM, deterministic UI snapshots, browser and native runtime smoke"
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
            "polyorama-gallery",
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
            "apps/polyorama-gallery/web/pkg",
            "target/wasm32-unknown-unknown/release/polyorama_gallery.wasm",
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
        "polyorama-core",
        &["egui", "eframe", "wgpu", "web-sys", "winit"],
    )?;
    assert_tree_excludes("polyorama-runtime", &["egui", "eframe", "wgpu"])?;
    let mut pane_sources = Vec::new();
    collect_rust_sources(
        Path::new("apps/analytical-workspace-lab/src/panes"),
        &mut pane_sources,
    )?;
    let ui_source = pane_sources
        .iter()
        .map(fs::read_to_string)
        .collect::<std::io::Result<Vec<_>>>()?
        .join("\n");
    for forbidden in [
        "&mut AppModel",
        "&mut Runtime",
        "&mut Workspace",
        "&mut Session",
        "pub session:",
        "&'a mut BTreeMap<PaneId, ActiveTool>",
        "&'a mut BTreeMap<PaneId, DisplaySettings>",
        "&'a mut Option<ResultId>",
        "&'a mut Option<AnnotationId>",
        "&wgpu::Device",
        "&wgpu::Queue",
        "egui_wgpu::",
        "submit_scalar_callback",
    ] {
        if ui_source.contains(forbidden) {
            bail!("pane API contains forbidden broad access: {forbidden}");
        }
    }
    let mut production_ui_sources = pane_sources.clone();
    collect_rust_sources(
        Path::new("crates/polyorama-ui-egui/src"),
        &mut production_ui_sources,
    )?;
    production_ui_sources.push(Path::new("apps/analytical-workspace-lab/src/app.rs").into());
    production_ui_sources.sort();
    production_ui_sources.dedup();
    for path in &production_ui_sources {
        let source = fs::read_to_string(path)?;
        for forbidden in [
            "title.len()",
            ".chars().count()",
            ".len() as f32",
            ".len() as f64",
        ] {
            if source.contains(forbidden) {
                bail!(
                    "production UI source {} contains forbidden character-count text sizing pattern {forbidden:?}; use egui galley measurement",
                    path.display()
                );
            }
        }
    }
    let mut application_ui_sources = pane_sources.clone();
    application_ui_sources.push(Path::new("apps/analytical-workspace-lab/src/app.rs").into());
    for path in &application_ui_sources {
        let source = fs::read_to_string(path)?;
        for forbidden in [
            "egui::Color32",
            "egui::RichText",
            "egui::TextStyle",
            "egui::FontId",
            "egui::CornerRadius",
            "egui::Margin",
            "ui.button(",
            "ui.selectable_label(",
            "ui.strong(",
            "ui.monospace(",
        ] {
            if source.contains(forbidden) {
                bail!(
                    "production UI source {} contains unmanaged style or control primitive {forbidden:?}; use typed tokens and a polyorama-ui-egui recipe",
                    path.display()
                );
            }
        }
        for (line_index, line) in source.lines().enumerate() {
            if line.contains("ui.add_space(") && !line.contains("tokens.") {
                bail!(
                    "production UI source {}:{} contains unmanaged spacing; use typed spacing tokens",
                    path.display(),
                    line_index + 1
                );
            }
        }
    }
    let pane_root = fs::read_to_string("apps/analytical-workspace-lab/src/panes/mod.rs")?;
    for feature in ["image", "camera_gestures", "annotations", "diagnostics"] {
        let path = format!("apps/analytical-workspace-lab/src/panes/{feature}.rs");
        if !Path::new(&path).is_file() {
            bail!("pane feature boundary is missing: {path}");
        }
    }
    for misplaced in [
        "fn image_pane",
        "fn handle_camera",
        "fn handle_annotations",
        "fn diagnostics_pane",
    ] {
        if pane_root.contains(misplaced) {
            bail!("pane dispatcher contains feature implementation: {misplaced}");
        }
    }
    let surface_start = pane_root
        .find("pub struct PaneSurface")
        .ok_or_else(|| anyhow!("PaneSurface is missing"))?;
    let surface_end = pane_root[surface_start..]
        .find("\n}\n")
        .map(|offset| surface_start + offset)
        .ok_or_else(|| anyhow!("PaneSurface boundary is malformed"))?;
    if pane_root[surface_start..surface_end].contains("\n    pub ") {
        bail!("PaneSurface exposes presentation fields instead of read models and feature state");
    }
    for required in ["PaneReadModel", "PaneFeatureState", "PaneIntent"] {
        if !pane_root.contains(required) {
            bail!("narrow pane API is missing {required}");
        }
    }
    let app_source = fs::read_to_string("apps/analytical-workspace-lab/src/app.rs")?;
    if !app_source.contains("if let Err(error) = submit_render_plan(&outputs.render_plan") {
        bail!("application shell must submit the complete typed frame render plan");
    }
    let egui_integration = fs::read_to_string("crates/polyorama-ui-egui/src/lib.rs")?;
    if egui_integration.contains("request_repaint") {
        bail!("egui integration must report interaction activity through FrameOutput");
    }
    for required in [
        "RenderPlanSubmissionError",
        "validate_plan_target_panes",
        "stage_renderer_maintenance",
    ] {
        if !egui_integration.contains(required) {
            bail!("egui integration is missing lifecycle enforcement: {required}");
        }
    }
    let renderer_source = fs::read_to_string("crates/polyorama-render-wgpu/src/lib.rs")?;
    if renderer_source.contains("create_device") {
        bail!("renderer or viewport creates an additional wgpu device");
    }
    let canonical_definitions = [
        "crates/polyorama-core/src/dock.rs",
        "crates/polyorama-ui-egui/src/lib.rs",
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
        "architecture boundaries passed: GPU-free core/reducers, egui-free runtime, narrow panes, measured UI text, one workspace tree, no viewport device creation"
    );
    Ok(())
}

fn collect_rust_sources(directory: &Path, output: &mut Vec<std::path::PathBuf>) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_rust_sources(&path, output)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            output.push(path);
        }
    }
    output.sort();
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
    run_with_environment(program, arguments, &[])
}

fn run_with_environment(
    program: &str,
    arguments: &[&str],
    environment: &[(&str, &Path)],
) -> Result<()> {
    if !Path::new("Cargo.toml").exists() {
        bail!("run xtask from the repository root");
    }
    println!("+ {program} {}", arguments.join(" "));
    let mut command = Command::new(program);
    command.args(arguments);
    for (name, value) in environment {
        command.env(name, value);
    }
    if program == "cargo" {
        let temporary_directory = env::current_dir()
            .context("resolve verification working directory")?
            .join(".tools/runtime/verification-tmp");
        fs::create_dir_all(&temporary_directory)
            .context("create verification temporary directory")?;
        // Rust response files avoid a potentially crowded shared /tmp while
        // remaining inside the repository's ignored runtime area.
        command.env("TMPDIR", temporary_directory);
    }
    let status = command.status().with_context(|| format!("run {program}"))?;
    if !status.success() {
        bail!(
            "command failed with {status}: {program} {}",
            arguments.join(" ")
        );
    }
    Ok(())
}
