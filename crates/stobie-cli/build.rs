//! Record the Stobie revision this CLI is built from, which `stobie new` pins
//! new applications to. `STOBIE_SOURCE_REVISION` overrides Git.

use std::path::Path;
use std::process::Command;

fn git(arguments: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(arguments)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn main() {
    println!("cargo:rerun-if-env-changed=STOBIE_SOURCE_REVISION");
    let revision = std::env::var("STOBIE_SOURCE_REVISION")
        .ok()
        .or_else(|| git(&["rev-parse", "HEAD"]));
    if let (Some(directory), Some(common)) = (
        git(&["rev-parse", "--absolute-git-dir"]),
        git(&["rev-parse", "--git-common-dir"]),
    ) {
        let common = Path::new(env!("CARGO_MANIFEST_DIR")).join(common);
        // A missing path would rerun the script on every build.
        let mut watched = vec![
            Path::new(&directory).join("HEAD"),
            common.join("packed-refs"),
        ];
        if let Some(reference) = git(&["symbolic-ref", "-q", "HEAD"]) {
            watched.push(common.join(reference));
        }
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
