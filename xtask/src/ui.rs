use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, anyhow, bail};
use polyorama_gallery::{GalleryConfiguration, StoryId};
use polyorama_ui_egui::TextAuditCoverage;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const FIXTURE_MANIFEST: &str = "docs/ui-snapshots/fixtures.json";
const EXPECTED_ROOT: &str = "docs/ui-snapshots/expected";
const SCHEMA_VERSION: u32 = 1;
const OUTPUT_MARKER: &str = ".polyorama-ui-output-v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureManifest {
    schema_version: u32,
    fixtures: Vec<UiFixture>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UiFixture {
    id: String,
    story: StoryId,
    viewport: Viewport,
    configuration: GalleryConfiguration,
    data_fixture: String,
    fonts: String,
    renderer: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Viewport {
    width: u32,
    height: u32,
}

#[derive(Debug)]
struct UiArguments {
    action: String,
    output_directory: PathBuf,
    fixture: Option<String>,
    all: bool,
}

#[derive(Debug, Deserialize)]
struct VisualDiff {
    dimensions_equal: bool,
    differing_pixels: u64,
    total_pixels: u64,
}

pub fn run(root: &Path, arguments: Vec<String>) -> Result<()> {
    let arguments = parse_arguments(arguments)?;
    let manifest = load_manifest(root)?;
    if arguments.action != "verify" {
        recreate_owned_directory(root, &arguments.output_directory)?;
    }

    match arguments.action.as_str() {
        "list" => list(&manifest, &arguments.output_directory),
        "render" | "inspect" => {
            let fixture = selected_fixture(&manifest, arguments.fixture.as_deref())?;
            match capture(root, fixture, &arguments.output_directory, None) {
                Ok(()) => write_summary(
                    &arguments.output_directory,
                    &arguments.action,
                    "passed",
                    json!({ "fixture": fixture.id }),
                ),
                Err(error) => {
                    write_summary(
                        &arguments.output_directory,
                        &arguments.action,
                        "failed",
                        json!({ "fixture": fixture.id, "error": format!("{error:#}") }),
                    )?;
                    Err(error)
                }
            }
        }
        "audit-text" => audit_text(root, &manifest, &arguments),
        "verify" => verify(root, &arguments.output_directory),
        action => bail!(
            "unknown ui action {action:?}; expected list, render, inspect, audit-text or verify"
        ),
    }
}

pub fn verify(root: &Path, output_directory: &Path) -> Result<()> {
    let manifest = load_manifest(root)?;
    validate_evaluation_seed(root)?;
    recreate_owned_directory(root, output_directory)?;
    let captures = output_directory.join("captures");
    let failures = output_directory.join("failures");
    recreate_owned_directory(root, &captures)?;
    recreate_owned_directory(root, &failures)?;

    let mut failure_ids = Vec::new();
    for fixture in &manifest.fixtures {
        let actual = captures.join(&fixture.id);
        let expected = root.join(EXPECTED_ROOT).join(&fixture.id);
        let expected_visual = expected.join("visual.png");
        let expected_visual = expected_visual.is_file().then_some(expected_visual);

        if let Err(error) = capture(root, fixture, &actual, expected_visual.as_deref()) {
            failure_ids.push(fixture.id.clone());
            write_unavailable_failure_bundle(
                root,
                &failures.join(&fixture.id),
                fixture,
                &expected,
                &actual,
                "capture_failed",
                &format!("{error:#}"),
            )?;
            continue;
        }

        if !has_complete_evidence(&expected) {
            failure_ids.push(fixture.id.clone());
            write_unavailable_failure_bundle(
                root,
                &failures.join(&fixture.id),
                fixture,
                &expected,
                &actual,
                "missing_checked_in_baseline",
                "Checked-in expected evidence is incomplete. Verification never creates or updates baselines.",
            )?;
            continue;
        }

        let comparison = (|| -> Result<(bool, VisualDiff)> {
            let mut failed = false;
            for name in ["metadata.json", "semantic.json", "text.json"] {
                if read_json(&expected.join(name))? != read_json(&actual.join(name))? {
                    failed = true;
                }
            }
            let semantic = read_json(&actual.join("semantic.json"))?;
            let text = read_json(&actual.join("text.json"))?;
            read_text_coverage(&text)?;
            if has_findings(&semantic, "semantic_audit")? || has_findings(&text, "audit")? {
                failed = true;
            }
            let visual_diff: VisualDiff =
                serde_json::from_value(read_json(&actual.join("visual-diff.json"))?)?;
            if !visual_diff.dimensions_equal || visual_diff.differing_pixels != 0 {
                failed = true;
            }
            Ok((failed, visual_diff))
        })();

        match comparison {
            Ok((false, _)) => {}
            Ok((true, visual_diff)) => {
                failure_ids.push(fixture.id.clone());
                populate_failure_bundle(
                    root,
                    &failures.join(&fixture.id),
                    &expected,
                    &actual,
                    &visual_diff,
                )?;
            }
            Err(error) => {
                failure_ids.push(fixture.id.clone());
                write_unavailable_failure_bundle(
                    root,
                    &failures.join(&fixture.id),
                    fixture,
                    &expected,
                    &actual,
                    "comparison_failed",
                    &format!("{error:#}"),
                )?;
            }
        }
    }

    let (unexpected_baselines, inventory_error) =
        match unexpected_expected_fixtures(root, &manifest) {
            Ok(unexpected) => {
                if !unexpected.is_empty() {
                    failure_ids.extend(
                        unexpected
                            .iter()
                            .map(|id| format!("unexpected-baseline:{id}")),
                    );
                    write_inventory_failure(root, &failures, &unexpected)?;
                }
                (unexpected, None)
            }
            Err(error) => {
                let error = format!("{error:#}");
                failure_ids.push("baseline-inventory".into());
                write_global_failure_bundle(
                    root,
                    &failures.join("baseline-inventory"),
                    "baseline_inventory_failed",
                    &error,
                )?;
                (BTreeSet::new(), Some(error))
            }
        };

    let status = if failure_ids.is_empty() {
        "passed"
    } else {
        "failed"
    };
    write_summary(
        output_directory,
        "verify",
        status,
        json!({
            "fixture_count": manifest.fixtures.len(),
            "failed_fixtures": failure_ids,
            "unexpected_baselines": unexpected_baselines,
            "baseline_inventory_error": inventory_error,
            "baseline_updates_allowed": false
        }),
    )?;
    if status == "failed" {
        bail!("UI verification failed; inspect {}", failures.display());
    }
    Ok(())
}

fn parse_arguments(arguments: Vec<String>) -> Result<UiArguments> {
    let mut arguments = arguments.into_iter();
    let action = arguments.next().ok_or_else(|| {
        anyhow!("ui requires an action: list, render, inspect, audit-text or verify")
    })?;
    let mut output_directory = None;
    let mut fixture = None;
    let mut all = false;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--output-dir" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| anyhow!("--output-dir requires a path"))?;
                output_directory = Some(PathBuf::from(value));
            }
            "--fixture" => {
                fixture = Some(
                    arguments
                        .next()
                        .ok_or_else(|| anyhow!("--fixture requires an id"))?,
                );
            }
            "--all" => all = true,
            unknown => bail!("unknown ui argument {unknown:?}"),
        }
    }
    let output_directory = output_directory
        .ok_or_else(|| anyhow!("ui commands require an explicit --output-dir <path>"))?;
    if fixture.is_some() && all {
        bail!("use either --fixture <id> or --all, not both");
    }
    match action.as_str() {
        "render" | "inspect" if fixture.is_none() => {
            bail!("ui {action} requires --fixture <id>")
        }
        "audit-text" if fixture.is_none() && !all => {
            bail!("ui audit-text requires --fixture <id> or --all")
        }
        "list" | "verify" if fixture.is_some() || all => {
            bail!("ui {action} does not accept --fixture or --all")
        }
        _ => {}
    }
    Ok(UiArguments {
        action,
        output_directory,
        fixture,
        all,
    })
}

fn load_manifest(root: &Path) -> Result<FixtureManifest> {
    let path = root.join(FIXTURE_MANIFEST);
    let manifest: FixtureManifest = serde_json::from_slice(
        &fs::read(&path).with_context(|| format!("read UI fixture manifest {}", path.display()))?,
    )
    .with_context(|| format!("parse UI fixture manifest {}", path.display()))?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_manifest(manifest: &FixtureManifest) -> Result<()> {
    if manifest.schema_version != SCHEMA_VERSION {
        bail!(
            "unsupported UI fixture schema {}; expected {SCHEMA_VERSION}",
            manifest.schema_version
        );
    }
    if manifest.fixtures.is_empty() {
        bail!("UI fixture manifest must not be empty");
    }
    let mut ids = BTreeSet::new();
    for fixture in &manifest.fixtures {
        if fixture.id.is_empty()
            || !fixture
                .id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            bail!("invalid stable UI fixture id {:?}", fixture.id);
        }
        if !ids.insert(&fixture.id) {
            bail!("duplicate UI fixture id {:?}", fixture.id);
        }
        if !(320..=3840).contains(&fixture.viewport.width)
            || !(240..=2160).contains(&fixture.viewport.height)
        {
            bail!("invalid viewport for UI fixture {:?}", fixture.id);
        }
        if !fixture.configuration.font_scale.is_finite()
            || fixture.configuration.validated() != fixture.configuration
        {
            bail!(
                "invalid gallery configuration for UI fixture {:?}",
                fixture.id
            );
        }
        for (label, value) in [
            ("data_fixture", fixture.data_fixture.as_str()),
            ("fonts", fixture.fonts.as_str()),
            ("renderer", fixture.renderer.as_str()),
        ] {
            if value.trim().is_empty() {
                bail!("UI fixture {:?} has empty {label}", fixture.id);
            }
        }
    }
    Ok(())
}

fn validate_evaluation_seed(root: &Path) -> Result<()> {
    let path = root.join("docs/ui-evaluation-seed.json");
    let seed = read_json(&path)?;
    if seed.get("schema_version").and_then(Value::as_u64) != Some(1)
        || seed.get("status").and_then(Value::as_str) != Some("frozen")
    {
        bail!("UI evaluation seed must use schema 1 and remain frozen");
    }
    let dimensions = seed
        .pointer("/scoring/dimensions")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("UI evaluation seed has no scoring dimensions"))?;
    if dimensions.len() != 4
        || seed
            .pointer("/scoring/scale/minimum")
            .and_then(Value::as_i64)
            != Some(0)
        || seed
            .pointer("/scoring/scale/maximum")
            .and_then(Value::as_i64)
            != Some(2)
    {
        bail!("UI evaluation seed must retain its four measurable 0–2 dimensions");
    }
    let tasks = seed
        .get("tasks")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("UI evaluation seed has no tasks"))?;
    if tasks.len() < 5 {
        bail!("UI evaluation seed must retain at least five frozen tasks");
    }
    let mut ids = BTreeSet::new();
    for task in tasks {
        let id = task
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("UI evaluation task has no id"))?;
        if !ids.insert(id) {
            bail!("duplicate UI evaluation task id {id:?}");
        }
        let story = task
            .get("story")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("UI evaluation task {id:?} has no story"))?;
        story
            .parse::<StoryId>()
            .map_err(|error| anyhow!("UI evaluation task {id:?}: {error}"))?;
        for dimension in ["visual_text", "semantics", "interaction", "evidence"] {
            let assertions = task
                .pointer(&format!("/assertions/{dimension}"))
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    anyhow!("UI evaluation task {id:?} has no {dimension} assertions")
                })?;
            if assertions.is_empty() {
                bail!("UI evaluation task {id:?} has empty {dimension} assertions");
            }
        }
    }
    Ok(())
}

fn unexpected_expected_fixtures(
    root: &Path,
    manifest: &FixtureManifest,
) -> Result<BTreeSet<String>> {
    let expected_root = root.join(EXPECTED_ROOT);
    let expected_ids = fs::read_dir(&expected_root)
        .with_context(|| format!("read expected UI snapshots {}", expected_root.display()))?
        .map(|entry| -> Result<Option<String>> {
            let entry = entry?;
            Ok(entry
                .file_type()?
                .is_dir()
                .then(|| entry.file_name().to_string_lossy().into_owned()))
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<BTreeSet<_>>();
    let fixture_ids = manifest
        .fixtures
        .iter()
        .map(|fixture| fixture.id.clone())
        .collect::<BTreeSet<_>>();
    Ok(expected_ids.difference(&fixture_ids).cloned().collect())
}

fn selected_fixture<'a>(manifest: &'a FixtureManifest, id: Option<&str>) -> Result<&'a UiFixture> {
    let id = id.ok_or_else(|| anyhow!("a fixture id is required"))?;
    manifest
        .fixtures
        .iter()
        .find(|fixture| fixture.id == id)
        .ok_or_else(|| {
            anyhow!("unknown UI fixture {id:?}; run `cargo xtask ui list --output-dir <path>`")
        })
}

fn list(manifest: &FixtureManifest, output_directory: &Path) -> Result<()> {
    write_json(&output_directory.join("manifest.json"), manifest)?;
    write_summary(
        output_directory,
        "list",
        "passed",
        json!({ "fixtures": manifest.fixtures }),
    )
}

fn audit_text(root: &Path, manifest: &FixtureManifest, arguments: &UiArguments) -> Result<()> {
    let fixtures: Vec<&UiFixture> = if arguments.all {
        manifest.fixtures.iter().collect()
    } else {
        vec![selected_fixture(manifest, arguments.fixture.as_deref())?]
    };
    let mut failed = Vec::new();
    let mut errors = Vec::new();
    let mut coverage = Vec::new();
    for fixture in &fixtures {
        let output = arguments.output_directory.join(&fixture.id);
        let result = (|| -> Result<bool> {
            capture(root, fixture, &output, None)?;
            let text = read_json(&output.join("text.json"))?;
            coverage.push(json!({ "fixture": fixture.id, "coverage": read_text_coverage(&text)? }));
            let semantic = read_json(&output.join("semantic.json"))?;
            Ok(has_findings(&text, "audit")? || has_findings(&semantic, "semantic_audit")?)
        })();
        match result {
            Ok(false) => {}
            Ok(true) => failed.push(fixture.id.clone()),
            Err(error) => {
                failed.push(fixture.id.clone());
                errors.push(json!({ "fixture": fixture.id, "error": format!("{error:#}") }));
            }
        }
    }
    let status = if failed.is_empty() {
        "passed"
    } else {
        "failed"
    };
    write_summary(
        &arguments.output_directory,
        "audit-text",
        status,
        json!({
            "fixture_count": fixtures.len(),
            "failed_fixtures": failed,
            "capture_errors": errors,
            "pass_meaning": "Every observed Polyorama text component passed.",
            "coverage": coverage
        }),
    )?;
    if status == "failed" {
        bail!("UI text or semantic audit failed")
    }
    Ok(())
}

fn capture(
    root: &Path,
    fixture: &UiFixture,
    output_directory: &Path,
    expected_visual: Option<&Path>,
) -> Result<()> {
    recreate_owned_directory(root, output_directory)?;
    let request = json!({
        "schema_version": SCHEMA_VERSION,
        "fixture": fixture,
        "output_directory": absolute(output_directory)?,
        "expected_visual": expected_visual.map(absolute).transpose()?,
    });
    let request_path = output_directory.join("request.json");
    write_json(&request_path, &request)?;
    let web_package = root.join("apps/polyorama-gallery/web/pkg/polyorama_gallery.js");
    if !web_package.is_file() {
        bail!("gallery web package is missing; run `cargo xtask build-web` before UI capture");
    }
    let temporary_directory = root.join(".tools/tmp");
    fs::create_dir_all(&temporary_directory)?;
    let status = Command::new("node")
        .arg(root.join("tools/ui-capture.mjs"))
        .arg("--request")
        .arg(absolute(&request_path)?)
        .env("TMPDIR", &temporary_directory)
        .status()
        .context("run deterministic UI capture")?;
    if !status.success() {
        bail!(
            "UI capture failed for {:?}; inspect {}",
            fixture.id,
            output_directory.join("logs").display()
        );
    }
    Ok(())
}

fn populate_failure_bundle(
    root: &Path,
    failure: &Path,
    expected: &Path,
    actual: &Path,
    visual_diff: &VisualDiff,
) -> Result<()> {
    recreate_owned_directory(root, failure)?;
    for side in ["expected", "actual"] {
        fs::create_dir_all(failure.join(side))?;
    }
    for name in ["metadata.json", "semantic.json", "text.json", "visual.png"] {
        fs::copy(expected.join(name), failure.join("expected").join(name))?;
        fs::copy(actual.join(name), failure.join("actual").join(name))?;
    }
    fs::create_dir_all(failure.join("diff"))?;
    for name in ["metadata.json", "semantic.json", "text.json"] {
        let expected_value = read_json(&expected.join(name))?;
        let actual_value = read_json(&actual.join(name))?;
        write_json(
            &failure.join("diff").join(name),
            &json!({
                "equal": expected_value == actual_value,
                "expected": expected_value,
                "actual": actual_value
            }),
        )?;
    }
    fs::copy(
        actual.join("visual-diff.png"),
        failure.join("diff/visual.png"),
    )?;
    write_json(
        &failure.join("diff/visual.json"),
        &json!({
            "dimensions_equal": visual_diff.dimensions_equal,
            "differing_pixels": visual_diff.differing_pixels,
            "total_pixels": visual_diff.total_pixels,
            "pixel_tolerance": 0
        }),
    )?;
    copy_directory(&actual.join("logs"), &failure.join("logs"))?;
    write_json(
        &failure.join("summary.json"),
        &json!({
            "schema_version": SCHEMA_VERSION,
            "status": "failed",
            "reason": "snapshot_mismatch"
        }),
    )
}

fn write_unavailable_failure_bundle(
    root: &Path,
    failure: &Path,
    fixture: &UiFixture,
    expected: &Path,
    actual: &Path,
    reason: &str,
    error: &str,
) -> Result<()> {
    recreate_owned_directory(root, failure)?;
    let expected_missing = copy_available_evidence(expected, &failure.join("expected"))?;
    let actual_missing = copy_available_evidence(actual, &failure.join("actual"))?;
    fs::create_dir_all(failure.join("diff"))?;
    fs::create_dir_all(failure.join("logs"))?;
    if actual.join("logs").is_dir() {
        copy_directory(&actual.join("logs"), &failure.join("logs"))?;
    }
    write_json(
        &failure.join("expected/unavailable.json"),
        &json!({ "missing": expected_missing }),
    )?;
    write_json(
        &failure.join("actual/unavailable.json"),
        &json!({ "missing": actual_missing }),
    )?;
    write_json(
        &failure.join("diff/unavailable.json"),
        &json!({ "reason": reason, "error": error }),
    )?;
    write_json(
        &failure.join("summary.json"),
        &json!({
            "schema_version": SCHEMA_VERSION,
            "status": "failed",
            "reason": reason,
            "error": error,
            "fixture": fixture
        }),
    )?;
    fs::write(
        failure.join("logs/runner.log"),
        format!("{reason}: {error}\n"),
    )?;
    Ok(())
}

fn write_inventory_failure(
    root: &Path,
    failures: &Path,
    unexpected: &BTreeSet<String>,
) -> Result<()> {
    let failure = failures.join("unexpected-baselines");
    recreate_owned_directory(root, &failure)?;
    for category in ["expected", "actual", "diff", "logs"] {
        fs::create_dir_all(failure.join(category))?;
    }
    let evidence = json!({
        "reason": "unexpected_checked_in_baselines",
        "directories": unexpected
    });
    for path in [
        "expected/inventory.json",
        "actual/unavailable.json",
        "diff/inventory.json",
    ] {
        write_json(&failure.join(path), &evidence)?;
    }
    fs::write(
        failure.join("logs/runner.log"),
        "Expected snapshot directories must correspond exactly to the closed fixture manifest.\n",
    )?;
    write_json(
        &failure.join("summary.json"),
        &json!({
            "schema_version": SCHEMA_VERSION,
            "status": "failed",
            "reason": "unexpected_checked_in_baselines",
            "directories": unexpected
        }),
    )
}

fn write_global_failure_bundle(
    root: &Path,
    failure: &Path,
    reason: &str,
    error: &str,
) -> Result<()> {
    recreate_owned_directory(root, failure)?;
    for category in ["expected", "actual", "diff", "logs"] {
        fs::create_dir_all(failure.join(category))?;
    }
    let unavailable = json!({ "reason": reason, "error": error });
    write_json(&failure.join("expected/unavailable.json"), &unavailable)?;
    write_json(&failure.join("actual/unavailable.json"), &unavailable)?;
    write_json(&failure.join("diff/unavailable.json"), &unavailable)?;
    fs::write(
        failure.join("logs/runner.log"),
        format!("{reason}: {error}\n"),
    )?;
    write_json(
        &failure.join("summary.json"),
        &json!({
            "schema_version": SCHEMA_VERSION,
            "status": "failed",
            "reason": reason,
            "error": error
        }),
    )
}

fn has_complete_evidence(directory: &Path) -> bool {
    ["metadata.json", "semantic.json", "text.json", "visual.png"]
        .into_iter()
        .all(|name| directory.join(name).is_file())
}

fn read_text_coverage(text: &Value) -> Result<TextAuditCoverage> {
    let coverage: TextAuditCoverage = serde_json::from_value(text["coverage"].clone())
        .context("text evidence is missing valid coverage metadata")?;
    let observations = text["observations"]
        .as_array()
        .ok_or_else(|| anyhow!("text evidence is missing observations"))?;
    let components: BTreeSet<String> = observations
        .iter()
        .map(|observation| observation["component_id"].to_string())
        .collect();
    if coverage.measured_components != components.len()
        || coverage.observed_native_controls > coverage.native_text_controls
    {
        bail!("text coverage counts disagree with their denominator");
    }
    Ok(coverage)
}

fn has_findings(value: &Value, field: &str) -> Result<bool> {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(|findings| !findings.is_empty())
        .ok_or_else(|| anyhow!("captured evidence has no {field} array"))
}

fn copy_available_evidence(source: &Path, destination: &Path) -> Result<Vec<&'static str>> {
    fs::create_dir_all(destination)?;
    let mut missing = Vec::new();
    for name in ["metadata.json", "semantic.json", "text.json", "visual.png"] {
        if source.join(name).is_file() {
            fs::copy(source.join(name), destination.join(name))?;
        } else {
            missing.push(name);
        }
    }
    Ok(missing)
}

fn write_summary(
    output_directory: &Path,
    command: &str,
    status: &str,
    details: Value,
) -> Result<()> {
    let summary = json!({
        "schema_version": SCHEMA_VERSION,
        "command": command,
        "status": status,
        "details": details
    });
    write_json(&output_directory.join("summary.json"), &summary)?;
    println!("{}", serde_json::to_string(&summary)?);
    Ok(())
}

fn read_json(path: &Path) -> Result<Value> {
    serde_json::from_slice(
        &fs::read(path).with_context(|| format!("read JSON evidence {}", path.display()))?,
    )
    .with_context(|| format!("parse JSON evidence {}", path.display()))
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(path, bytes).with_context(|| format!("write JSON evidence {}", path.display()))
}

fn recreate_owned_directory(root: &Path, path: &Path) -> Result<()> {
    let path = validated_output_path(root, path)?;
    if path.exists() {
        if !path.is_dir() {
            bail!("UI output path is not a directory: {}", path.display());
        }
        let marker = path.join(OUTPUT_MARKER);
        let is_empty = fs::read_dir(&path)?.next().is_none();
        if !marker.is_file() && !is_empty {
            bail!(
                "refusing to delete unowned UI output directory {}; use a new directory beneath .tools",
                path.display()
            );
        }
        if marker.is_file() {
            let marker_value = fs::read_to_string(&marker)?;
            if marker_value != "Polyorama deterministic UI output v1\n" {
                bail!("invalid UI output ownership marker: {}", marker.display());
            }
            fs::remove_dir_all(&path)
                .with_context(|| format!("remove previous UI evidence {}", path.display()))?;
        }
    }
    fs::create_dir_all(&path)
        .with_context(|| format!("create UI evidence directory {}", path.display()))?;
    fs::write(
        path.join(OUTPUT_MARKER),
        "Polyorama deterministic UI output v1\n",
    )?;
    Ok(())
}

fn validated_output_path(root: &Path, path: &Path) -> Result<PathBuf> {
    let repository_root = normalise_absolute(&absolute(root)?)?;
    let tools_root = repository_root.join(".tools");
    let path = normalise_absolute(&absolute(path)?)?;
    if path == tools_root || !path.starts_with(&tools_root) {
        bail!(
            "UI output must be a dedicated directory beneath {}; observed {}",
            tools_root.display(),
            path.display()
        );
    }
    if tools_root
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        bail!("repository .tools directory must not be a symlink");
    }
    let relative = path.strip_prefix(&tools_root)?;
    let mut current = tools_root;
    for component in relative.components() {
        current.push(component.as_os_str());
        if current
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            bail!(
                "UI output path must not traverse a symlink: {}",
                current.display()
            );
        }
    }
    Ok(path)
}

fn normalise_absolute(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        bail!("expected an absolute path, observed {}", path.display());
    }
    let mut normalised = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalised.pop() {
                    bail!("path escapes the filesystem root: {}", path.display());
                }
            }
            _ => normalised.push(component.as_os_str()),
        }
    }
    Ok(normalised)
}

fn copy_directory(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()?.join(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_in_fixture_manifest_is_typed_and_valid() {
        let manifest: FixtureManifest =
            serde_json::from_str(include_str!("../../docs/ui-snapshots/fixtures.json")).unwrap();
        validate_manifest(&manifest).unwrap();
        assert_eq!(manifest.fixtures.len(), 5);
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        validate_evaluation_seed(&root).unwrap();
    }

    #[test]
    fn output_directory_is_mandatory() {
        let error = parse_arguments(vec!["list".into()]).unwrap_err();
        assert!(error.to_string().contains("explicit --output-dir"));
    }

    #[test]
    fn render_requires_a_closed_fixture_selection() {
        let error = parse_arguments(vec!["render".into(), "--output-dir".into(), "out".into()])
            .unwrap_err();
        assert!(error.to_string().contains("requires --fixture"));
    }

    #[test]
    fn automatic_baseline_update_flag_is_rejected() {
        let error = parse_arguments(vec![
            "verify".into(),
            "--output-dir".into(),
            "out".into(),
            "--update".into(),
        ])
        .unwrap_err();
        assert!(error.to_string().contains("unknown ui argument"));
    }

    #[test]
    fn empty_audit_requires_consistent_coverage_metadata() {
        let mut text = json!({ "audit": [], "observations": [] });
        assert!(read_text_coverage(&text).is_err());
        text["coverage"] = serde_json::to_value(TextAuditCoverage::default()).unwrap();
        assert_eq!(read_text_coverage(&text).unwrap().measured_components, 0);
        text["coverage"]["measured_components"] = json!(1);
        assert!(read_text_coverage(&text).is_err());
        text["coverage"]["measured_components"] = json!(0);
        text["coverage"]["observed_native_controls"] = json!(1);
        assert!(read_text_coverage(&text).is_err());
    }

    #[test]
    fn retained_audit_findings_are_classified_as_failures() {
        assert!(!has_findings(&json!({ "audit": [] }), "audit").unwrap());
        assert!(
            has_findings(
                &json!({ "audit": [{ "kind": "undeclared_overflow" }] }),
                "audit"
            )
            .unwrap()
        );
    }

    #[test]
    fn failure_bundle_contains_every_evidence_category() {
        let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
        let root = repository_root
            .join(".tools")
            .join(format!("xtask-ui-bundle-test-{}", std::process::id()));
        let expected = root.join("fixture-expected");
        let actual = root.join("fixture-actual");
        let failure = root.join("fixture-failure");
        recreate_owned_directory(&repository_root, &root).unwrap();
        fs::create_dir_all(actual.join("logs")).unwrap();
        fs::create_dir_all(&expected).unwrap();
        for name in ["metadata.json", "semantic.json", "text.json"] {
            write_json(&expected.join(name), &json!({ "side": "expected" })).unwrap();
            write_json(&actual.join(name), &json!({ "side": "actual" })).unwrap();
        }
        for path in [
            expected.join("visual.png"),
            actual.join("visual.png"),
            actual.join("visual-diff.png"),
        ] {
            fs::write(path, b"png").unwrap();
        }
        fs::write(actual.join("logs/runner.log"), b"evidence\n").unwrap();

        populate_failure_bundle(
            &repository_root,
            &failure,
            &expected,
            &actual,
            &VisualDiff {
                dimensions_equal: true,
                differing_pixels: 1,
                total_pixels: 4,
            },
        )
        .unwrap();

        for path in [
            "expected/visual.png",
            "actual/visual.png",
            "diff/visual.png",
            "diff/semantic.json",
            "diff/text.json",
            "logs/runner.log",
            "summary.json",
        ] {
            assert!(failure.join(path).is_file(), "missing {path}");
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn destructive_output_is_restricted_to_owned_tools_directories() {
        let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
        for unsafe_path in [
            repository_root.clone(),
            repository_root.join("docs"),
            repository_root.join(EXPECTED_ROOT),
            repository_root.join(".tools/../docs/ui-snapshots/expected"),
        ] {
            let error = recreate_owned_directory(&repository_root, &unsafe_path).unwrap_err();
            assert!(error.to_string().contains("beneath"));
        }

        let unowned = repository_root
            .join(".tools")
            .join(format!("xtask-ui-unowned-test-{}", std::process::id()));
        fs::create_dir_all(&unowned).unwrap();
        fs::write(unowned.join("user-data"), b"preserve").unwrap();
        let error = recreate_owned_directory(&repository_root, &unowned).unwrap_err();
        assert!(error.to_string().contains("refusing to delete unowned"));
        assert_eq!(fs::read(unowned.join("user-data")).unwrap(), b"preserve");
        fs::remove_dir_all(unowned).unwrap();
    }
}
