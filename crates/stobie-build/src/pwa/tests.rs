use std::path::PathBuf;

use super::*;

/// A page with every tag an installable page needs.
pub(crate) const PAGE: &str = r##"<!doctype html>
<html><head>
<meta charset="utf-8">
<meta name=viewport content="width=device-width, initial-scale=1, viewport-fit=cover">
<!-- <link rel="manifest" href="elsewhere.webmanifest"> is a comment -->
<title>Field &amp; Notes</title>
<meta name="apple-mobile-web-app-title" content='Notes'>
<meta name="description" content="Notes in the field">
<meta name="theme-color" media="(prefers-color-scheme: light)" content="#f4f6f7">
<meta name="theme-color" content="#0b0f12">
<script>if (1 < 2) document.title = "<link rel=manifest>";</script>
<link rel="manifest" href="manifest.webmanifest">
<link rel="icon" type="image/png" sizes="192x192" href="icons/icon-192.png">
<link rel="icon" type="image/png" sizes="512x512" href="icons/icon-512.png">
<link rel="apple-touch-icon" href="icons/apple-touch-icon.png">
</head><body><link rel="icon" sizes="64x64" href="ignored.png"></body></html>"##;

/// Write the page's icons, at their sizes unless `sizes` overrides them.
pub(crate) fn write_icons(directory: &Path, sizes: [u32; 3]) {
    fs::create_dir_all(directory.join("icons")).unwrap();
    for (name, size) in ["icon-192.png", "icon-512.png", "apple-touch-icon.png"]
        .into_iter()
        .zip(sizes)
    {
        let icon = placeholder_icon(size, [11, 15, 18], [79, 179, 191]);
        fs::write(directory.join("icons").join(name), icon).unwrap();
    }
}

fn scratch(name: &str) -> PathBuf {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/stobie-build-tests")
        .join(format!("pwa-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).unwrap();
    directory
}

#[test]
fn a_page_without_a_manifest_link_asks_for_nothing() {
    let page = "<html><head><title>Lab</title><link rel=icon href=data:,></head></html>";
    assert_eq!(declaration(page).unwrap(), None);
}

#[test]
fn the_head_declares_name_icons_and_colours() {
    let declared = declaration(PAGE).unwrap().expect("installable");
    assert_eq!(declared.name, "Field & Notes");
    assert_eq!(declared.short_name, "Notes");
    assert_eq!(declared.description.as_deref(), Some("Notes in the field"));
    assert_eq!(
        declared.theme_color, "#0b0f12",
        "the colour without a media query"
    );
    assert_eq!(declared.background_color, "#0b0f12");
    assert_eq!(
        declared.icons,
        [
            Icon {
                href: "icons/icon-192.png".into(),
                size: 192
            },
            Icon {
                href: "icons/icon-512.png".into(),
                size: 512
            },
        ],
        "body links are not head links"
    );
    assert_eq!(declared.apple_touch_icon, "icons/apple-touch-icon.png");
    // Only the link's value moves, so the rest of the page is unchanged.
    let moved = declared.with_manifest_href(PAGE, "../../manifest.webmanifest");
    assert_eq!(
        moved.replace("../../manifest.webmanifest", "manifest.webmanifest"),
        PAGE
    );
    assert!(moved.contains(r#"<link rel="manifest" href="../../manifest.webmanifest">"#));
}

#[test]
fn an_incomplete_page_names_what_is_missing() {
    let cases = [
        (
            r#"href="manifest.webmanifest""#,
            r#"href="app.json""#,
            "must name manifest.webmanifest",
        ),
        ("viewport-fit=cover", "", "viewport-fit=cover"),
        (
            r#"<meta name="apple-mobile-web-app-title" content='Notes'>"#,
            "",
            "short name",
        ),
        (r##"<meta name="theme-color" content="#0b0f12">"##, "", ""),
        (r#"sizes="512x512""#, r#"sizes="256x256""#, "512×512 icon"),
        (
            r#"<link rel="apple-touch-icon" href="icons/apple-touch-icon.png">"#,
            "",
            "apple-touch-icon",
        ),
    ];
    for (from, to, expected) in cases {
        let page = PAGE.replace(from, to);
        match declaration(&page) {
            Err(error) => assert!(
                error.message.contains(expected),
                "{from}: {}",
                error.message
            ),
            // Removing the plain theme colour leaves the light one.
            Ok(Some(declared)) if expected.is_empty() => {
                assert_eq!(declared.theme_color, "#f4f6f7")
            }
            other => panic!("{from}: {other:?}"),
        }
    }
}

#[test]
fn icons_must_be_pngs_of_their_size_inside_the_root() {
    let directory = scratch("icons");
    let declared = declaration(PAGE).unwrap().unwrap();
    write_icons(&directory, [192, 512, 180]);
    declared.check_icons(&directory).unwrap();
    assert_eq!(
        png_size(&fs::read(directory.join("icons/icon-512.png")).unwrap()),
        Some((512, 512))
    );
    write_icons(&directory, [192, 512, 192]);
    let error = declared.check_icons(&directory).unwrap_err();
    assert!(
        error.message.contains("192×192, not 180×180"),
        "{}",
        error.message
    );
    fs::write(directory.join("icons/icon-192.png"), "not a png").unwrap();
    assert!(
        declared
            .check_icons(&directory)
            .unwrap_err()
            .message
            .contains("not a PNG")
    );
    let escaping = declaration(&PAGE.replace("icons/icon-192.png", "../icon.png"))
        .unwrap()
        .unwrap();
    assert!(
        escaping
            .check_icons(&directory)
            .unwrap_err()
            .message
            .contains("inside the web root")
    );
    fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn the_manifest_and_service_worker_name_the_web_root() {
    let declared = declaration(PAGE).unwrap().unwrap();
    let manifest: serde_json::Value =
        serde_json::from_str(&declared.manifest("assets/abc/")).unwrap();
    for (key, value) in [
        ("id", "./"),
        ("start_url", "./"),
        ("scope", "./"),
        ("display", "standalone"),
        ("name", "Field & Notes"),
        ("short_name", "Notes"),
        ("theme_color", "#0b0f12"),
    ] {
        assert_eq!(manifest[key], value, "{key}");
    }
    let sources: Vec<_> = manifest["icons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|icon| {
            (
                icon["src"].as_str().unwrap(),
                icon["sizes"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        sources,
        [
            ("assets/abc/icons/icon-192.png", "192x192"),
            ("assets/abc/icons/icon-512.png", "512x512"),
            ("assets/abc/icons/apple-touch-icon.png", "180x180"),
        ]
    );
    let worker = service_worker("notes", "abc", &["./".into(), "assets/abc/app.js".into()]);
    assert!(worker.contains(r#"const APP = "notes";"#));
    assert!(worker.contains(r#"const VERSION = "abc";"#));
    assert!(worker.contains(r#"const PRECACHE = ["./","assets/abc/app.js"];"#));
    assert!(!worker.contains("__STOBIE_"));
}
