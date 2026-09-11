# Reproduce the bounded real-scene viewer

Use the committed Cargo lockfile, Rust toolchain and `wasm-bindgen` version
required by `xtask`. Install browser dependencies with `npm ci` and
`npx playwright install chromium`. Native requires a working X11/Wayland display
and Vulkan loader; browser qualification requires an actual hardware adapter.
The harness records the observed adapter. Software smoke does not qualify it.

Set `VIEWER_BUILD` to a registered build directory, `SCENE_EVIDENCE` to a fresh
`real-scene-viewing-viewer-final-*` group in the approved RarePlanes store,
`PAN_REPRESENTATION` and `RGB_REPRESENTATION` to the retained development 4 bpp
representation directories, and `CODEC_CHECKOUT`/`BENCHMARK_CHECKOUT` to the
intended owner checkouts. Keep the approved notice, attribution and lineage in
each evidence group. All image bytes, captures and reference payloads remain in
that store. Do not overwrite earlier runs. These diagnostic representations fail
the frozen display criteria; preparation is not repeated for decoder integration.
The original preparation route is `tools/viewer-real-scene-prepare.py --help` and
the [service contract](emuella-viewer-service.md); it needs the reviewed source
store and installed GDAL library, without a private orchestration script.

Build from a clean committed checkout and record `git rev-parse HEAD`,
`git rev-parse HEAD^{tree}`, `sha256sum Cargo.lock`, the commands and resulting
native/static hashes before running. The decoder pin is the `emuella-j2k-*` and
`emuella-jpip` source revision in `Cargo.lock`; the manifest's `codec_revision`
is the historical **encoder**. `--codec-repo` records an observed checkout HEAD,
which alone cannot attest the linked library. Keep the build record beside the
journey. Record runtime and harness revisions separately if they differ.

`xtask` currently reads WASM from the logical `target` path. When using an
external `CARGO_TARGET_DIR`, make `target` an ignored symlink to that same build
directory in an isolated checkout. Do not replace an existing build tree.

```sh
export CARGO_TARGET_DIR="$VIEWER_BUILD/cargo"
export POLYORAMA_VIEWER_WEB_DIR="$VIEWER_BUILD/web"
cargo build --locked --release -p emuella-viewer -p emuella-viewer-tools
cargo xtask build-viewer-web
mkdir "$VIEWER_BUILD/measured-bin"
cp "$CARGO_TARGET_DIR/release/emuella-viewer" "$VIEWER_BUILD/measured-bin/"
cp "$CARGO_TARGET_DIR/release/emuella-viewer-tools" "$VIEWER_BUILD/measured-bin/"
```

Retain these binaries before canonical workspace verification, whose feature
unification can change their hashes. In a separate terminal start the service:

```sh
"$VIEWER_BUILD/measured-bin/emuella-viewer-tools" serve \
  --representation "$PAN_REPRESENTATION" --representation "$RGB_REPRESENTATION" \
  --listen 127.0.0.1:8194 --verify-payload true --web "$VIEWER_BUILD/web"
```

After the coordinator grants the timing window, run one native and one browser
journey. This shell loop is the complete normal-journey orchestration; it does
not need unpublished scratch scripts. The native journey is the first service
observation; browser follows it with warm server state. Neither conditions OS
or physical-storage caches.

```sh
for mode in native browser; do
  cache=uncontrolled-first-observation
  if [ "$mode" = browser ]; then cache=warm-server; fi
  python3 tools/viewer-composed-journey.py \
    --mode "$mode" --output "$SCENE_EVIDENCE/$mode" \
    --url http://127.0.0.1:8194 \
    --native-bin "$VIEWER_BUILD/measured-bin/emuella-viewer" \
    --web-root "$VIEWER_BUILD/web" --codec-repo "$CODEC_CHECKOUT" \
    --benchmark-repo "$BENCHMARK_CHECKOUT" --server-cache-state "$cache" \
    --workload apps/emuella-viewer/real-scene-workload.json \
    --catalogue-contract apps/emuella-viewer/development-catalogue.json \
    --thresholds "apps/emuella-viewer/qualification/real-scene-$mode-thresholds.json"
done
```

For pressure, use the same command with `--mode recovery`, a fresh output,
`--recovery-pressure real-scene-pan-sweep`, no `--workload`, and
`real-scene-pressure-thresholds.json`. Run development and then restart the
service with the retained Boca pair and `validation-catalogue.json`. The explicit
64-window PAN sweep preserves 1/4/16 MiB pressure budgets and seven required
events; omitted pressure selection retains the inherited image/gallery default.
Do not repeat the rate bracket, Tok matrix or rejected presentation screen.

Admit each trace using the benchmark owner's `emuella-benchmark journey TRACE
THRESHOLDS` command. Extract the actual `worker.decoded_evidence` records from
application stages and browser/recovery snapshots, partition by immutable `tid`
and at most 1024 records, and run `emuella-viewer-tools reference
--representation PATH --requests RECORDS.json`. The [reference contract](emuella-viewer-service.md#complete-selected-tile-reference-comparison)
defines dimensions, precision, checksum and memory limits. Compare retained
payload/manifest/descriptor SHA-256 before and after journeys. Report absolute
latencies without a repeated statistical speed claim. Stop the owned service
and display after qualification.

For a directly executable extraction step, set `REFERENCE_INPUTS` to the paths
of each retained `app.json`, `app.json.stages.json`, `browser.json` and
`browser-recovery.json` being compared, and export `SCENE_EVIDENCE`. The following
writes unique observed records without generating reference checksums itself:

```sh
python3 - $REFERENCE_INPUTS <<'PYTHON'
import collections, json, os, sys
from pathlib import Path
records = collections.defaultdict(dict)
def visit(value):
    if isinstance(value, dict):
        for record in value.get('decoded_evidence', []):
            records[record['tid']][json.dumps(record, sort_keys=True)] = record
        for child in value.values():
            visit(child)
    elif isinstance(value, list):
        for child in value:
            visit(child)
for name in sys.argv[1:]:
    visit(json.loads(Path(name).read_text()))
output = Path(os.environ['SCENE_EVIDENCE']) / 'reference-requests'
output.mkdir(exist_ok=False)
for tid, unique in records.items():
    rows = list(unique.values())
    for offset in range(0, len(rows), 1024):
        path = output / f'{tid}-{offset // 1024:02}.json'
        path.write_text(json.dumps(rows[offset:offset + 1024], indent=2) + '\n')
PYTHON
```

Match each filename's `tid` to the representation manifest, then pass that file
to the documented `reference --representation PATH --requests FILE` command.
Retain its complete JSON result and exit status inside the same approved group.

Run `cargo xtask verify` on the final report-bearing commit. Keep its canonical
log hash in delivery metadata rather than adding another unverified tracked
receipt. On this protected-data host, the canonical logical `.tools/runtime`
path is bind-mounted to a fresh approved evidence group's `runtime` directory;
all normal UI ownership and path checks still apply. Final post-merge consumer
confirmation belongs to the coordinator.
