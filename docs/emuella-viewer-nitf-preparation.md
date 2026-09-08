# NITF preparation measurements

`tools/viewer-prepare-nitf.py` runs the actual `emuella-viewer-tools prepare`
command against an explicit maintained GDAL library and JP2Emuella plugin. It
requires the native source precision, selected bands, complete output profile,
full source revisions and retained build records. Revisions are declared build
provenance; binary and build-record hashes identify the supplied artefacts, but
the wrapper does not infer a binary's revision from its filename. Use build
records that establish that relationship.

The separate NITF baseline has no inherited synthetic workload bounds. A
successful baseline reports `completed: true, qualified: false`. Independent
fixture authorship must be established by the fixture's owning generation record;
this wrapper measures preparation and does not establish encoder independence or
vendor qualification.

NITF preparation requires the plugin's retained source-index capability. The
tool checks `JP2EMUELLA_SOURCE_INDEX=REQUIRED_SUPPORTED` on the registered driver
and selects `JP2EMUELLA_REQUIRE_SOURCE_INDEX=YES`. An older plugin cannot silently
fall back to repeated whole-source header traversal. The plugin's indexed profile
uses explicit header/marker/tile-part budgets; over-budget sources fail rather
than changing preparation architecture. Ordinary plugin callers retain the
legacy constructor unless they explicitly select indexed operation.

Set the variables below to existing authorised inputs, retained build records,
and a new directory under registered campaign scratch. `GDAL_LIBRARY` must name
the maintained library; `JP2EMUella_PLUGIN` must name its compatible plugin and
`CODEC_LIBRARY` the plugin's codec C API library.
Supply required loader directories through `LD_LIBRARY_PATH`. The wrapper passes
only the recorded loader/locale/GDAL data environment and its explicit settings
to preparation, including `GDAL_DRIVER_PATH`, disabled PAM and single-thread GDAL,
OpenMP and OpenBLAS settings.

```sh
python3 tools/viewer-prepare-nitf.py \
  --tool "$VIEWER_TOOL" --tool-revision "$VIEWER_REVISION" \
  --gdal-library "$GDAL_LIBRARY" --gdal-revision "$GDAL_REVISION" \
  --plugin "$JP2EMUella_PLUGIN" --plugin-revision "$PLUGIN_REVISION" \
  --codec-library "$CODEC_LIBRARY" --codec-revision "$CODEC_REVISION" \
  --build-record "$VIEWER_BUILD_RECORD" --build-record "$GDAL_BUILD_RECORD" \
  --build-record "$PLUGIN_BUILD_RECORD" \
  --input "$NITF_INPUT" --output "$NEW_OUTPUT" --target scientific-u11 \
  --width 1024 --height 1024 --bits 11 --bands 1 \
  --tile 512 --levels 5 --bpp 2 --measurement-mode observer
```

Dimensions must match the selected input. UInt16 storage containing unsigned
11-bit samples uses `--bits 11`; UInt16 RGB uses `--bits 16 --bands 1,2,3`.
Preparation retains the application's synchronous tile-window callback and
existing native Byte/UInt16 checks. No additional raster buffer is allocated by
the wrapper.

Observer mode uses Linux `strace -ff -yy -s 0` and attributes returned bytes to
the resolved original source pathname in `read`, `pread64`, `readv`, `preadv` and
`preadv2`. Positive returns count as read operations; zero and error returns have
separate counters. Buffer contents are omitted. Every traced task must terminate
successfully. Missing source reads, unattributed descriptors, incomplete trace
lines, source mappings/transfers and successful `io_uring_setup` fail closed.
Supported syscall coverage is Linux x86_64/aarch64. Source/library paths must be
literal printable ASCII without strace path delimiters or escapes. The trace
must also show mappings for the explicit GDAL library, plugin and codec C API.

These are syscall-level source-file totals, including rereads and the tool's
deliberate source SHA-256 scan. They are **not physical-device or NFS traffic**.
The report retains `source_hash_read_bytes` and `source_hash_ms` separately and
does not subtract a guessed hash contribution. Existing `logical_read_bytes`
and `logical_read_operations` describe native encoder tile callbacks;
`process_read_bytes`, `process_rchar` and `process_syscr` remain process-wide.

Observer wall time and the tool's elapsed time include strace overhead. Run a
separate invocation with `--measurement-mode performance` and a fresh output
directory for uninstrumented timing. That report has `source_syscalls: null`;
do not substitute observer timings for uninstrumented performance or claim the
performance invocation's source counts were measured. Source hashes and profiles
allow the coordinator to compare both invocations. The wrapper hashes the input
before timing and validates its hash again afterwards. Both wrapper scans are
outside the observed preparation; the tool's own deliberate hash scan remains
inside it.

`--source-cache-state uncontrolled` is the default: the wrapper's identity hash
pass precedes preparation without further cache conditioning. Select `cold-os`
or `warm-os` to condition this source file after hashing, immediately before
launching the timed process:

- `cold-os` calls `fsync` on the source descriptor, requests eviction with
  page-aligned `POSIX_FADV_DONTNEED` covering the complete file, then requires
  zero resident source pages. Advisory eviction alone does not establish cold
  state; unavailable residency or any remaining resident page rejects admission.
- `warm-os` performs an explicit sequential source scan using at most 1 MiB per
  buffer, records its bytes/operations/time separately, then requires every
  source page to be resident. Partial or unavailable residency rejects admission.

The source must be a non-empty regular file owned by the caller for residency
proof. `mincore` observes at most 64 MiB of virtual mapping at a time, with one
byte of residency data per page. Mappings use `PROT_NONE`, their contents are
never touched, and every mapping ends before preparation starts. Observations
include the final partial page. `source_cache.before` is sampled after
conditioning; `source_cache.after` is sampled as soon as the process exits,
before output validation or the wrapper's final source hash scan. Both record
page totals, resident pages, file metadata and monotonic observation times. The
launch timestamp makes the sampling-to-launch interval inspectable. Unavailable
observations are explicit in uncontrolled mode and fail either controlled mode.

These are **source-file OS page-cache snapshots**, not device or NFS cache
states. The wrapper does not drop global caches, pin pages or prevent concurrent
access/eviction. Snapshot residency may change after observation. A cold source
at process launch does not mean cold decoder tile reads: the preparation tool's
own deliberate hash scan can warm source pages before tile decoding. Its scan
remains part of timed preparation and source syscall totals.

The mechanism follows the Linux user-space interface documentation for
[`posix_fadvise`](https://man7.org/linux/man-pages/man2/posix_fadvise.2.html) and
[`mincore`](https://man7.org/linux/man-pages/man2/mincore.2.html). File-specific
eviction is advisory; mincore residency is only a snapshot.

Each fresh output contains `result.json` (the exact tool result), `stderr.txt`,
`representation/`, observer trace files when selected, and
`preparation-result.json`. The report binds source, binary, wrapper, build-record,
result, manifest, payload and descriptor hashes, all preparation flags, environment,
wall time and the tool's peak RSS. It validates the source/profile/codec identity,
NITF C8 route, payload and every descriptor. Reusing any output directory fails
before invocation. Failed output remains inspectable; a retry needs a new root.
Timeouts terminate the preparation process group.

After calibration, supply `--limits "$NITF_THRESHOLDS"` to qualify against a
separately frozen JSON document. Its schema is
`viewer-nitf-preparation-thresholds/1`; required fields are `frozen_unix_ms`,
`source_sha256`, `bands`, `measurement_mode`, `source_cache_state`, the exact manifest `profile`, and
non-empty `bounds`. Each bound maps an observed numeric field to either
`{"maximum": value}` or `{"exact": value}`. The freeze must precede invocation
and match source, bands, profile, measurement mode and requested source-cache
state. `source_cache_state` is required even for `uncontrolled`; cold, warm and
uncontrolled observations cannot share a freeze. No default limits apply.

Available bound fields include tool metrics, `wall_ms`, `encoded_bytes`,
`descriptor_to_encoded_ratio`, and (observer mode only) `source_read_bytes` and
`source_read_operations`. Freeze the chosen limits for time, peak RSS, tile-index
size, source syscall bytes/operations, callback bytes/operations, encoded size and
descriptor ratio against the representative NITF profile. Choose separate
observer and performance documents where both boundaries are required; numeric
limits are owned by calibration, not this harness. Missing observations and
non-finite bounds fail qualification.

Focused verification is:

```sh
python3 -m unittest discover -s tools/tests -p test_viewer_prepare_nitf.py -v
```

The tests include actual strace observations against a small project-authored
byte file and a child process, plus negative attribution, unsupported I/O,
output-reuse and freeze-identity cases. Source-cache tests use an authored file
with a partial final page, bounded mapping windows, real sequential warming and
file-specific eviction; unsupported eviction must reject cold admission. Negative
tests cover ineffective eviction, partial warmth, unavailable residency and
cache-state freeze mismatch. Actual NITF preparation is a separate
integration probe using the maintained builds and independently authored fixture.
