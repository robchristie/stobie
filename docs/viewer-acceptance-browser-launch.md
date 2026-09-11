# One-context browser launch diagnosis

Status: **prepared, unexecuted**. Main owns the commit and a later execution
grant after the native owner stops. This preparation grants no launch. The
question is whether one fresh blank persistent context exposes the previously
lost launch stderr. Polyorama owns the helper, protocol and bounded result;
main owns cause assessment and any subsequent functional proof. Exit preparation
with authored mock tests and these frozen bounds. No downstream agents, browser,
application, service or hardware measurement is used in preparation.

The [nine failed records](viewer-acceptance-browser-masks-results.md) remain
immutable: full Chrome revision-directory 1234, correct `LD_LIBRARY_PATH`, zero
usable contexts. The separate diagnosis report is
`/nvme/development/emuella/.build-targets/viewer-acceptance/contexts/browser-launch-diagnosis-report.txt`.
Present `ldd -r` resolution succeeds; historical resolved-library bytes and the
complete inherited environment were not recorded. The truncated errors establish
no cause. A long Unix socket path is plausible, **not proved**.

## Frozen operation

Exactly **one invocation, one fresh persistent full-Chrome context, one launch
call, zero retries**, 60 seconds for launch and a **90-second hard outer timeout**
including preflight and close. The default persistent page is `about:blank`.
No navigation, page inspection, pixels, screenshots, events, debug streams,
traces, application assets, Worker, job, HTTP service or protected payload is
requested. Successful startup proves no GPU, quality, mask or viewer acceptance.
Chrome may initialise its ordinary internal processes; these are not GPU proof.

Use `/home/rob/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome`,
290,614,600 bytes, SHA-256
`0b20b130e7edd9dd51873be867761295fe0cfad490c2b9a64f95bd3cfc08fa71`.
Keep `headless: true`, downloads disabled, service workers blocked and the exact
four explicit arguments in `launchOptions`: `--no-sandbox`,
`--disable-background-networking`, `--disable-breakpad`,
`--disable-crash-reporter`. Playwright 1.62.1 supplies its unchanged defaults,
including `--headless`, `--remote-debugging-pipe` and `about:blank`. Pin the
installed API files below to bind those defaults; never use `ignoreDefaultArgs`,
headless shell, a channel override or added diagnostic flags. The flags are bound
by exact API bytes plus launch options, without capturing a raw launch transcript.

Use a new direct child of the approved store:
`/nvme/development/emuella/emuella-testdata/artifacts/rareplanes-expanded-v1/viewer-acceptance-browser-masks-fullscene-YYYYMMDDtHHMMSSz`.
The timestamp must differ from `20260911t072423z`. Retaining the old naming shape
is deliberate: equal component lengths, `scene-1/profile`, `scene-1/downloads`,
`tmp`, `cache` and `config`. `TMPDIR` and the two XDG homes stay in this new long
group. Do not silently substitute a short temporary directory. Path spelling,
host state and historically unrecorded environment remain limitations, even with
the same shape. The outcome belongs to a **new diagnostic identity**.

## Capsule, committed identity and one-probe grant

Main first commits exactly the helper, runner, authored launch tests and this
protocol. `FILES` in the runner defines the four committed SHA-256 bindings;
local bytes must match that full commit. This package makes no commit or push.
Main then prepares a fresh attributed approved execution group named as the output
group plus `-execution`; capsule, grant, process log and stop receipt stay there.
No protected payload is needed or copied. No original or failed group is removed.

`capsule.json`, schema `viewer-acceptance-browser-launch-input/1`, contains:

- `output_name`; `paths`: the exact object returned by `paths(output_name)`.
- `chromium`, `node`: executable identities including resolved real paths.
- `environment_sha256`: `environmentIdentity(environment(paths, inheritedEnv))`.
  This records relevant variable names and SHA-256 values, including loader,
  display, XDG, home, locale, proxy, browser, Node and graphics settings. Main
  retains the launch environment privately; never dump its values. The known
  library path below must match. Debug/preload/audit overrides are rejected, not
  silently removed. Do not infer historical parity for unrecorded variables.
- `launch_options`: `launchOptions(paths)` without `env` (bound separately).
- `resolved_libraries`: all paths returned by `resolvedLibraries(lddStdout)`,
  sorted, each with `{path, realpath, bytes, sha256}`. Use `/usr/bin/ldd -r` with
  the effective environment; require exit 0 and empty stderr. Include every
  transitive resolved library and the ELF interpreter; the virtual VDSO has no
  file hash. Missing/unknown loader lines reject. These are startup loader
  identities, not a claim to cover successful future `dlopen` loads.
- `playwright_core_files`: identities in this order under the repository's
  `node_modules/playwright-core`: `package.json`, `index.js`, `index.mjs`,
  `lib/bootstrap.js`, `lib/coreBundle.js`, `lib/utilsBundle.js`. The package must
  report 1.62.1. Hash installed bytes only; no external source acquisition or
  application/library source investigation is part of this protocol.

All identities use lowercase SHA-256 and byte lengths. Main may generate these
records through bounded local metadata/hash and loader inspection without
starting Chrome. The runner rechecks them immediately before execution. Its
`validate CAPSULE COMMIT` mode performs that same read-only preflight and prints
the capsule digest, committed file map and budget; it imports no Playwright and
launches no browser. Validation is not execution authority.

`grant.json`, schema `viewer-acceptance-browser-launch-grant/1`, requires:

- `disposition: "quality-rejected-diagnostic-only"`, `protocol_commit`,
  `protocol_files`, `capsule_sha256`, `output_name`.
- `budget`: exactly `{ "invocations": 1, "browser_launches": 1, "workers": 0,
  "jobs": 0, "services": 0, "retries": 0, "launch_ms": 60000,
  "outer_seconds": 90 }`.
- Nonempty `operation_owner`, `attribution`, and `lineage` (1–16 strings,
  including the original failed group and approved rights/evidence owner).
- `native_owner_stopped: true`, `no_native_hardware_overlap: true`, and
  `native_stop_receipt`: the exact retained stop confirmation reference. Main
  verifies the receipt and scheduling exclusion before issuing the grant; the
  runner checks these declarations, not the native owner's live process state.

The execution group's `lineage.json` must equal the grant's
`{operation_owner, attribution, lineage}`. The runner exclusively creates the
output group and writes lineage and identity before any launch. Once consumed,
the grant cannot be reused, including after setup failure or outer timeout.
Only main can grant a new identity. Preserve partial files and all nine failures.

After the commit, native stop, identity checks and explicit grant, main's exact
one-probe command is below. Set `EXECUTION_GROUP` and `PROTOCOL_COMMIT` to their
actual frozen values. The explicit KILL bounds the whole timeout process group
at 90 seconds with no extra grace period. There is no second call on failure.

```sh
LD_LIBRARY_PATH=/home/rob/.cache/ms-playwright/chromium-1234/chrome-linux64:/nvme/development/polyorama/.tools/sysroot/usr/lib \
  timeout --signal=KILL 90s node tools/viewer-acceptance-browser-launch.mjs run \
  "$EXECUTION_GROUP/capsule.json" "$PROTOCOL_COMMIT" \
  >"$EXECUTION_GROUP/process.log" 2>&1
```

## Retention and predeclared decision

Only a rejected launch retains exception text: sanitised header capped at 512
UTF-8 bytes plus tail capped at 4,096 UTF-8 bytes, original UTF-16 character
count, sanitised size, truncation/sanitisation flags and parsed exit/signal when
present. Strip terminal escapes, controls and bidi markers; redact known secret
environment values and recognised credentials before taking the tail. This is a
bounded exception excerpt, not event/debug transcript capture. Other failures
retain only generic status. Successful context close records a boolean.

The execution owner retains process exit/timeout and checks owned-process cleanup
without restarting anything. An absent terminal result or unconfirmed cleanup is
missing proof. The blank context alone does not identify why the earlier nine
failed; a generic closed-target error also supplies no cause.

**Frozen next-step rule:** if this one probe provides explicit cause evidence
(for example a socket pathname error or a named loader/initialisation failure),
main may select **one** directly supported environment repair under a new
identity and separately grant **one finite functional proof** with its own frozen
inputs, operation counts and deadlines. Record the causal excerpt, selected
repair and proof criteria before that work. No repair is implemented here.
Otherwise retain **missing-proof, nonterminal** status: no startup sweep,
alternative executable trial, short-TMPDIR experiment or retry-until-pass loop.
No codec/source acquisition, new terms, protected copying/deletion or native
hardware timing overlap is authorised.

Preparation check: `node --test tools/tests/viewer-acceptance-browser-launch.test.mjs`.
It uses authored strings and mocks only. Full `cargo xtask verify` includes a
browser smoke launch and is deliberately outside this preparation grant.
