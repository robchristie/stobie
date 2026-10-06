//! Explicit persistence for committed records and the canonical workspace.

use crate::model::{SavedState, decode, encode};

#[cfg(not(target_arch = "wasm32"))]
use crate::model::MAX_STATE_BYTES;
#[cfg(not(target_arch = "wasm32"))]
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[cfg(target_arch = "wasm32")]
const STORAGE_KEY: &str = "polyorama.record-desk.v1";

pub struct Store {
    #[cfg(not(target_arch = "wasm32"))]
    path: Result<PathBuf, String>,
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

impl Store {
    pub fn new() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            path: native_state_path(),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn at_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        Self {
            path: if path.as_os_str().is_empty() {
                Err("The Record Desk state path is empty.".into())
            } else {
                Ok(path)
            },
        }
    }

    pub fn description(&self) -> String {
        #[cfg(not(target_arch = "wasm32"))]
        {
            match &self.path {
                Ok(path) => path.display().to_string(),
                Err(error) => format!("Storage unavailable: {error}"),
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            format!("Browser local storage ({STORAGE_KEY})")
        }
    }

    pub fn load(&self) -> Result<Option<SavedState>, String> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = self.path.as_ref().map_err(Clone::clone)?;
            let file = match File::open(path) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(format!("Could not open {}: {error}", path.display())),
            };
            // Bound the actual read, including a file that grows after opening.
            let mut bytes = Vec::new();
            file.take((MAX_STATE_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
            if bytes.len() > MAX_STATE_BYTES {
                return Err(format!(
                    "Saved state at {} exceeds the 1 MiB limit.",
                    path.display()
                ));
            }
            let text = std::str::from_utf8(&bytes).map_err(|error| {
                format!("Saved state at {} is not UTF-8: {error}", path.display())
            })?;
            decode(text)
                .map(Some)
                .map_err(|error| format!("Could not load {}: {error}", path.display()))
        }
        #[cfg(target_arch = "wasm32")]
        {
            let text = browser_storage()?
                .get_item(STORAGE_KEY)
                .map_err(|error| format!("Could not read browser local storage: {error:?}"))?;
            text.as_deref().map(decode).transpose()
        }
    }

    pub fn save(&self, state: &SavedState) -> Result<(), String> {
        let text = encode(state)?;
        // Never replace a malformed or unsupported existing state, including
        // state changed externally since the application last loaded it.
        self.load()?;
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = self.path.as_ref().map_err(Clone::clone)?;
            let parent = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
            let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|error| {
                format!("Could not prepare a save in {}: {error}", parent.display())
            })?;
            temporary
                .write_all(text.as_bytes())
                .map_err(|error| format!("Could not write the temporary state file: {error}"))?;
            temporary.as_file().sync_all().map_err(|error| {
                format!("Could not synchronise the temporary state file: {error}")
            })?;
            temporary.persist(path).map_err(|error| {
                format!("Could not replace {}: {}", path.display(), error.error)
            })?;
            Ok(())
        }
        #[cfg(target_arch = "wasm32")]
        {
            browser_storage()?
                .set_item(STORAGE_KEY, &text)
                .map_err(|error| format!("Could not save to browser local storage: {error:?}"))
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn native_state_path() -> Result<PathBuf, String> {
    resolve_native_state_path(
        std::env::var_os("RECORD_DESK_STATE_PATH"),
        std::env::var_os("XDG_STATE_HOME"),
        std::env::var_os("HOME"),
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_native_state_path(
    override_path: Option<std::ffi::OsString>,
    xdg_state_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> Result<PathBuf, String> {
    if let Some(path) = override_path {
        return if path.is_empty() {
            Err("RECORD_DESK_STATE_PATH is empty.".into())
        } else {
            Ok(PathBuf::from(path))
        };
    }
    let base = xdg_state_home
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            home.filter(|path| !path.is_empty())
                .map(|home| PathBuf::from(home).join(".local/state"))
        })
        .ok_or_else(|| {
            "Set RECORD_DESK_STATE_PATH, XDG_STATE_HOME or HOME to choose a state location."
                .to_owned()
        })?;
    if !base.is_absolute() {
        return Err("XDG_STATE_HOME or HOME must identify an absolute state directory.".into());
    }
    Ok(base.join("polyorama-record-desk/state.json"))
}

#[cfg(target_arch = "wasm32")]
fn browser_storage() -> Result<web_sys::Storage, String> {
    web_sys::window()
        .ok_or_else(|| "Browser storage requires a window.".to_owned())?
        .local_storage()
        .map_err(|error| format!("Browser local storage is unavailable: {error:?}"))?
        .ok_or_else(|| "Browser local storage is unavailable.".to_owned())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::model::{Desk, VERSION, default_workspace};

    fn saved_state() -> SavedState {
        SavedState::from_desk(&Desk::default(), &default_workspace())
    }

    #[test]
    fn native_location_honours_explicit_and_xdg_paths_without_a_cwd_fallback() {
        let override_path = Some("/chosen/state.json".into());
        let xdg = Some("/state-root".into());
        let home = Some("/home/example".into());
        assert_eq!(
            resolve_native_state_path(override_path, xdg.clone(), home.clone()).unwrap(),
            PathBuf::from("/chosen/state.json")
        );
        assert_eq!(
            resolve_native_state_path(None, xdg, home.clone()).unwrap(),
            PathBuf::from("/state-root/polyorama-record-desk/state.json")
        );
        assert_eq!(
            resolve_native_state_path(None, Some("".into()), home).unwrap(),
            PathBuf::from("/home/example/.local/state/polyorama-record-desk/state.json")
        );
        assert!(resolve_native_state_path(None, None, None).is_err());
        assert!(
            resolve_native_state_path(Some("".into()), None, Some("/home/example".into())).is_err()
        );
        assert!(resolve_native_state_path(None, Some("relative".into()), None).is_err());
        assert!(resolve_native_state_path(None, None, Some("relative".into())).is_err());
        assert_eq!(
            resolve_native_state_path(Some("explicit-relative.json".into()), None, None).unwrap(),
            PathBuf::from("explicit-relative.json")
        );
    }

    #[test]
    fn native_save_creates_directories_and_roundtrips_replacements() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nested/state.json");
        let store = Store::at_path(&path);
        assert_eq!(store.load().unwrap(), None);
        assert_eq!(store.description(), path.display().to_string());
        let mut state = saved_state();
        store.save(&state).unwrap();
        assert_eq!(store.load().unwrap(), Some(state.clone()));
        state.records[0].title = "Saved replacement".into();
        store.save(&state).unwrap();
        assert_eq!(store.load().unwrap(), Some(state));
        let remaining = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(remaining, [std::ffi::OsString::from("state.json")]);
    }

    #[test]
    fn malformed_and_unsupported_existing_bytes_are_preserved() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.json");
        let store = Store::at_path(&path);
        let mut unsupported = saved_state();
        unsupported.schema_version = VERSION + 1;
        let cases = [
            b"{ malformed saved state".to_vec(),
            serde_json::to_vec(&unsupported).unwrap(),
            vec![0xff, 0xfe],
            vec![b' '; MAX_STATE_BYTES + 1],
        ];
        for original in cases {
            std::fs::write(&path, &original).unwrap();
            assert!(store.load().is_err());
            assert!(store.save(&saved_state()).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), original);
        }
    }

    #[test]
    fn duplicate_existing_records_and_invalid_replacements_are_preserved() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.json");
        let store = Store::at_path(&path);
        let valid = saved_state();
        store.save(&valid).unwrap();
        let original = std::fs::read(&path).unwrap();
        let mut invalid = valid.clone();
        invalid.records[1].id = invalid.records[0].id;
        assert!(store.save(&invalid).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        let duplicate_bytes = serde_json::to_vec(&invalid).unwrap();
        std::fs::write(&path, &duplicate_bytes).unwrap();
        assert!(store.save(&valid).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), duplicate_bytes);
    }

    #[test]
    fn write_failures_and_unavailable_paths_return_errors() {
        let directory = tempfile::tempdir().unwrap();
        let blocker = directory.path().join("a-file");
        std::fs::write(&blocker, "keep me").unwrap();
        let blocked = Store::at_path(blocker.join("state.json"));
        assert!(blocked.save(&saved_state()).is_err());
        assert_eq!(std::fs::read_to_string(blocker).unwrap(), "keep me");
        let target_directory = Store::at_path(directory.path());
        assert!(target_directory.load().is_err());
        assert!(target_directory.save(&saved_state()).is_err());
        let unavailable = Store::at_path("");
        assert!(unavailable.load().is_err());
        assert!(unavailable.save(&saved_state()).is_err());
        assert!(
            unavailable
                .description()
                .starts_with("Storage unavailable:")
        );
    }
}
