# Merged viewing qualification protocol

**Prepared only; no execution grant.** Main owns Git, review/landing, merged
revision/build selection and grants. One operation owner/watcher serialises the
final operations below. No builds, services, browsers, native/GPU launches or
timed measurements during preparation; do not monitor the running mask proof.
Completion-pump candidate removal at `3d0565f` restores the production behaviour
of `fe82f85984fba5c75d7a5a8f10b2d914b773a838`. No encoding/resource repair,
re-encoding, rate retuning, warmups, added retries or replacement observations.

## Identity freeze before main's grant

Resolve owners from the live
`/nvme/development/emuella/emuella-workspace/workspace.local.toml`; the acceptance
workspace's local configuration currently agrees. Configured roots are
`/nvme/development/emuella/{emuella-j2k,emuella-benchmark,emuella-testdata}`;
Polyorama's configured owner is `/nvme/development/polyorama`, distinct from this
task checkout. Main selects its merged checkout explicitly. Keep these revisions:

| Owner | Full revision |
| --- | --- |
| Codec | `6586e3d50f95429b242cb2e3535742b002784f2d` |
| Benchmark | `70eb3d1d9448f617333d658d9a60bb5b9439ecb2` |
| Testdata | `89a1681d6ed6900d057d09a87cb34a57f2a4f30c` |

Set `STORE` to the configured testdata owner's
`artifacts/rareplanes-expanded-v1`. Rights/source evidence is the workspace's
`docs/evidence/viewer-acceptance/source-coverage.json`, SHA-256
`9bba0a0ae9064fa6450ceb2b56263a6715b562e7aa7a5ab5cd1adf138ea117e8`.
Retain notice, attribution and lineage. Sources, services' writable files,
temporary files, profiles, captures and logs stay in fresh approved groups;
no acquisition, terms changes, protected deletion or outside copies. Register
build scratch beneath configured `.build-targets` as code-only.

After merge, rebuild from the clean merged commit using the commands and external
target-path requirements in [reproduction](real-scene-viewing-reproduction.md).
Build release `emuella-viewer` and `emuella-viewer-tools`, then
`cargo xtask build-viewer-web`; retain new `measured-bin` copies before canonical
verification. Never relabel historical binaries. Before running, main binds full
commit/tree, Cargo.lock/toolchain, linked codec, build commands, native/service
binary and every WASM/static-file SHA-256; harness/preload hashes; Chromium
executable/build, Playwright package/lock, Node, loaded libraries and graphics
loader/ICD/driver hashes. Bind launch environment/flags, display identity and
actual native 1440×900×24 and browser 1440×900 geometry/DPR. Observe actual hardware
adapter/backend during each run; NVIDIA history is not fresh hardware proof.
No software substitution; unavailable GPU timing remains unavailable.

Revalidate all five native full-source mask/reference receipts and their source,
representation manifest/payload/descriptor/mask and quality/reference hashes from
[full-scene results](viewer-acceptance-full-scenes-results.json), SHA-256
`37463f135f58b90c8916274c700da32d24523b10480ce36357a3ad21dfc24bd2`.
Representation roots are `$STORE/viewer-acceptance-full-scenes-NAME-01/representation`,
where NAME is each ordered result's `name`. Do not prepare or decode again.
Compare the full-scene native owner `4a1594b0840eadae26a4d6005d50d9201734a1a7`
against merged source: retain semantic tree comparison of reference, masks,
quality and service code plus linked codec. Separately compare the frozen viewing
runtime and mask-proof runtime to merged source. If review changes meaningful
runtime code, main evaluates evidence invalidation **before grant**; historical
runs retain their original revisions.

Pin unchanged workload/catalogue/threshold files by these SHA-256s:

| Input under `apps/emuella-viewer` | SHA-256 |
| --- | --- |
| `real-scene-workload.json` | `9c11bead76a2c556dd49c13164dd7a462aa01e325ed6d3fb03510054e17f50dd` |
| `development-catalogue.json` | `0ba9c081775b5f7663119ee86ccadd0c86e054175c9fbaa26f1f856ca9388680` |
| `validation-catalogue.json` | `b8720229f298c30b194233d1b5df58c30c09c0010c379c26c9febe2efdc757a9` |
| `qualification/real-scene-native-thresholds.json` | `ad6d9e596dbc9e2aa5142973433e7bb4b184498a98c4dd5123262305998e08d0` |
| `qualification/real-scene-browser-thresholds.json` | `5883523c3ed7cab87e55c5470e885ebd1abf2dd78b845a62d3304cc0ee0f12e8` |
| `qualification/real-scene-pressure-thresholds.json` | `cb1c1b16429413861024b0a5f68a0641f0eeae835e147ac1b7f2d761f208e18a` |

Tok retains its historical ordered two-source catalogue: RGB8 primary and
Mansfield RGB16 comparison-only auxiliary. The historical contract at
`$STORE/real-scene-viewing-viewer-tok-journeys-01/browser-01/catalogue-contract.json`
has SHA-256 `d49ef376ab76e22f6e12e1b43fff1aa948160e6239e58db055f215e09f0f324a`.
Copy those exact contract bytes into the fresh execution group after checking
both rows against the retained full-scene manifests. Serve Tok RGB8 4 first and
Mansfield RGB16 12 second: source hashes, bands and full geometries remain equal
to the historical contract. The auxiliary's diagnostic rate is the declared
current RGB16 12 configuration; record its actual representation identity.
No new regression source, singleton substitution, workload change or admission
exception is introduced. Historical Tok detail failures remain unresolved until
proved otherwise; one new observation is not a repeated performance comparison.

## Later bounded execution commands

Main creates a fresh group named
`viewer-acceptance-browser-masks-merged-IDENTITY-execution` under `STORE` (the prefix
is required by the existing alias validator; this is a new viewing operation).
Set `EXECUTION` to it, create `tmp` mode 0700 and approved `cache/config/data`
directories, and pin `alias.json` using the exact alias object in
[browser environment](viewer-acceptance-browser-environment.md), with fresh
backing dev/inode and invoking host mount namespace. Set
`VIEWER_MERGED_ALIAS=$EXECUTION/alias.json` and its observed SHA-256 in
`VIEWER_MERGED_ALIAS_SHA256`. The preload validates physical backing, effective
fdinfo mount ID, private propagation, no submounts and output/XDG placement before
Playwright import. Each invocation has one browser launch; recovery's three fresh
contexts remain inside that immutable namespace. It retains per-invocation
`temporary-alias.json`. No global mount or harness/benchmark alteration.
Main's owned native display must use that same approved temporary backing, so
its X11 socket remains reachable through the private alias; retain socket/lock
files and process lifecycle evidence. Do not borrow the mask-proof service/display.
Inherited Playwright transient cleanup is unchanged; retain all remaining files,
perform no additional deletion, and bind that lifecycle explicitly in main's grant.

From main's merged checkout, set `VIEWER_BUILD`, `CODEC_CHECKOUT`,
`BENCHMARK_CHECKOUT`, `URL`, approved display/library environment, and `BENCH` to
the existing `native-cohort-benchmark-v1/cargo/release/emuella-benchmark` beneath
configured `.build-targets/viewer-acceptance`; binary SHA-256
`af0685cdba760f1a56db162fcfd087a1e32781c1a784690d733d1ba4c8f98ab7`.
The following function is a later command template, not an invocation:

```sh
journey() (
  mode=$1 label=$2 contract=$3 cache=$4; shift 4
  export TMPDIR=/tmp XDG_CACHE_HOME="$EXECUTION/cache"
  export XDG_CONFIG_HOME="$EXECUTION/config" XDG_DATA_HOME="$EXECUTION/data"
  unset NODE_OPTIONS
  if [ "$mode" != native ]; then
    export VIEWER_MERGED_ALIAS VIEWER_MERGED_ALIAS_SHA256
    export NODE_OPTIONS="--import=$PWD/tools/viewer-merged-qualification-preload.mjs"
  fi
  code=0
  unshare -Urm --propagation private sh -eu -c '
    mount --bind "$1/tmp" /tmp; shift; exec "$@"
  ' merged-viewing "$EXECUTION" python3 tools/viewer-composed-journey.py \
    --mode "$mode" --output "$EXECUTION/$label" --url "$URL" \
    --native-bin "$VIEWER_BUILD/measured-bin/emuella-viewer" \
    --web-root "$VIEWER_BUILD/web" --codec-repo "$CODEC_CHECKOUT" \
    --benchmark-repo "$BENCHMARK_CHECKOUT" --server-cache-state "$cache" \
    --catalogue-contract "$contract" "$@" >"$EXECUTION/$label-process.log" 2>&1 || code=$?
  printf '%s\n' "$code" >"$EXECUTION/$label-exit.txt"
)
```

Execute exactly this order, without overlapping service/display lifecycle work:

| Order / service catalogue | Mode / label | Cache state |
| --- | --- | --- |
| 1 Mansfield PAN4/RGB12, development contract | native / `mansfield-native` | uncontrolled-first-observation |
| 2 same service | browser / `mansfield-browser` | warm-server |
| 3 same service | recovery / `mansfield-recovery` | warm-server |
| 4 restart with Boca PAN4/RGB12, validation contract | browser / `boca-browser` | uncontrolled-first-observation |
| 5 same service | recovery / `boca-recovery` | warm-server |
| 6 restart with Tok RGB8 4 + Mansfield RGB16 12 auxiliary, historical contract | browser / `tok-browser` | uncontrolled-first-observation |

For normal rows append `--workload apps/emuella-viewer/real-scene-workload.json
--thresholds apps/emuella-viewer/qualification/real-scene-MODE-thresholds.json`.
For recovery append `--recovery-pressure real-scene-pan-sweep --thresholds
apps/emuella-viewer/qualification/real-scene-pressure-thresholds.json`; omit workload.
Use rebuilt service's existing `serve --representation PATH [--representation PATH]
--listen 127.0.0.1:PORT --verify-payload true --web "$VIEWER_BUILD/web"` route;
pin each delivered catalogue, process ownership and readiness without browser probes.
Retain existing 660-second process deadline, 64-window sweep, 1/4/16 MiB budgets
and seven recovery events. Exactly **3 normal browser launches + 2 recovery
launches / 6 recovery contexts**, and **one native normal journey**. The native
run confirms merged integration under original thresholds; it neither replaces
the observed five-start cohort nor repairs historical resource failures.

Admit each retained trace with `"$BENCH" journey "$EXECUTION/LABEL/composed-trace.json"
THRESHOLDS`, retaining stdout/stderr and exit. One observation supports absolute
results only, following the configured benchmark owner's `docs/composed-journeys.md`
and `docs/comparisons.md`; no repeated statistical speed claim. Count persistent
payload/descriptor/manifest/mask bytes separately from service deltas, actual
`wire.json` TCP application bytes and browser transfer counters. Preflight direct
HTTP lies outside the runner proxy; TCP excludes TCP/IP framing. Preserve failed
and unavailable fields; do not replace measured transport with file sizes.
Revalidate the same input inventories after execution and retain before/after
identities, all exit statuses and owned process termination evidence.

## Inspection, verification and closeout

Main opens inherited `browser-final.png` and recovery final captures; no extra
screenshot run. Record source region and phase/settlement evidence: script completion
alone is not proof every phase settled. Historical native 2/5-second captures can
be intermediate; distinguish them from settled images. Boca has genuine source
mask boundaries; Mansfield/Tok are fully valid and cannot prove boundary handling.
Relate visible regions to native/full-scene and separate browser mask evidence;
neither capture existence nor catalogue delivery proves mask correctness.

Source quality already fails. Tested failed predicates are **diagnostic rejection**;
missing/incomplete proof is **nonterminal**, not an observed pass or rejection of
an untested predicate. Preserve Tok historical detail failures (296.04 > 133.59 ms
native; 334.50 > 168.50 ms browser) unless new corresponding observations pass;
even then retain old results. No configuration selection/acceptance, reserved
validation, ML or human claim. Keep [native](viewer-acceptance-native-cohort-results.md)
and [scheduling](viewer-acceptance-scheduling-results.md) failures unchanged.

Preparation permits syntax, authored no-launch tests and whitespace checks only.
Canonical `cargo xtask verify` is separately main-owned and still required for
delivery. Existing `tools/ui-capture.mjs` creates `.tools/runtime/ui-x11-*`, binds
it over `/tmp` inside bwrap, then recursively removes it. An outer approved alias
is shadowed. The retention option and private mapping below preserve the
approved backing. No canonical run has yet been performed for this candidate;
scoped prose verification does not qualify this report. One owner PR can land verified mask/diagnostic implementation; final
merged qualification evidence belongs in the workspace terminal receipt and owner
PR comment, without a recursive owner closeout PR.

## Canonical temporary retention

`POLYORAMA_RETAIN_UI_TEMP=1` leaves the UI capture's owned temporary directory
for caller-managed retention and records `logs/temporary-lifecycle.json`. The
default cleanup behaviour is unchanged. For this campaign, bind fresh approved
backing to the ignored `.tools/runtime` and `.tools/tmp` paths within a private
mount namespace, and enable retention before canonical verification. Confirm
path identities and effective mounts; keep the backing directories afterwards.
The existing inner bwrap mapping then points at that same approved backing.
This setting changes diagnostic-file lifecycle only, not capture, workload,
renderer, comparison or acceptance behaviour. Use already-installed dependencies
and offline package caches; no bootstrap acquisition is authorised.
