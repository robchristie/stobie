//! Record the Stobie revision this CLI is built from, which `stobie new` pins
//! new applications to. `STOBIE_SOURCE_REVISION` overrides the checkout's.
//!
//! The revision comes from the `git` command when it runs, and otherwise from
//! the checkout's own files (`build/checkout.rs`), so `cargo install --git`
//! records it without Git installed: Cargo fetches with its own Git library.
//!
//! Cargo's build-script hashes ignore a git package's revision, and once a
//! script emits `rerun-if-changed`, Cargo checks only those paths, even when a
//! later `cargo install --git` reuses the target directory with a new checkout.
//! In a Cargo git checkout (marked by `.cargo-ok`) the script therefore emits
//! no `rerun-if` directive, and Cargo reruns it for each new revision. In a development checkout
//! it watches Git's HEAD and branch. It reads its directory at run time.

use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "build/checkout.rs"]
mod checkout;

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
    // repository that vendors this source: the checkout's top level, two
    // directories up, holds the Git directory.
    let top = manifest.join("../..").canonicalize().ok();
    let directories = top.as_deref().and_then(checkout::directories);
    let revision = std::env::var("STOBIE_SOURCE_REVISION").ok().or_else(|| {
        let top = top.as_deref()?;
        let directories = directories.as_ref()?;
        git(top, &["rev-parse", "HEAD"]).or_else(|| checkout::head_revision(directories))
    });
    let cargo_checkout = directories.is_some()
        && top
            .as_ref()
            .is_some_and(|top| top.join(".cargo-ok").exists());
    // Any `rerun-if` directive replaces Cargo's own freshness rule.
    if !cargo_checkout {
        println!("cargo:rerun-if-env-changed=STOBIE_SOURCE_REVISION");
    }
    if let (false, Some(directories)) = (cargo_checkout, &directories) {
        let mut watched = vec![
            directories.git.join("HEAD"),
            directories.common.join("packed-refs"),
        ];
        if let Some(reference) = checkout::head_reference(directories) {
            watched.push(directories.common.join(reference));
        }
        // A missing path would rerun the script on every build.
        for path in watched.iter().filter(|path| path.exists()) {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    if let Some(revision) = revision.as_deref().and_then(checkout::revision) {
        println!("cargo:rustc-env=STOBIE_SOURCE_REVISION={revision}");
    }
}
