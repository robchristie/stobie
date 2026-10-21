//! Read a Git checkout's revision from its files, for a build without the
//! `git` command, such as `cargo install --git` in a container image: Cargo
//! fetches and checks out with its own Git library, and its checkout is an
//! ordinary repository whose `HEAD` names a branch at the revision.
//!
//! Shared by `build.rs` and the CLI's unit tests.

use std::fs;
use std::path::{Path, PathBuf};

/// The top level of the Stobie checkout whose CLI manifest directory is
/// `manifest`, with its Git directories. Only a Stobie checkout names a Stobie
/// revision: source vendored into another repository, such as
/// `<repository>/vendor/stobie-cli`, sits under that repository's Git
/// directory but is not at `crates/stobie-cli` in it, so it names none.
pub fn stobie_checkout(manifest: &Path) -> Option<(PathBuf, GitDirectories)> {
    let top = manifest.join("../..").canonicalize().ok()?;
    let own = manifest.join("Cargo.toml").canonicalize().ok()?;
    if top
        .join("crates/stobie-cli/Cargo.toml")
        .canonicalize()
        .ok()?
        != own
    {
        return None;
    }
    let directories = directories(&top)?;
    Some((top, directories))
}

/// A checkout's Git directory (its own `HEAD`) and common directory (shared
/// refs), as Git's `--absolute-git-dir` and `--git-common-dir` name them.
#[derive(Debug, Eq, PartialEq)]
pub struct GitDirectories {
    pub git: PathBuf,
    pub common: PathBuf,
}

/// The Git directories of the checkout whose top level is `top`: `.git` is
/// the directory itself, or a file naming it (`gitdir: PATH`), as in a linked
/// worktree or a submodule, whose `commondir` names the shared directory.
pub fn directories(top: &Path) -> Option<GitDirectories> {
    let dot_git = top.join(".git");
    let git = if dot_git.is_dir() {
        dot_git
    } else {
        let text = fs::read_to_string(&dot_git).ok()?;
        let named = text.strip_prefix("gitdir:")?.trim();
        top.join(named)
    };
    if !git.join("HEAD").is_file() {
        return None;
    }
    let common = match fs::read_to_string(git.join("commondir")) {
        Ok(text) => git.join(text.trim()),
        Err(_) => git.clone(),
    };
    Some(GitDirectories { git, common })
}

/// The branch `HEAD` names, such as `refs/heads/main`, or `None` when it is
/// detached.
pub fn head_reference(directories: &GitDirectories) -> Option<String> {
    let head = fs::read_to_string(directories.git.join("HEAD")).ok()?;
    head.strip_prefix("ref:")
        .map(|reference| reference.trim().to_owned())
}

/// The full revision `HEAD` names: a detached `HEAD` holds it, and a branch's
/// is in its loose ref or in `packed-refs`.
pub fn head_revision(directories: &GitDirectories) -> Option<String> {
    let head = fs::read_to_string(directories.git.join("HEAD")).ok()?;
    let Some(reference) = head_reference(directories) else {
        return revision(head.trim());
    };
    // A branch's loose ref is in the common directory; only per-worktree
    // refs live beside a worktree's own `HEAD`.
    for directory in [&directories.common, &directories.git] {
        if let Ok(text) = fs::read_to_string(directory.join(&reference)) {
            return revision(text.trim());
        }
    }
    let packed = fs::read_to_string(directories.common.join("packed-refs")).ok()?;
    packed.lines().find_map(|line| {
        let (value, name) = line.split_once(' ')?;
        (name.trim() == reference)
            .then(|| revision(value))
            .flatten()
    })
}

/// `text` when it is a full 40-character hexadecimal revision.
pub fn revision(text: &str) -> Option<String> {
    (text.len() == 40 && text.bytes().all(|byte| byte.is_ascii_hexdigit())).then(|| text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAIN: &str = "0123456789abcdef0123456789abcdef01234567";
    const OTHER: &str = "89abcdef0123456789abcdef0123456789abcdef";

    /// An empty directory for one test, removed when it ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("stobie-checkout-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn write(&self, path: &str, text: &str) {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_cargo_checkout_names_its_branch_revision_from_a_loose_ref() {
        // Cargo's own checkout of a git dependency looks like this.
        let top = Scratch::new("loose");
        top.write(".git/HEAD", "ref: refs/heads/main\n");
        top.write(".git/refs/heads/main", &format!("{MAIN}\n"));
        top.write(".cargo-ok", "{\"v\":1}");
        let directories = directories(&top.0).unwrap();
        assert_eq!(directories.git, top.0.join(".git"));
        assert_eq!(directories.common, top.0.join(".git"));
        assert_eq!(
            head_reference(&directories).as_deref(),
            Some("refs/heads/main")
        );
        assert_eq!(head_revision(&directories).as_deref(), Some(MAIN));
    }

    #[test]
    fn a_packed_or_detached_head_names_its_revision() {
        let top = Scratch::new("packed");
        top.write(".git/HEAD", "ref: refs/heads/main\n");
        top.write(
            ".git/packed-refs",
            &format!(
                "# pack-refs with: peeled fully-peeled sorted\n\
                 {OTHER} refs/heads/mainline\n{MAIN} refs/heads/main\n^{OTHER}\n"
            ),
        );
        let directories = directories(&top.0).unwrap();
        assert_eq!(head_revision(&directories).as_deref(), Some(MAIN));

        top.write(".git/HEAD", &format!("{OTHER}\n"));
        assert_eq!(head_reference(&directories), None);
        assert_eq!(head_revision(&directories).as_deref(), Some(OTHER));
    }

    #[test]
    fn a_linked_worktree_reads_its_head_and_the_shared_refs() {
        let main = Scratch::new("main");
        main.write(".git/HEAD", "ref: refs/heads/main\n");
        main.write(".git/refs/heads/main", &format!("{MAIN}\n"));
        main.write(".git/refs/heads/topic", &format!("{OTHER}\n"));
        main.write(".git/worktrees/topic/HEAD", "ref: refs/heads/topic\n");
        main.write(".git/worktrees/topic/commondir", "../..\n");
        let linked = Scratch::new("linked");
        linked.write(
            ".git",
            &format!(
                "gitdir: {}\n",
                main.0.join(".git/worktrees/topic").display()
            ),
        );
        let directories = directories(&linked.0).unwrap();
        assert_eq!(directories.git, main.0.join(".git/worktrees/topic"));
        assert_eq!(
            directories.common.canonicalize().unwrap(),
            main.0.join(".git").canonicalize().unwrap()
        );
        assert_eq!(head_revision(&directories).as_deref(), Some(OTHER));
    }

    #[test]
    fn only_a_stobie_checkout_is_one() {
        let checkout = Scratch::new("stobie");
        checkout.write(".git/HEAD", &format!("{MAIN}\n"));
        checkout.write("crates/stobie-cli/Cargo.toml", "[package]\n");
        let (top, directories) = stobie_checkout(&checkout.0.join("crates/stobie-cli")).unwrap();
        assert_eq!(top, checkout.0.canonicalize().unwrap());
        assert_eq!(head_revision(&directories).as_deref(), Some(MAIN));

        // `cargo vendor` puts the crate two directories below the consumer's
        // own repository.
        let consumer = Scratch::new("consumer");
        consumer.write(".git/HEAD", &format!("{OTHER}\n"));
        consumer.write("vendor/stobie-cli/Cargo.toml", "[package]\n");
        assert_eq!(stobie_checkout(&consumer.0.join("vendor/stobie-cli")), None);

        // Source that is not a checkout at all.
        let unpacked = Scratch::new("unpacked");
        unpacked.write("crates/stobie-cli/Cargo.toml", "[package]\n");
        assert_eq!(stobie_checkout(&unpacked.0.join("crates/stobie-cli")), None);
    }

    #[test]
    fn anything_else_names_no_revision() {
        let top = Scratch::new("none");
        assert_eq!(directories(&top.0), None, "no Git directory");
        top.write(".git", "not a Git file\n");
        assert_eq!(directories(&top.0), None, "a .git file without gitdir");

        let top = Scratch::new("unborn");
        top.write(".git/HEAD", "ref: refs/heads/main\n");
        let directories = directories(&top.0).unwrap();
        assert_eq!(head_revision(&directories), None, "a branch with no commit");
        top.write(".git/HEAD", "abc123\n");
        assert_eq!(head_revision(&directories), None, "a short revision");

        assert_eq!(revision(MAIN).as_deref(), Some(MAIN));
        assert_eq!(revision(&MAIN[1..]), None);
        assert_eq!(revision(&format!("{}g", &MAIN[1..])), None);
    }
}
