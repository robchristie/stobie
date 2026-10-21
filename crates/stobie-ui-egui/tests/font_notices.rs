//! The bundled faces, their licence texts and the crate's licence statement
//! agree with `assets/fonts/README.md`, which applications copy into their
//! third-party notices.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

fn fonts_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts")
}

fn sha256(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The README's table rows whose first cell is a code-quoted file name, keyed
/// by that name, with their remaining cells.
fn readme_rows() -> BTreeMap<String, Vec<String>> {
    let readme = fs::read_to_string(fonts_dir().join("README.md")).unwrap();
    readme
        .lines()
        .filter_map(|line| {
            let cells: Vec<String> = line
                .strip_prefix('|')?
                .strip_suffix('|')?
                .split('|')
                .map(|cell| cell.trim().to_owned())
                .collect();
            let file = cells.first()?.strip_prefix('`')?.strip_suffix('`')?;
            // A file of this directory, not an upstream path or a family.
            (file.contains('.') && !file.contains('/'))
                .then(|| (file.to_owned(), cells[1..].to_vec()))
        })
        .collect()
}

fn hash_cell(cells: &[String]) -> String {
    cells
        .last()
        .and_then(|cell| cell.strip_prefix('`')?.strip_suffix('`'))
        .unwrap_or_else(|| panic!("no hash cell in {cells:?}"))
        .to_owned()
}

#[test]
fn the_crate_declares_the_font_licence_with_its_own() {
    assert_eq!(env!("CARGO_PKG_LICENSE"), "Apache-2.0 AND OFL-1.1");
}

#[test]
fn every_bundled_file_is_listed_with_its_hash() {
    let rows = readme_rows();
    let mut files: Vec<String> = fs::read_dir(fonts_dir())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name != "README.md")
        .collect();
    files.sort();
    assert_eq!(
        files,
        rows.keys().cloned().collect::<Vec<_>>(),
        "README.md must list every bundled file, and only those"
    );
    for (file, cells) in &rows {
        assert_eq!(
            sha256(&fonts_dir().join(file)),
            hash_cell(cells),
            "{file}'s hash in README.md"
        );
    }
}

#[test]
fn every_face_names_an_ofl_licence_text_beside_it() {
    let rows = readme_rows();
    let faces: Vec<_> = rows.keys().filter(|file| file.ends_with(".ttf")).collect();
    assert_eq!(faces.len(), 3);
    for face in faces {
        let licence = rows[face]
            .get(1)
            .and_then(|cell| {
                cell.strip_prefix('[')?
                    .split_once("](")?
                    .1
                    .strip_suffix(')')
            })
            .unwrap_or_else(|| panic!("{face} names no licence file"));
        let text = fs::read_to_string(fonts_dir().join(licence)).unwrap();
        assert!(
            text.contains("SIL Open Font License") && text.contains("Version 1.1"),
            "{face}'s licence {licence} is not the OFL 1.1"
        );
    }
}
