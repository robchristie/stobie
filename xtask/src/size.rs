//! `cargo xtask web-size`: the transfer sizes of web production packages,
//! raw and with Brotli and gzip, as a report rather than a gate. Without
//! `--package` it builds the repository's production package (the Lab and
//! the Gallery) and a template application made by `stobie new` against
//! this checkout, then reports both.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};
use stobie_build::size::{PackageSize, Report};

const USAGE: &str =
    "cargo xtask web-size [--package LABEL=DIR]... [--baseline REPORT.json] [--output REPORT.json]";

struct Options {
    packages: Vec<(String, PathBuf)>,
    baseline: Option<PathBuf>,
    output: PathBuf,
}

fn parse(arguments: &[String]) -> Result<Options> {
    let mut options = Options {
        packages: Vec::new(),
        baseline: None,
        output: PathBuf::from("target/web-size/report.json"),
    };
    let mut index = 0;
    while index < arguments.len() {
        let value = arguments
            .get(index + 1)
            .with_context(|| format!("{} requires a value; {USAGE}", arguments[index]))?;
        match arguments[index].as_str() {
            "--package" => {
                let (label, directory) = value
                    .split_once('=')
                    .filter(|(label, directory)| !label.is_empty() && !directory.is_empty())
                    .with_context(|| format!("--package {value:?} is not LABEL=DIR"))?;
                options.packages.push((label.into(), directory.into()));
            }
            "--baseline" => options.baseline = Some(value.into()),
            "--output" => options.output = value.into(),
            flag => bail!("unknown web-size option {flag}; {USAGE}"),
        }
        index += 2;
    }
    Ok(options)
}

pub fn run(arguments: Vec<String>) -> Result<()> {
    let mut options = parse(&arguments)?;
    if options.packages.is_empty() {
        let repository = PathBuf::from("target/web-size/browser-production");
        super::browser::build(vec!["--output".into(), repository.display().to_string()])?;
        let template = build_template(Path::new("target/web-size/template"))?;
        options.packages = vec![
            ("browser-production".into(), repository),
            ("template".into(), template),
        ];
    }
    report(
        &options.packages,
        options.baseline.as_deref(),
        &options.output,
    )
}

/// Report `packages`, compared with `baseline` when given: the Markdown
/// table on standard output and in a GitHub step summary when there is one,
/// the JSON report at `output` and the table beside it.
pub fn report(
    packages: &[(String, PathBuf)],
    baseline: Option<&Path>,
    output: &Path,
) -> Result<()> {
    let report = Report::new(
        packages
            .iter()
            .map(|(label, directory)| {
                Ok(PackageSize {
                    label: label.clone(),
                    apps: stobie_build::size::read(directory)?,
                })
            })
            .collect::<Result<_>>()?,
    );
    let baseline = baseline
        .map(|path| -> Result<Report> {
            let text = fs::read_to_string(path)
                .with_context(|| format!("read baseline {}", path.display()))?;
            serde_json::from_str(&text)
                .with_context(|| format!("parse baseline {}", path.display()))
        })
        .transpose()?;
    let table = report.markdown(baseline.as_ref());
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, serde_json::to_vec_pretty(&report)?)?;
    fs::write(output.with_extension("md"), &table)?;
    println!("Web package sizes in bytes (a report, not a gate):\n\n{table}");
    println!("wrote {}", output.display());
    if let Some(summary) = env::var_os("GITHUB_STEP_SUMMARY") {
        let mut text = fs::read_to_string(&summary).unwrap_or_default();
        text.push_str(&format!("### Web package sizes in bytes\n\n{table}\n"));
        fs::write(&summary, text)?;
    }
    Ok(())
}

fn target_directory() -> Result<PathBuf> {
    let metadata = stobie_build::cargo_metadata(None, false)?;
    Ok(PathBuf::from(
        metadata["target_directory"]
            .as_str()
            .context("cargo metadata has no target_directory")?,
    ))
}

fn output_of(command: &mut Command) -> Option<String> {
    let output = command.output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// The revision `stobie new` pins. Cargo fetches the pin even though the
/// local patch replaces it, so it must be a commit GitHub has: as in
/// `tools/outside-app-smoke.sh`, `STOBIE_OUTSIDE_PIN`, CI's own checkout, or
/// locally the merge base with `origin/main` when `HEAD` is unpushed.
fn pin() -> Option<String> {
    if let Ok(pin) = env::var("STOBIE_OUTSIDE_PIN") {
        return Some(pin);
    }
    let pushed = output_of(Command::new("git").args(["branch", "-r", "--contains", "HEAD"]))
        .is_some_and(|branches| !branches.is_empty());
    if env::var_os("GITHUB_ACTIONS").is_some() || pushed {
        return None;
    }
    output_of(Command::new("git").args(["merge-base", "HEAD", "origin/main"]))
}

/// Build the template application from `stobie new` against this checkout
/// with `stobie build-web --production`, and return its package directory.
fn build_template(output: &Path) -> Result<PathBuf> {
    super::run("cargo", &["build", "--release", "-p", "stobie-cli"])?;
    let target = target_directory()?;
    let stobie = target.join("release/stobie");
    let root = env::current_dir()?;
    // Outside the checkout, so that it is its own workspace.
    let scratch = env::temp_dir().join(format!("stobie-web-size-{}", std::process::id()));
    let application = scratch.join("size-template");
    let _ = fs::remove_dir_all(&scratch);
    let mut new = Command::new(&stobie);
    new.args(["new", "size-template", "--path"])
        .arg(&application)
        .arg("--stobie-path")
        .arg(&root);
    if let Some(pin) = pin() {
        new.args(["--stobie-rev", &pin]);
    }
    let result = (|| -> Result<PathBuf> {
        status(&mut new)?;
        let output = root.join(output);
        let mut build = Command::new(&stobie);
        build
            .args(["build-web", "--production", "--output"])
            .arg(&output)
            .current_dir(&application)
            // Shared with the outside-application smoke, so each reuses the
            // other's dependencies.
            .env("CARGO_TARGET_DIR", target.join("outside-app"));
        status(&mut build)?;
        Ok(output)
    })();
    let _ = fs::remove_dir_all(&scratch);
    result
}

fn status(command: &mut Command) -> Result<()> {
    println!("+ {command:?}");
    let status = command
        .status()
        .with_context(|| format!("run {command:?}"))?;
    if !status.success() {
        bail!("command failed with {status}: {command:?}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn options_name_packages_a_baseline_and_an_output() {
        let options = parse(&arguments(&[
            "--package",
            "gallery=target/browser-production",
            "--baseline",
            "before.json",
            "--output",
            "after.json",
        ]))
        .unwrap();
        assert_eq!(
            options.packages,
            [(
                "gallery".to_owned(),
                PathBuf::from("target/browser-production")
            )]
        );
        assert_eq!(options.baseline, Some(PathBuf::from("before.json")));
        assert_eq!(options.output, PathBuf::from("after.json"));
        let defaults = parse(&[]).unwrap();
        assert!(defaults.packages.is_empty());
        assert_eq!(
            defaults.output,
            PathBuf::from("target/web-size/report.json")
        );
        for bad in [
            &["--package", "gallery"][..],
            &["--package", "=dir"],
            &["--package"],
            &["--gate", "1"],
        ] {
            assert!(parse(&arguments(bad)).is_err(), "{bad:?}");
        }
    }
}
