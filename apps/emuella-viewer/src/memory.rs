//! Opt-in native phase markers and separate allocator observations, never an RSS sum.
use serde_json::{Value, json};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

const RECORD_LIMIT: usize = 256;
const BYTE_LIMIT: usize = 256 << 10;

pub struct MemoryMarkers {
    markers: File,
    allocations: File,
    sequence: usize,
    marker_bytes: usize,
    allocation_bytes: usize,
    last_ns: u64,
    start_time_ticks: u64,
    failed: bool,
}

fn bounded_read(path: &str, limit: usize) -> Result<String, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("proc read byte bound exceeded".into());
    }
    String::from_utf8(bytes).map_err(|e| e.to_string())
}
fn start_ticks(stat: &str) -> Result<u64, String> {
    stat.rsplit_once(')')
        .and_then(|(_, tail)| tail.split_whitespace().nth(19))
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| "unavailable process start identity".into())
}
fn label_valid(label: &str) -> bool {
    (1..=96).contains(&label.len())
        && label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.:-".contains(&b))
}
fn monotonic_ns() -> Result<u64, String> {
    #[cfg(target_os = "linux")]
    {
        let mut stamp = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // CLOCK_MONOTONIC is the Python collector's Linux clock, not Rust's elapsed origin.
        if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut stamp) } != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        u64::try_from(stamp.tv_sec)
            .ok()
            .and_then(|s| s.checked_mul(1_000_000_000))
            .and_then(|s| s.checked_add(stamp.tv_nsec as u64))
            .ok_or_else(|| "invalid monotonic clock".into())
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err("Linux CLOCK_MONOTONIC unavailable".into())
    }
}
fn rss_field(status: &str, name: &str) -> Value {
    let value = status.lines().find_map(|line| {
        let mut words = line.strip_prefix(name)?.split_whitespace();
        let value: u64 = words.next()?.parse().ok()?;
        (words.next()? == "kB" && words.next().is_none())
            .then(|| value.checked_mul(1024))
            .flatten()
    });
    value.map_or_else(
        || json!({"unavailable":"missing or invalid status field"}),
        |v| json!({"bytes":v}),
    )
}
fn allocator() -> Value {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        if std::env::var_os("LD_PRELOAD").is_some_and(|v| !v.is_empty()) {
            return json!({"unavailable":"LD_PRELOAD set; allocator interposition unresolved"});
        }
        // The binary uses Rust's default System allocator. These are glibc arena
        // statistics across threads, not all process allocations or physical pages.
        // Resolve optionally so opting out does not impose a new GLIBC_2.33
        // loader requirement on hosts whose system allocator lacks mallinfo2.
        let symbol = unsafe { libc::dlsym(libc::RTLD_DEFAULT, c"mallinfo2".as_ptr()) };
        if symbol.is_null() {
            return json!({"unavailable":"system allocator has no mallinfo2 symbol"});
        }
        let query: unsafe extern "C" fn() -> libc::mallinfo2 =
            unsafe { std::mem::transmute(symbol) };
        let m = unsafe { query() };
        let version =
            unsafe { std::ffi::CStr::from_ptr(libc::gnu_get_libc_version()) }.to_string_lossy();
        json!({"provider":"glibc.mallinfo2", "version":version,
            "arena_bytes":m.arena, "in_use_arena_bytes":m.uordblks,
            "free_arena_bytes":m.fordblks, "mmap_bytes":m.hblkhd,
            "mmap_regions":m.hblks, "top_releasable_bytes":m.keepcost,
            "boundary":"current glibc arenas; free includes retained blocks, top is a subset; tcache and foreign/driver allocations are not fully attributed; not RSS or Rust live bytes"})
    }
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    {
        json!({"unavailable":"identified glibc mallinfo2 provider unavailable"})
    }
}

impl MemoryMarkers {
    /// Called before graphics initialisation. Files are exclusive and never overwritten.
    pub fn from_env() -> Result<Option<Self>, String> {
        std::env::var_os("EMUELLA_VIEWER_MEMORY_MARKERS")
            .map(|p| Self::create(Path::new(&p)))
            .transpose()
    }
    pub fn create(path: &Path) -> Result<Self, String> {
        let start_time_ticks = start_ticks(&bounded_read("/proc/self/stat", 16 << 10)?)?;
        let markers = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        let allocations = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path.with_extension("allocations.jsonl"))
            .map_err(|e| e.to_string())?;
        Ok(Self {
            markers,
            allocations,
            sequence: 0,
            marker_bytes: 0,
            allocation_bytes: 0,
            last_ns: 0,
            start_time_ticks,
            failed: false,
        })
    }
    pub fn emit(&mut self, kind: &str, label: &str, lifetimes: Value) -> Result<(), String> {
        if self.failed {
            return Err("memory marker producer disabled after failure".into());
        }
        let result = self.emit_inner(kind, label, lifetimes);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn emit_inner(&mut self, kind: &str, label: &str, lifetimes: Value) -> Result<(), String> {
        if !label_valid(label)
            || ![
                "startup",
                "graphics-ready",
                "phase-start",
                "phase-settled",
                "cycle-evicted",
                "cycle-revisited",
            ]
            .contains(&kind)
        {
            return Err("invalid memory marker kind or label".into());
        }
        let ns = monotonic_ns()?;
        if ns < self.last_ns {
            return Err("memory marker clock inversion".into());
        }
        let marker = json!({"schema":"viewer_memory_phase_marker/1", "clock":"linux_monotonic",
            "pid":std::process::id(), "start_time_ticks":self.start_time_ticks,
            "monotonic_ns":ns, "sequence":self.sequence, "phase_label":label, "kind":kind});
        let status = bounded_read("/proc/self/status", 64 << 10);
        let rss = match status {
            Ok(s) => {
                json!({"current_rss":rss_field(&s,"VmRSS:"),"high_water_rss":rss_field(&s,"VmHWM:")})
            }
            Err(e) => json!({"unavailable":e}),
        };
        let observation = json!({"schema":"viewer_native_allocation_marker/1", "marker":marker,
            "instrument":"native-completion-memory-v1", "rss":rss, "allocator":allocator(),
            "lifetimes":lifetimes, "observation_finished_monotonic_ns":monotonic_ns()?,
            "physical_gpu_allocation":{"unavailable":"logical textures do not measure driver physical allocations"}});
        let mut marker = serde_json::to_vec(&marker).map_err(|e| e.to_string())?;
        marker.push(b'\n');
        let mut observation = serde_json::to_vec(&observation).map_err(|e| e.to_string())?;
        observation.push(b'\n');
        check_bounds(self.sequence, self.marker_bytes, marker.len())?;
        check_bounds(self.sequence, self.allocation_bytes, observation.len())?;
        self.markers.write_all(&marker).map_err(|e| e.to_string())?;
        self.allocations
            .write_all(&observation)
            .map_err(|e| e.to_string())?;
        self.marker_bytes += marker.len();
        self.allocation_bytes += observation.len();
        self.sequence += 1;
        self.last_ns = ns;
        Ok(())
    }
}
fn check_bounds(sequence: usize, bytes: usize, next: usize) -> Result<(), String> {
    if sequence >= RECORD_LIMIT || next > BYTE_LIMIT.saturating_sub(bytes) {
        Err("memory marker record/byte bound exhausted; coverage incomplete".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn proc_identity_labels_units_and_bounds_are_strict() {
        let stat = format!("42 (space ) name) S {} 12345 0", vec!["0"; 18].join(" "));
        assert_eq!(start_ticks(&stat).unwrap(), 12345);
        assert!(start_ticks("42 (truncated)").is_err());
        for label in ["", "has spaces", "slash/path", &"x".repeat(97)] {
            assert!(!label_valid(label));
        }
        assert!(label_valid("cycle-01.revisit:phase"));
        assert_eq!(rss_field("VmRSS: 12 kB", "VmRSS:")["bytes"], 12288);
        assert!(
            rss_field("VmRSS: 12 bytes", "VmRSS:")
                .get("unavailable")
                .is_some()
        );
        assert!(check_bounds(255, BYTE_LIMIT - 1, 1).is_ok());
        assert!(check_bounds(256, 0, 1).is_err());
        assert!(check_bounds(0, BYTE_LIMIT, 1).is_err());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn producer_writes_exact_schema_and_separate_observations_exclusively() {
        let path = std::env::temp_dir().join(format!(
            "viewer-authored-markers-{}-{}.jsonl",
            std::process::id(),
            monotonic_ns().unwrap()
        ));
        let mut writer = MemoryMarkers::create(&path).unwrap();
        writer.emit("startup", "startup", Value::Null).unwrap();
        writer
            .emit("graphics-ready", "graphics", Value::Null)
            .unwrap();
        assert!(MemoryMarkers::create(&path).is_err());
        let lines: Vec<Value> = std::fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].as_object().unwrap().len(), 8);
        assert_eq!(lines[0]["sequence"], 0);
        assert_eq!(lines[1]["sequence"], 1);
        assert!(lines[1]["monotonic_ns"].as_u64() >= lines[0]["monotonic_ns"].as_u64());
        let observations =
            std::fs::read_to_string(path.with_extension("allocations.jsonl")).unwrap();
        assert_eq!(observations.lines().count(), 2);
        assert!(
            writer
                .emit("phase-start", "invalid label", Value::Null)
                .is_err()
        );
        assert!(writer.emit("phase-start", "valid", Value::Null).is_err());
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_file(path.with_extension("allocations.jsonl")).unwrap();
    }
}
