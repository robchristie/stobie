//! Transfer sizes of production packages, read from each package's
//! `manifest.json`: for every application, its WebAssembly and everything it
//! serves, raw and with the package's Brotli and gzip encodings. This is a
//! report for comparing builds, not a gate.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Error;

/// The report's JSON schema version.
pub const SCHEMA_VERSION: u32 = 1;

/// Bytes of one file or a sum of files in each representation.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Bytes {
    pub raw: u64,
    pub br: u64,
    pub gzip: u64,
}

impl Bytes {
    fn add(&mut self, other: Self) {
        self.raw += other.raw;
        self.br += other.br;
        self.gzip += other.gzip;
    }
}

/// One packaged application's sizes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AppSize {
    pub name: String,
    /// Its `.wasm` assets, such as the application and a worker.
    pub wasm: Bytes,
    /// Everything it serves: the entry page, its assets and, for an
    /// installable page, the manifest and service worker.
    pub total: Bytes,
    /// How many files `total` counts.
    pub files: usize,
}

/// One production package's sizes, under a label such as `gallery`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PackageSize {
    pub label: String,
    pub apps: Vec<AppSize>,
}

/// A size report over one or more packages.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub schema_version: u32,
    pub packages: Vec<PackageSize>,
}

fn malformed(what: &str) -> Error {
    Error::new(
        format!("the package manifest has no {what}"),
        "pass a directory written by `stobie build-web --production` or `stobie package-web`",
    )
}

fn representation(record: &Value) -> Result<(&str, Bytes), Error> {
    let bytes = |name: &str| {
        record[name]["bytes"]
            .as_u64()
            .ok_or_else(|| malformed(&format!("{name} bytes")))
    };
    Ok((
        record["path"].as_str().ok_or_else(|| malformed("path"))?,
        Bytes {
            raw: bytes("raw")?,
            br: bytes("br")?,
            gzip: bytes("gzip")?,
        },
    ))
}

/// The sizes of every application in a package's `manifest.json`.
pub fn of_manifest(manifest: &Value) -> Result<Vec<AppSize>, Error> {
    let apps = manifest["apps"]
        .as_array()
        .ok_or_else(|| malformed("apps"))?;
    apps.iter()
        .map(|app| {
            let mut records = vec![&app["entry"]];
            records.extend(
                app["assets"]
                    .as_array()
                    .ok_or_else(|| malformed("assets"))?,
            );
            if let Some(pwa) = app.get("pwa").filter(|pwa| !pwa.is_null()) {
                records.extend([&pwa["manifest"], &pwa["serviceWorker"]]);
            }
            let mut size = AppSize {
                name: app["name"]
                    .as_str()
                    .ok_or_else(|| malformed("app name"))?
                    .to_owned(),
                wasm: Bytes::default(),
                total: Bytes::default(),
                files: records.len(),
            };
            for record in records {
                let (path, bytes) = representation(record)?;
                size.total.add(bytes);
                if path.ends_with(".wasm") {
                    size.wasm.add(bytes);
                }
            }
            Ok(size)
        })
        .collect()
}

/// The sizes in a package directory's `manifest.json`, or in a manifest file
/// given directly.
pub fn read(package: &Path) -> Result<Vec<AppSize>, Error> {
    let file = if package.is_dir() {
        package.join("manifest.json")
    } else {
        package.to_path_buf()
    };
    let text = fs::read_to_string(&file).map_err(|error| {
        Error::new(
            format!("read {}: {error}", file.display()),
            "build the package first with `stobie build-web --production`",
        )
    })?;
    let manifest = serde_json::from_str(&text)
        .map_err(|error| Error::new(format!("parse {}: {error}", file.display()), ""))?;
    of_manifest(&manifest)
}

fn cell(current: u64, baseline: Option<u64>) -> String {
    match baseline {
        Some(baseline) if baseline != current => {
            let delta = i128::from(current) - i128::from(baseline);
            let percent = if baseline == 0 {
                String::new()
            } else {
                format!(", {:+.1}%", delta as f64 * 100.0 / baseline as f64)
            };
            format!("{current} ({delta:+}{percent})")
        }
        Some(_) => format!("{current} (=)"),
        None => current.to_string(),
    }
}

impl Report {
    pub fn new(packages: Vec<PackageSize>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            packages,
        }
    }

    fn find(&self, label: &str, app: &str) -> Option<&AppSize> {
        self.packages
            .iter()
            .filter(|package| package.label == label)
            .flat_map(|package| &package.apps)
            .find(|candidate| candidate.name == app)
    }

    /// A Markdown table in bytes, with each change from `baseline` where it
    /// has the same package and application.
    pub fn markdown(&self, baseline: Option<&Report>) -> String {
        let mut text = String::from(
            "| Package | App | WASM raw | WASM Brotli | Total raw | Total Brotli | Total gzip | Files |\n\
             | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |\n",
        );
        for package in &self.packages {
            for app in &package.apps {
                let before = baseline.and_then(|report| report.find(&package.label, &app.name));
                let _ = writeln!(
                    text,
                    "| {} | {} | {} | {} | {} | {} | {} | {} |",
                    package.label,
                    app.name,
                    cell(app.wasm.raw, before.map(|size| size.wasm.raw)),
                    cell(app.wasm.br, before.map(|size| size.wasm.br)),
                    cell(app.total.raw, before.map(|size| size.total.raw)),
                    cell(app.total.br, before.map(|size| size.total.br)),
                    cell(app.total.gzip, before.map(|size| size.total.gzip)),
                    app.files,
                );
            }
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(path: &str, raw: u64, br: u64, gzip: u64) -> Value {
        serde_json::json!({
            "path": path,
            "raw": {"bytes": raw, "sha256": ""},
            "br": {"bytes": br, "sha256": ""},
            "gzip": {"bytes": gzip, "sha256": ""},
        })
    }

    fn manifest() -> Value {
        serde_json::json!({
            "schemaVersion": 1,
            "apps": [
                {
                    "name": "lab",
                    "entry": record("lab/index.html", 100, 40, 50),
                    "assets": [
                        record("lab/assets/x/pkg/lab_bg.wasm", 1000, 300, 400),
                        record("lab/assets/x/worker-pkg/worker_bg.wasm", 500, 150, 200),
                        record("lab/assets/x/pkg/lab.js", 200, 60, 70),
                    ],
                },
                {
                    "name": "notes",
                    "entry": record("notes/index.html", 10, 4, 5),
                    "assets": [record("notes/assets/y/pkg/notes_bg.wasm", 2000, 600, 800)],
                    "pwa": {
                        "manifest": record("notes/manifest.webmanifest", 30, 20, 25),
                        "serviceWorker": record("notes/sw.js", 300, 100, 120),
                    },
                },
            ],
        })
    }

    #[test]
    fn sizes_sum_wasm_and_every_served_file() {
        let sizes = of_manifest(&manifest()).unwrap();
        assert_eq!(
            sizes,
            [
                AppSize {
                    name: "lab".into(),
                    wasm: Bytes {
                        raw: 1500,
                        br: 450,
                        gzip: 600
                    },
                    total: Bytes {
                        raw: 1800,
                        br: 550,
                        gzip: 720
                    },
                    files: 4,
                },
                AppSize {
                    name: "notes".into(),
                    wasm: Bytes {
                        raw: 2000,
                        br: 600,
                        gzip: 800
                    },
                    total: Bytes {
                        raw: 2340,
                        br: 724,
                        gzip: 950
                    },
                    files: 4,
                },
            ]
        );
        assert!(of_manifest(&serde_json::json!({"apps": [{"name": "x"}]})).is_err());
    }

    #[test]
    fn the_markdown_report_shows_changes_from_a_baseline() {
        let current = Report::new(vec![PackageSize {
            label: "template".into(),
            apps: of_manifest(&manifest()).unwrap(),
        }]);
        let mut earlier = current.clone();
        earlier.packages[0].apps[1].wasm.br = 500;
        earlier.packages[0].apps[1].total.br = 624;
        earlier.packages[0].apps.remove(0);
        let text = current.markdown(Some(&earlier));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 4);
        assert_eq!(
            lines[2], "| template | lab | 1500 | 450 | 1800 | 550 | 720 | 4 |",
            "no baseline for this application"
        );
        assert_eq!(
            lines[3],
            "| template | notes | 2000 (=) | 600 (+100, +20.0%) | 2340 (=) | 724 (+100, +16.0%) | 950 (=) | 4 |"
        );
        let round_trip: Report =
            serde_json::from_str(&serde_json::to_string(&current).unwrap()).unwrap();
        assert_eq!(round_trip, current);
    }

    #[test]
    fn a_written_package_reports_what_its_manifest_records() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/stobie-build-tests")
            .join(format!("size-report-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("manifest.json"),
            serde_json::to_vec(&manifest()).unwrap(),
        )
        .unwrap();
        let expected = of_manifest(&manifest()).unwrap();
        assert_eq!(read(&directory).unwrap(), expected);
        assert_eq!(read(&directory.join("manifest.json")).unwrap(), expected);
        assert!(read(&directory.join("absent")).is_err());
        fs::remove_dir_all(&directory).unwrap();
    }
}
