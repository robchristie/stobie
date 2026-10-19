use std::os::unix::fs::{PermissionsExt as _, symlink};

use super::*;

struct Fixture {
    directory: PathBuf,
    input: PathBuf,
    output: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

impl Fixture {
    fn new(name: &str) -> Self {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/stobie-build-tests")
            .join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        let input = directory.join("input");
        fs::create_dir_all(input.join("pkg/snippets/helper")).unwrap();
        fs::write(
            input.join("index.html"),
            r#"<html><head></head><body><script type="module" src="bootstrap.js"></script></body></html>"#,
        )
        .unwrap();
        fs::write(
            input.join("bootstrap.js"),
            "import './pkg/app.js'; new Worker('worker.js', {type:'module'});\n",
        )
        .unwrap();
        fs::write(input.join("worker.js"), "import './pkg/app.js';\n").unwrap();
        fs::write(
            input.join("pkg/app.js"),
            "import './snippets/helper/module.js'; export const wasm = new URL('app_bg.wasm', import.meta.url);\n",
        )
        .unwrap();
        fs::write(
            input.join("pkg/snippets/helper/module.js"),
            "export const value = 42;\n",
        )
        .unwrap();
        fs::write(input.join("pkg/app_bg.wasm"), [0, 97, 115, 109, 1, 0, 0, 0]).unwrap();
        fs::create_dir_all(input.join("tests")).unwrap();
        fs::write(input.join("tests/ignored.test.mjs"), "ignored\n").unwrap();
        let output = directory.join("output");
        Self {
            directory,
            input,
            output,
        }
    }

    fn options(&self) -> Options {
        Options {
            apps: vec![App {
                name: "lab".into(),
                directory: self.input.clone(),
            }],
            output: self.output.clone(),
            variant: Variant::None,
            wasm_opt: "wasm-opt".into(),
            source_identity: Value::String("fixture".into()),
        }
    }
}

fn decompress(record: &Representations, output: &Path) {
    let raw = fs::read(output.join(&record.path)).unwrap();
    let br = fs::read(output.join(format!("{}.br", record.path))).unwrap();
    let gz = fs::read(output.join(format!("{}.gz", record.path))).unwrap();
    let mut unbr = Vec::new();
    brotli::BrotliDecompress(&mut &br[..], &mut unbr).unwrap();
    let mut ungz = Vec::new();
    flate2::read::GzDecoder::new(&gz[..])
        .read_to_end(&mut ungz)
        .unwrap();
    assert_eq!((unbr == raw, ungz == raw), (true, true), "{}", record.path);
    assert_eq!(record.raw, describe(&raw));
    assert_eq!(record.br, describe(&br));
    assert_eq!(record.gzip, describe(&gz));
}

#[test]
fn packaging_is_reproducible_preserves_relative_imports_and_round_trips() {
    let fixture = Fixture::new("reproducible");
    let options = fixture.options();
    let first = package(&options).unwrap();
    assert_eq!(package(&options).unwrap(), first);
    let app = &first.apps[0];
    assert_eq!(app.content_id.len(), 64);
    let html = fs::read_to_string(fixture.output.join(&app.entry.path)).unwrap();
    assert!(html.contains(&format!(
        "<head>\n<base href=\"./assets/{}/\">",
        app.content_id
    )));
    for record in app.assets.iter().chain([&app.entry]) {
        decompress(record, &fixture.output);
    }
    for relative in ["worker.js", "pkg/app.js", "pkg/snippets/helper/module.js"] {
        assert_eq!(
            fs::read(fixture.output.join(&app.asset_directory).join(relative)).unwrap(),
            fs::read(fixture.input.join(relative)).unwrap()
        );
    }
    assert!(
        !fixture
            .output
            .join(&app.asset_directory)
            .join("tests")
            .exists()
    );
    let manifest: Value =
        serde_json::from_slice(&fs::read(fixture.output.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["schemaVersion"], 1);
    assert_eq!(manifest["apps"][0]["contentId"], app.content_id.as_str());
    assert_eq!(manifest["settings"]["compression"]["brotliQuality"], 11);
    let different = Options {
        source_identity: Value::String("different-configuration".into()),
        ..options.clone()
    };
    assert_ne!(
        package(&different).unwrap().apps[0].content_id,
        app.content_id
    );
    fs::write(fixture.input.join("bootstrap.js"), "changed input\n").unwrap();
    let second = package(&options).unwrap();
    assert_ne!(second.apps[0].content_id, app.content_id);
    assert!(!fixture.output.join(&app.asset_directory).exists());
}

#[test]
fn missing_or_incompatible_optimiser_fails_before_replacing_output() {
    let fixture = Fixture::new("optimiser");
    let options = fixture.options();
    let previous = package(&options).unwrap();
    let with = |variant, wasm_opt: PathBuf| Options {
        variant,
        wasm_opt,
        ..options.clone()
    };
    let error = package(&with(Variant::Oz, fixture.directory.join("missing"))).unwrap_err();
    assert!(
        error.message.contains("wasm-opt 131 is required"),
        "{error}"
    );
    let fake = fixture.directory.join("wasm-opt");
    fs::write(&fake, "#!/bin/sh\necho 'wasm-opt version 130'\n").unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    let error = package(&with(Variant::O3, fake.clone())).unwrap_err();
    assert!(error.message.contains("Incompatible wasm-opt"), "{error}");
    fs::write(
        &fake,
        "#!/bin/sh\nif [ \"$1\" = --version ]; then echo 'wasm-opt version 131'; else exit 1; fi\n",
    )
    .unwrap();
    let error = package(&with(Variant::Oz, fake)).unwrap_err();
    assert!(error.message.contains("wasm-opt failed"), "{error}");
    let manifest: Value =
        serde_json::from_slice(&fs::read(fixture.output.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["apps"][0]["contentId"],
        previous.apps[0].content_id.as_str()
    );
    assert!(Variant::parse("O4").is_err());
}

#[test]
fn output_replacement_and_inputs_reject_unsafe_locations_and_symlinks() {
    let fixture = Fixture::new("unsafe");
    let options = fixture.options();
    let at = |output: PathBuf| Options {
        output,
        ..options.clone()
    };
    let isolated = |result: Result<Manifest, Error>| {
        assert!(result.unwrap_err().message.contains("isolated"));
    };
    isolated(package(&at(fixture.directory.clone())));
    isolated(package(&at(fixture.input.join("output"))));
    let unrelated = fixture.directory.join("unrelated");
    fs::create_dir_all(&unrelated).unwrap();
    fs::write(unrelated.join("keep"), "keep").unwrap();
    let error = package(&at(unrelated.clone())).unwrap_err();
    assert!(error.message.contains("unrelated output"), "{error}");
    symlink(unrelated.join("keep"), fixture.input.join("linked")).unwrap();
    let error = package(&options).unwrap_err();
    assert!(error.message.contains("Symlink"), "{error}");
    let invalid = Options {
        apps: vec![App {
            name: "Lab".into(),
            directory: fixture.input.clone(),
        }],
        ..options.clone()
    };
    assert!(package(&invalid).is_err());
}

#[test]
fn build_configuration_changes_identity_but_provenance_does_not() {
    let fixture = Fixture::new("identity");
    let with = |identity: Value| Options {
        source_identity: identity,
        ..fixture.options()
    };
    let first = package(&with(
        serde_json::json!({"revision": "first", "dirty": true, "profile": "release"}),
    ))
    .unwrap();
    let recorded = package(&with(
        serde_json::json!({"revision": "second", "dirty": false, "profile": "release"}),
    ))
    .unwrap();
    assert_eq!(first.apps[0].content_id, recorded.apps[0].content_id);
    assert_ne!(first.source_identity, recorded.source_identity);
    let configured = package(&with(
        serde_json::json!({"revision": "second", "dirty": false, "profile": "different"}),
    ))
    .unwrap();
    assert_ne!(recorded.apps[0].content_id, configured.apps[0].content_id);
}

#[test]
fn html_needs_a_head_and_no_base() {
    assert_eq!(
        add_base("a", "<html><HEAD lang=x><title>t</title>", "id").unwrap(),
        "<html><HEAD lang=x>\n<base href=\"./assets/id/\"><title>t</title>"
    );
    assert!(add_base("a", "<html><header></header></html>", "id").is_err());
    assert!(add_base("a", "<head><base href=/></head>", "id").is_err());
    // `<basefont>` is not a base element.
    assert!(add_base("a", "<head><basefont></head>", "id").is_ok());
}

/// The checks stage installs the pinned Binaryen and sets
/// `STOBIE_REQUIRE_WASM_OPT`, so this runs there; elsewhere set `WASM_OPT`.
#[test]
fn pinned_real_wasm_opt_builds_distinct_variants() {
    let wasm_opt = std::env::var_os("WASM_OPT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../.tools/browser/binaryen-version_131/bin/wasm-opt")
        });
    if !wasm_opt.exists() && std::env::var_os("STOBIE_REQUIRE_WASM_OPT").is_none() {
        eprintln!("skipping: no wasm-opt at {}", wasm_opt.display());
        return;
    }
    let fixture = Fixture::new("variants");
    let with = |variant| Options {
        variant,
        wasm_opt: wasm_opt.clone(),
        ..fixture.options()
    };
    let ids: BTreeSet<String> = [Variant::None, Variant::Oz, Variant::O3]
        .into_iter()
        .map(|variant| {
            let manifest = package(&with(variant)).unwrap();
            if variant != Variant::None {
                assert!(
                    manifest
                        .tool_versions
                        .wasm_opt
                        .as_deref()
                        .is_some_and(|version| version.starts_with("wasm-opt version 131"))
                );
            }
            manifest.apps[0].content_id.clone()
        })
        .collect();
    assert_eq!(ids.len(), 3);
}

#[test]
fn an_installable_page_gets_its_manifest_and_service_worker_at_the_web_root() {
    let fixture = Fixture::new("installable");
    fs::write(fixture.input.join("index.html"), crate::pwa::tests::PAGE).unwrap();
    crate::pwa::tests::write_icons(&fixture.input, [192, 512, 180]);
    // A development build's own files are never packaged as assets.
    fs::write(fixture.input.join("sw.js"), "// development\n").unwrap();
    fs::write(fixture.input.join("manifest.webmanifest"), "{}\n").unwrap();
    let packaged = package(&fixture.options()).unwrap();
    let app = &packaged.apps[0];
    let pwa = app.pwa.as_ref().expect("installable");
    assert_eq!(pwa.manifest.path, "lab/manifest.webmanifest");
    assert_eq!(pwa.service_worker.path, "lab/sw.js");
    decompress(&pwa.manifest, &fixture.output);
    decompress(&pwa.service_worker, &fixture.output);
    let assets: Vec<&str> = app
        .assets
        .iter()
        .map(|asset| {
            asset
                .path
                .strip_prefix(&format!("{}/", app.asset_directory))
                .unwrap()
        })
        .collect();
    assert!(assets.contains(&"icons/apple-touch-icon.png"));
    assert!(!assets.contains(&"sw.js") && !assets.contains(&"manifest.webmanifest"));
    // The base points into the asset directory; the manifest link climbs out.
    let html = fs::read_to_string(fixture.output.join(&app.entry.path)).unwrap();
    assert!(html.contains(&format!("<base href=\"./assets/{}/\">", app.content_id)));
    assert!(html.contains(r#"<link rel="manifest" href="../../manifest.webmanifest">"#));
    let manifest: Value =
        serde_json::from_slice(&fs::read(fixture.output.join("lab/manifest.webmanifest")).unwrap())
            .unwrap();
    assert_eq!(
        manifest["icons"][0]["src"],
        format!("assets/{}/icons/icon-192.png", app.content_id)
    );
    // The service worker caches the page, the manifest and every asset.
    let worker = fs::read_to_string(fixture.output.join("lab/sw.js")).unwrap();
    assert!(worker.contains(&format!("const VERSION = \"{}\";", app.content_id)));
    let precache: Vec<String> = serde_json::from_str(
        worker
            .lines()
            .find_map(|line| line.strip_prefix("const PRECACHE = "))
            .unwrap()
            .trim_end_matches(';'),
    )
    .unwrap();
    assert_eq!(&precache[..2], ["./", "manifest.webmanifest"]);
    assert!(worker.contains(r#"const FILES = new Set(["manifest.webmanifest"]);"#));
    assert_eq!(precache.len(), app.assets.len() + 2);
    for asset in &app.assets {
        let relative = asset.path.strip_prefix("lab/").unwrap();
        assert!(precache.iter().any(|path| path == relative), "{relative}");
    }
    let recorded: Value =
        serde_json::from_slice(&fs::read(fixture.output.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(
        recorded["apps"][0]["pwa"]["serviceWorker"]["path"],
        "lab/sw.js"
    );
    // Without the opt-in, the worker has no push handling and the package
    // manifest records none.
    assert!(!pwa.web_push && !worker.contains("'push'"));
    assert!(recorded["apps"][0]["pwa"].get("webPush").is_none());
}

#[test]
fn an_opted_in_page_gets_a_push_worker() {
    let fixture = Fixture::new("web-push");
    let page = crate::pwa::tests::PAGE.replace(
        "<title>",
        "<meta name=\"stobie-web-push\" content=\"enabled\"><title>",
    );
    fs::write(fixture.input.join("index.html"), page).unwrap();
    crate::pwa::tests::write_icons(&fixture.input, [192, 512, 180]);
    let packaged = package(&fixture.options()).unwrap();
    let pwa = packaged.apps[0].pwa.as_ref().expect("installable");
    assert!(pwa.web_push);
    decompress(&pwa.service_worker, &fixture.output);
    let worker = fs::read_to_string(fixture.output.join("lab/sw.js")).unwrap();
    assert!(worker.contains(r#"const NAME = "Field & Notes";"#));
    assert!(worker.contains("self.addEventListener('push'"));
    let recorded: Value =
        serde_json::from_slice(&fs::read(fixture.output.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(recorded["apps"][0]["pwa"]["webPush"], true);
}

#[test]
fn a_page_without_a_manifest_link_packages_as_before() {
    let fixture = Fixture::new("not-installable");
    let packaged = package(&fixture.options()).unwrap();
    assert!(packaged.apps[0].pwa.is_none());
    assert!(!fixture.output.join("lab/sw.js").exists());
    let recorded: Value =
        serde_json::from_slice(&fs::read(fixture.output.join("manifest.json")).unwrap()).unwrap();
    assert!(recorded["apps"][0].get("pwa").is_none());
}
