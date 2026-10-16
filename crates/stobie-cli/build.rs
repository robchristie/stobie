//! Record the Stobie revision this CLI is built from, which `stobie new` pins
//! new applications to. `STOBIE_SOURCE_REVISION` overrides Git.
//!
//! Cargo's build-script hashes ignore a git package's revision, and once a
//! script emits `rerun-if-changed`, Cargo checks only those paths, even when a
//! later `cargo install --git` reuses the target directory with a new checkout.
//! In a Cargo git checkout (marked by `.cargo-ok`) the script therefore emits
//! no `rerun-if` directive, and Cargo reruns it for each new revision. In a development checkout
//! it watches Git's HEAD and branch. It reads its directory at run time.

use std::path::{Path, PathBuf};
use std::process::Command;

fn git(directory: &Path, arguments: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(directory)
        .args(arguments)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("set by Cargo"));
    // Only a Stobie checkout names a Stobie revision, not, say, another
    // repository that vendors this source.
    let top = git(&manifest, &["rev-parse", "--show-toplevel"])
        .and_then(|top| Path::new(&top).canonicalize().ok())
        .filter(|top| {
            top.join("crates/stobie-cli/Cargo.toml").canonicalize().ok()
                == manifest.join("Cargo.toml").canonicalize().ok()
        });
    let revision = std::env::var("STOBIE_SOURCE_REVISION")
        .ok()
        .or_else(|| top.as_ref().and(git(&manifest, &["rev-parse", "HEAD"])));
    let cargo_checkout = top
        .as_ref()
        .is_some_and(|top| top.join(".cargo-ok").exists());
    // Any `rerun-if` directive replaces Cargo's own freshness rule.
    if !cargo_checkout {
        println!("cargo:rerun-if-env-changed=STOBIE_SOURCE_REVISION");
    }
    if let (false, Some(directory), Some(common)) = (
        cargo_checkout,
        git(&manifest, &["rev-parse", "--absolute-git-dir"]),
        git(&manifest, &["rev-parse", "--git-common-dir"]),
    ) {
        let common = manifest.join(common);
        let mut watched = vec![
            Path::new(&directory).join("HEAD"),
            common.join("packed-refs"),
        ];
        if let Some(reference) = git(&manifest, &["symbolic-ref", "-q", "HEAD"]) {
            watched.push(common.join(reference));
        }
        // A missing path would rerun the script on every build.
        for path in watched.iter().filter(|path| path.exists()) {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    if let Some(revision) = revision.filter(|revision| {
        revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
    }) {
        println!("cargo:rustc-env=STOBIE_SOURCE_REVISION={revision}");
    }
}
