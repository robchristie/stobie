//! The local verifier and hosted matrix share the same closed qualification surface.
use super::*;
use std::time::Instant;

const STAGES: &str = include_str!("../../tools/verification-stages.json");
const NAMES: [&str; 10] = [
    "checks",
    "native-lab",
    "native-other",
    "browser",
    "gallery-browser",
    "production",
    "ui",
    "record-desk",
    "webgl2",
    "outside-app",
];

fn configured_names(configuration: &serde_json::Value) -> Result<Vec<&str>> {
    if configuration["schema_version"] != 1 {
        bail!("unsupported verification stage inventory");
    }
    let stages = configuration["stages"]
        .as_array()
        .context("verification stage inventory")?;
    let names: Vec<_> = stages
        .iter()
        .map(|stage| stage["name"].as_str().context("verification stage name"))
        .collect::<Result<_>>()?;
    let selected: std::collections::BTreeSet<_> = names.iter().copied().collect();
    if names.len() != NAMES.len() || selected != std::collections::BTreeSet::from(NAMES) {
        bail!("verification stage inventory must cover the complete surface exactly once");
    }
    Ok(names)
}

pub fn all() -> Result<()> {
    let configuration: serde_json::Value = serde_json::from_str(STAGES)?;
    for name in configured_names(&configuration)? {
        run_stage(name)?;
    }
    println!(
        "Stobie verification passed: plans, format, lint, tests, architecture, release native, release WASM, deterministic UI snapshots, browser and native runtime smoke, and an application built outside the repository"
    );
    Ok(())
}

pub fn run_stage(name: &str) -> Result<()> {
    configured_names(&serde_json::from_str(STAGES)?)?;
    let started = Instant::now();
    println!("Verification stage: {name}");
    let evidence = env::current_dir()?.join(".tools/runtime/verification-evidence");
    fs::create_dir_all(evidence.join("typed-icons"))?;
    let evidence_environment = [("STOBIE_EVIDENCE_DIR", evidence.as_path())];
    let icons = evidence.join("typed-icons");
    let icon_environment = [("STOBIE_EVIDENCE_DIR", icons.as_path())];
    let interface = evidence.join(format!("interface-{name}"));
    let interface_environment = [("STOBIE_EVIDENCE_DIR", interface.as_path())];
    match name {
        "checks" => {
            // Clear owned configurations before setup or compilation can fail.
            run("python3", &["tools/test-rust.py", "prepare"])?;
            run(
                "python3",
                &["-m", "unittest", "discover", "-s", "tools/tests"],
            )?;
            run("python3", &["tools/check-tracked-files.py"])?;
            plans::check(Path::new("."))?;
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
                    "stobie-gallery",
                    "-p",
                    "stobie-tile-worker",
                    "-p",
                    "live-feed",
                    "--",
                    "-D",
                    "warnings",
                ],
            )?;
            // The checks stage installs Binaryen, so the real wasm-opt
            // packaging test must run rather than skip.
            run_with_environment(
                "python3",
                &["tools/test-rust.py", "workspace"],
                &[("STOBIE_REQUIRE_WASM_OPT", Path::new("1"))],
            )?;
            run("python3", &["tools/check-api-docs.py"])?;
            architecture()?;
            run("python3", &["tools/test-rust.py", "ui-no-default"])?;
            run("npm", &["ci"])?;
            run(
                "node",
                &[
                    "--test",
                    "tools/tests/browser-startup.test.mjs",
                    "tools/tests/browser-launch.test.mjs",
                    "tools/tests/browser-launch-failure.test.mjs",
                    "tools/tests/browser-idle.test.mjs",
                    "tools/tests/navigation-browser-focus.test.mjs",
                    "tools/tests/record-desk-target.test.mjs",
                    "tools/tests/application-client.test.mjs",
                    "tools/tests/application-interface-selection.test.mjs",
                    "tools/tests/lab-result-selection.test.mjs",
                    "tools/tests/browser-serve.test.mjs",
                    "tools/tests/service-worker-push.test.mjs",
                ],
            )?;
        }
        "native-lab" => {
            // Selecting the application and its example together avoids a second
            // feature-resolution/build pass for their shared framework crates.
            run(
                "cargo",
                &[
                    "build",
                    "--release",
                    "-p",
                    "analytical-workspace-lab",
                    "--lib",
                    "--bins",
                    "--examples",
                ],
            )?;
            if cfg!(target_os = "linux") {
                run("bash", &["tools/bootstrap-linux-ui.sh"])?;
                run("npm", &["ci"])?;
                run_with_environment("bash", &["tools/native-smoke.sh"], &evidence_environment)?;
                run_with_environment(
                    "bash",
                    &["tools/minimal-native-smoke.sh"],
                    &evidence_environment,
                )?;
                run_with_environment(
                    "bash",
                    &[
                        "tools/application-interface-smoke.sh",
                        "--apps",
                        "lab",
                        "--hosts",
                        "native",
                    ],
                    &interface_environment,
                )?;
                run("cargo", &["build", "--release", "-p", "stobie-cli"])?;
                run_with_environment(
                    "bash",
                    &["tools/stobie-cli-smoke.sh", "lab"],
                    &evidence_environment,
                )?;
            }
        }
        "native-other" => {
            // Together with native-lab, this covers every workspace library and
            // binary previously selected by `cargo build --workspace --release`.
            run(
                "cargo",
                &[
                    "build",
                    "--workspace",
                    "--exclude",
                    "analytical-workspace-lab",
                    "--release",
                ],
            )?;
            if cfg!(target_os = "linux") {
                run("bash", &["tools/bootstrap-linux-ui.sh"])?;
                run_with_environment(
                    "bash",
                    &["tools/gallery-native-smoke.sh"],
                    &evidence_environment,
                )?;
                run_with_environment(
                    "bash",
                    &["tools/icon-actions-native-smoke.sh"],
                    &icon_environment,
                )?;
                run_with_environment(
                    "bash",
                    &["tools/navigation-native-smoke.sh"],
                    &evidence_environment,
                )?;
                run_with_environment(
                    "bash",
                    &["tools/status-chip-native-smoke.sh"],
                    &evidence_environment,
                )?;
                run_with_environment(
                    "bash",
                    &["tools/reading-text-native-smoke.sh"],
                    &evidence_environment,
                )?;
                run_with_environment(
                    "bash",
                    &["tools/live-feed-smoke.sh", "native"],
                    &evidence_environment,
                )?;
            }
        }
        "browser" => {
            build_web()?;
            prepare_browser()?;
            run_with_environment("npm", &["run", "browser-smoke"], &evidence_environment)?;
            run_with_environment(
                "bash",
                &["tools/aria-layer-browser.sh", "--app", "lab"],
                &evidence_environment,
            )?;
            if cfg!(target_os = "linux") {
                run_with_environment(
                    "bash",
                    &[
                        "tools/application-interface-smoke.sh",
                        "--apps",
                        "lab",
                        "--hosts",
                        "browser",
                    ],
                    &interface_environment,
                )?;
                run_with_environment(
                    "bash",
                    &["tools/live-feed-smoke.sh", "browser"],
                    &evidence_environment,
                )?;
            }
        }
        "gallery-browser" => {
            build_gallery_web()?;
            prepare_browser()?;
            run_with_environment(
                "npm",
                &["run", "gallery-browser-smoke"],
                &evidence_environment,
            )?;
            // Local time through the Gallery's WebAssembly build and the page's Intl.
            run_with_environment(
                "node",
                &["tools/local-time-browser.mjs"],
                &evidence_environment,
            )?;
            run_with_environment(
                "bash",
                &["tools/icon-actions-browser-smoke.sh"],
                &icon_environment,
            )?;
            run_with_environment(
                "bash",
                &["tools/navigation-browser-smoke.sh"],
                &evidence_environment,
            )?;
            run_with_environment(
                "bash",
                &["tools/status-chip-browser-smoke.sh"],
                &evidence_environment,
            )?;
            run_with_environment(
                "bash",
                &["tools/reading-text-browser-smoke.sh"],
                &evidence_environment,
            )?;
            run_with_environment(
                "bash",
                &["tools/markdown-browser-smoke.sh"],
                &evidence_environment,
            )?;
            run_with_environment(
                "bash",
                &["tools/chat-browser-smoke.sh"],
                &evidence_environment,
            )?;
            run_with_environment(
                "bash",
                &["tools/scroll-browser-smoke.sh"],
                &evidence_environment,
            )?;
            run_with_environment(
                "bash",
                &["tools/copy-browser-smoke.sh"],
                &evidence_environment,
            )?;
            run_with_environment(
                "bash",
                &["tools/dialog-browser-smoke.sh"],
                &evidence_environment,
            )?;
            run_with_environment(
                "bash",
                &["tools/responsive-navigation-browser-smoke.sh"],
                &evidence_environment,
            )?;
            run_with_environment(
                "bash",
                &["tools/aria-layer-browser.sh", "--app", "gallery"],
                &evidence_environment,
            )?;
        }
        "production" => {
            prepare_browser()?;
            browser::build(Vec::new())?;
            run(
                "node",
                &[
                    "tools/browser-startup-smoke.mjs",
                    "target/browser-production",
                ],
            )?;
            run(
                "node",
                &[
                    "tools/browser-startup-benchmark.mjs",
                    "--directory",
                    "target/browser-production",
                    "--output",
                    ".tools/runtime/verification-evidence/startup",
                    "--pairs",
                    "1",
                    "--profiles",
                    "local",
                    "--apps",
                    "lab,gallery",
                ],
            )?;
            size::report(
                &[(
                    "browser-production".into(),
                    "target/browser-production".into(),
                )],
                None,
                &evidence.join("web-size/browser-production.json"),
            )?;
        }
        "ui" => {
            build_gallery_web()?;
            prepare_browser()?;
            ui::verify(Path::new("."), &evidence.join("ui-snapshots"))?;
        }
        "record-desk" => {
            run("python3", &["tools/check-record-desk.py"])?;
            prepare_browser()?;
            run_with_environment(
                "bash",
                &["tools/record-desk-browser-smoke.sh"],
                &evidence_environment,
            )?;
            run_with_environment(
                "bash",
                &["tools/aria-layer-browser.sh", "--app", "record-desk"],
                &evidence_environment,
            )?;
            if cfg!(target_os = "linux") {
                run_with_environment(
                    "bash",
                    &["tools/record-desk-native-smoke.sh"],
                    &evidence_environment,
                )?;
                run_with_environment(
                    "bash",
                    &[
                        "tools/application-interface-smoke.sh",
                        "--apps",
                        "record-desk",
                    ],
                    &interface_environment,
                )?;
                // The Rust CLI and the Node client agree on the same scenarios.
                run("cargo", &["build", "--release", "-p", "stobie-cli"])?;
                run(
                    "cargo",
                    &[
                        "build",
                        "--locked",
                        "--release",
                        "--manifest-path",
                        "consumers/record-desk/Cargo.toml",
                    ],
                )?;
                run_with_environment(
                    "bash",
                    &["tools/stobie-cli-smoke.sh", "record-desk"],
                    &evidence_environment,
                )?;
                run_with_environment(
                    "bash",
                    &["tools/stobie-cli-smoke.sh", "record-desk-browser"],
                    &evidence_environment,
                )?;
            }
        }
        "outside-app" => {
            // `stobie new` creates an application outside the checkout that
            // builds against this source; it is built natively and with
            // `stobie build-web`, then started natively and on WebGPU and
            // WebGL2 and operated only through `stobie`.
            run("cargo", &["build", "--release", "-p", "stobie-cli"])?;
            prepare_browser()?;
            if cfg!(target_os = "linux") {
                run_with_environment(
                    "bash",
                    &["tools/outside-app-smoke.sh"],
                    &evidence_environment,
                )?;
                // The template application's production package, as built.
                size::report(
                    &[(
                        "template".into(),
                        evidence.join("outside-app/production-manifest.json"),
                    )],
                    None,
                    &evidence.join("web-size/template.json"),
                )?;
            }
        }
        "webgl2" => {
            // The Lab, the Gallery, Record Desk and Live Feed repeat their
            // browser checks on WebGL2: WebGPU flags are withheld and pages open with
            // `?renderer=webgl2`. Each smoke checks the canvas reports WebGL2.
            let webgl2 = evidence.join("webgl2");
            let environment = [
                ("STOBIE_EVIDENCE_DIR", webgl2.as_path()),
                ("STOBIE_BROWSER_RENDERER", Path::new("webgl2")),
            ];
            let icons = webgl2.join("typed-icons");
            let icon_environment = [
                ("STOBIE_EVIDENCE_DIR", icons.as_path()),
                ("STOBIE_BROWSER_RENDERER", Path::new("webgl2")),
            ];
            let interface_environment = [
                ("STOBIE_EVIDENCE_DIR", interface.as_path()),
                ("STOBIE_BROWSER_RENDERER", Path::new("webgl2")),
            ];
            fs::create_dir_all(&icons)?;
            build_web()?;
            run("bash", &["consumers/record-desk/build-web.sh"])?;
            prepare_browser()?;
            run_with_environment("npm", &["run", "browser-smoke"], &environment)?;
            run_with_environment("npm", &["run", "gallery-browser-smoke"], &environment)?;
            run_with_environment(
                "bash",
                &["tools/icon-actions-browser-smoke.sh"],
                &icon_environment,
            )?;
            for smoke in [
                "tools/navigation-browser-smoke.sh",
                "tools/status-chip-browser-smoke.sh",
                "tools/reading-text-browser-smoke.sh",
                "tools/markdown-browser-smoke.sh",
                "tools/chat-browser-smoke.sh",
                "tools/scroll-browser-smoke.sh",
                "tools/copy-browser-smoke.sh",
                "tools/dialog-browser-smoke.sh",
                "tools/responsive-navigation-browser-smoke.sh",
                "tools/record-desk-browser-smoke.sh",
            ] {
                run_with_environment("bash", &[smoke], &environment)?;
            }
            for app in ["lab", "gallery", "record-desk"] {
                run_with_environment(
                    "bash",
                    &["tools/aria-layer-browser.sh", "--app", app],
                    &environment,
                )?;
            }
            if cfg!(target_os = "linux") {
                run_with_environment(
                    "bash",
                    &["tools/live-feed-smoke.sh", "browser"],
                    &environment,
                )?;
                run_with_environment(
                    "bash",
                    &[
                        "tools/application-interface-smoke.sh",
                        "--apps",
                        "lab",
                        "--hosts",
                        "browser",
                    ],
                    &interface_environment,
                )?;
                run("cargo", &["build", "--release", "-p", "stobie-cli"])?;
                run_with_environment(
                    "bash",
                    &["tools/stobie-cli-smoke.sh", "record-desk-browser"],
                    &environment,
                )?;
            }
        }
        _ => bail!("unknown verification stage {name:?}"),
    }
    println!(
        "Verification stage {name} passed in {:.2}s",
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

fn build_gallery_web() -> Result<()> {
    stobie_build::write_startup_loader(Path::new("apps/stobie-gallery/web"))?;
    ensure_wasm_bindgen()?;
    run(
        "cargo",
        &[
            "build",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "-p",
            "stobie-gallery",
        ],
    )?;
    bindgen(
        target_directory()?.join("wasm32-unknown-unknown/release/stobie_gallery.wasm"),
        "apps/stobie-gallery/web/pkg",
    )
}

fn prepare_browser() -> Result<()> {
    if cfg!(target_os = "linux") {
        run("bash", &["tools/bootstrap-linux-ui.sh"])?;
    }
    run("npm", &["ci"])?;
    run("npx", &["playwright", "install", "chromium"])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn hosted_inventory_covers_the_closed_canonical_surface_once() {
        let configuration: serde_json::Value = serde_json::from_str(STAGES).unwrap();
        assert_eq!(configuration["schema_version"], 1);
        let stages = configuration["stages"].as_array().unwrap();
        let names: BTreeSet<_> = stages
            .iter()
            .map(|stage| stage["name"].as_str().unwrap())
            .collect();
        assert_eq!(names.len(), stages.len());
        assert_eq!(
            names,
            BTreeSet::from([
                "checks",
                "native-lab",
                "native-other",
                "browser",
                "gallery-browser",
                "production",
                "ui",
                "record-desk",
                "webgl2",
                "outside-app"
            ])
        );
    }

    #[test]
    fn missing_duplicate_and_unknown_stages_cannot_qualify_a_partial_surface() {
        let original: serde_json::Value = serde_json::from_str(STAGES).unwrap();
        for variant in 0..3 {
            let mut configuration = original.clone();
            let stages = configuration["stages"].as_array_mut().unwrap();
            match variant {
                0 => {
                    stages.pop();
                }
                1 => {
                    stages[1] = stages[0].clone();
                }
                _ => {
                    stages[0]["name"] = "unknown".into();
                }
            }
            assert!(configured_names(&configuration).is_err());
        }
    }
}
