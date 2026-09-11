# Private temporary alias for the corrected browser mask proof

Status: **prepared, browser-unexecuted; missing-proof, nonterminal**. Main selected
one repair from the [fresh launch diagnosis](viewer-acceptance-browser-launch-results.md):
its 183-byte singleton socket pathname caused SIGABRT. The approved base itself
is 75 bytes; shortening a child cannot accommodate the Chromium suffix. A private
Linux mount alias makes that same suffix `/tmp/org.chromium.Chromium.XXXXXX/SingletonSocket`
(49 bytes). No new store, executable, Chromium argument, Playwright default,
source array, lossy-data repair or workload is selected.

Preparation question: can an isolated `/tmp` bind preserve approved physical
backing and reject an unmapped or mismatched alias? The representative probe is
an authored directory bind with device/inode and effective mount-ID checks, a
synthetic marker write, retained provenance and unchanged host `/tmp`. Polyorama
owns the code and this evidence; main owns the implementation decision, committed
candidate and execution grant. Exit preparation when these filesystem checks
pass. They prove the mapping mechanism, not Chromium startup or browser masks.

## Capsule and retained provenance

Keep schema `viewer-acceptance-browser-masks-input/1` and all existing required
fields. Add exactly this optional object, replacing the metadata placeholders
with observations from the fresh backing directory and the invoking host:

```json
{
  "temporary_alias": {
    "kind": "private-linux-tmp/1",
    "path": "/tmp",
    "backing": "/nvme/development/emuella/emuella-testdata/artifacts/rareplanes-expanded-v1/viewer-acceptance-browser-masks-private-tmp-01-execution/tmp",
    "dev": "DECIMAL_STAT_DEV",
    "ino": "DECIMAL_STAT_INO",
    "parent_mount_namespace": "mnt:[HOST_NAMESPACE_INODE]"
  }
}
```

Main exclusively creates that execution group and its `tmp` (mode 0700), then
writes `lineage.json` equal to the grant's `{operation_owner, attribution, lineage}`.
No existing group is reused. Include the approved rights/native owners and both
failed operation groups in lineage. Capsule and grant must resolve directly
inside this execution group; the backing must resolve to its literal approved
path. Record `stat` device/inode as decimal strings and `/proc/self/ns/mnt` via
`readlink`, without starting a browser. The new environment identity is
`private-linux-tmp/1` plus these pinned fields and the new capsule digest. The new
harness identity is main's full protocol commit and committed four-file SHA-256
map, including this document. Main commits the schema/protocol before issuing
the bounded grant; this preparation neither commits nor issues it.

Before Playwright import, and immediately before every persistent-context launch,
the runner checks that the current mount namespace differs from the pinned host,
both directory stats match the pinned device/inode, and `/tmp` is a private bind
with no submounts. `/proc/self/fdinfo` selects the effective mount ID, including
when the hidden host `/tmp` also appears in mountinfo. Ordinary host `/tmp`, a
symlink escape, wrong backing, shared propagation or missing mapping rejects.
`temporary-alias.json` in the output group and each attempted context records
the namespace identities, physical path, both directory identities, effective
mount IDs, matching alias/backing mountinfo lines and full mountinfo SHA-256.
The per-context result also carries this provenance. Schema-only `validate`
outside the namespace does not claim this live preflight has passed.

Node receives `TMPDIR=/tmp` before Playwright import; Chromium receives the same
verified value. Profiles, downloads, results, cache and config keep their ordinary
approved output paths. Temporary files are physically in execution-group `tmp`.
Namespace exit releases only the alias; it never removes the backing directory.
The helper performs no cleanup/deletion. Playwright's inherited transient-file
cleanup remains unchanged; preserve all remaining backing files and evidence.

## Exactly one later corrected invocation

Use the same frozen `fe82f85984fba5c75d7a5a8f10b2d914b773a838` WASM/static assets,
native evidence and five ordered representations from the original capsule
(SHA-256 `9062781ea078709bf384ed8b421006f5c1a12217dae066d9696238d7b005a44a`).
Keep the service implementation at `4a1594b0840eadae26a4d6005d50d9201734a1a7`,
binary SHA-256 `7ea60902498fbbfcac67f7d7785e42a38b4ce06741895ad916f8996c0f137102`,
and catalogue SHA-256 `49dfe454bdcce17dc2212e5de3ee1b130b9b7e92f02f2440dff80f419c67841d`.
Main owns service readiness and the fresh service process receipt; the previous
service was stopped. Do not assert historical PID liveness or launch a service
as part of this preparation. Keep full Chromium 1234 and Playwright 1.62.1 pinned.

The new grant retains **one invocation, nine serial contexts/Workers, 2,309 jobs,
1,883 catalogue masks, 47 forward + 47 revisit pressure jobs, zero warmups and
zero added retries**. Canonical plan digest stays
`45db6fd835feafebcb88aefc410a248943a32a2caf5893288c30f592b6d5def4`.
Bounds stay **60 seconds/job, 660/context, 6,000/proof, 6,100/outer**, with all
HTTP, memory and acceptance criteria inherited from the mask protocol. Main
binds the fresh capsule digest, protocol files, output name, plan and ownership
in the ordinary mask grant; confirms scheduling exclusion and stop authority;
and owns timeout/descendant cleanup observation. Missing evidence stays partial.

Set `PROTOCOL_COMMIT` to main's actual committed candidate. Main first prepares
and pins the capsule/grant above. This is the **only** corrected full-proof
command; it has **not been executed**:

```sh
OUTPUT_NAME=viewer-acceptance-browser-masks-private-tmp-01
EXECUTION_GROUP=/nvme/development/emuella/emuella-testdata/artifacts/rareplanes-expanded-v1/${OUTPUT_NAME}-execution
LD_LIBRARY_PATH=/home/rob/.cache/ms-playwright/chromium-1234/chrome-linux64:/nvme/development/polyorama/.tools/sysroot/usr/lib \
  timeout --signal=KILL 6100s unshare -Urm --propagation private sh -eu -c '
    mount --bind "$1/tmp" /tmp
    exec node tools/viewer-acceptance-browser-masks.mjs run \
      --capsule "$1/browser-masks-input.json" --grant "$1/browser-masks-grant.json" \
      --protocol-commit "$2" --output-name "$3"
  ' browser-mask-private-tmp "$EXECUTION_GROUP" "$PROTOCOL_COMMIT" "$OUTPUT_NAME" \
  >"$EXECUTION_GROUP/browser-masks-process.log" 2>&1
```

Run from the repository root. `-Urm` isolates user/mount namespaces and maps the
caller to namespace root; it deliberately omits `-n` so the frozen loopback
service remains reachable. Mount propagation changes only inside that namespace.
No repaired blank-page launch, startup sweep or intermediate browser call is
allowed. The previous nine failures and one diagnosis remain immutable; none is
relabelled successful. This remains `quality-rejected-diagnostic-only` browser
mask evidence; hardware viewer journeys and benchmarks remain separate.

## Read-only inspection of future harnesses

`tools/viewer-composed-browser.mjs:7` and `tools/viewer-browser-recovery.mjs:58`
call `chromium.launch` without temporary-path overrides; Playwright inherits
Node's temporary environment. A future separately granted wrapper can verify
the same private alias before importing either entry point and set `TMPDIR=/tmp`,
with approved output/XDG paths. Keep existing Chromium flags, workloads,
measurements and recovery events. Their ephemeral profiles follow Playwright's
temporary directory and cleanup; retention authority must be settled before
protected runs. No edits to either harness are made here.

`tools/ui-capture.mjs:77` uses `tmpdir()` for the system-library profile. Its
default Linux path instead creates `.tools/runtime/ui-x11-*` and binds that over
`/tmp` in separate bwrap invocations for Xvfb and Chrome; an outer alias alone
would be shadowed. The minimal future change is an optional verified approved
backing for that existing inner bind, validated inside each final namespace,
and preserving that backing instead of recursively removing it in `finally`
(lines 380–389). Keep profiles/output under approved paths and retain the same
display setup, flags, fixture, semantic captures and timing. Do not switch to
system libraries to avoid this routing issue. No canonical UI harness or
benchmark changes are included in this repair.

## Preparation verification

Run only authored checks in this no-browser/no-service window:

```sh
node --check tools/viewer-acceptance-browser-masks.mjs
node --check tools/tests/viewer-acceptance-browser-masks.test.mjs
POLYORAMA_TEST_PRIVATE_MOUNT=1 node --test --test-skip-pattern='authored real HTTP|proxy rejects' tools/tests/viewer-acceptance-browser-masks.test.mjs
node --test tools/tests/viewer-acceptance-browser-launch.test.mjs
```

The real mount test is opt-in for Linux hosts allowing unprivileged namespaces;
it uses only registered authored scratch, never the approved protected store.
Canonical verification includes browser smoke and is excluded from preparation.
An initial incorrect negative name filter ran the two inherited synthetic HTTP
tests, briefly opening and closing local test listeners; no browser, application
or protected-data service was launched. Subsequent verification explicitly skips
those tests. The initial mount probe rejected stacked `/tmp` records; the
corrected preflight resolves the active mount through fdinfo and passes.

Final authored checks on 11 September 2026: **20 mask tests passed**, including
the real private mount probe; **12 launch-diagnostic mock tests passed**; both
Node syntax checks and whitespace checks passed. Source base was
`5bcb8cce766625b41b296b2e82f3627addf93fe1` with the uncommitted runner SHA-256
`41922b0df02070180da33f8643a9730ebc9d10681866cce00f049a5c57a4696f`
and mask-test SHA-256
`f29fe2cf63364c5cf89562e99ae75ffb6da76a4bae358102e33601b372d7b408`.
Retain the alias mechanism for main's candidate; do not infer browser success.
Authored probe evidence is
`/nvme/development/emuella/.build-targets/viewer-acceptance/browser-proof/authored-tests-AKkW1o/mount-5ryb9P/mount-preflight.json`,
SHA-256 `c5b71095821d75b9156138a3962c9a72d1bbee31005e4a4aae9a52c66af9fa73`.
Both paths reported device `60`, inode `287799929`; the alias and backing had
distinct effective mount IDs `726` and `671`. The namespace changed from
`mnt:[4026531832]` to `mnt:[4026534687]` only inside the probe. These are synthetic
probe identities, not values to insert into the protected execution capsule.
