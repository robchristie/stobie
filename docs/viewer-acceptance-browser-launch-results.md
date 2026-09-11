# One-context browser startup diagnostic result

The one authorised diagnostic completed on 11 September 2026. Chromium failed
before returning a usable context: **the singleton socket path was too long**.
This cause is proved for this fresh invocation. The earlier nine failures remain
unresolved individually; mask, quality, viewer and hardware acceptance remain
**missing-proof, nonterminal**.

Protocol commit: `a397f71d42a0d883f1db8f67548a4791e0298e9c`. All four committed protocol files
matched local bytes. The capsule and runner verified full Chromium 1234
(290,614,600 bytes; SHA-256 `0b20b130e7edd9dd51873be867761295fe0cfad490c2b9a64f95bd3cfc08fa71`), Node,
Playwright 1.62.1's six pinned API files, 71 resolved libraries including
the ELF interpreter, and 16 relevant environment variable hashes.
`ldd -r` returned 0 with empty stderr. The runner repeated preflight immediately
before launch. Complete file paths, sizes, real paths and hashes are retained in
the capsule; the JSON report includes executable and API identities.

Native stop authority was verified against
`viewer-acceptance-native-cohort-mansfield-01/operation-closeout.json`
(SHA-256 `f131baa4e8be5779866f24f8572ac7d0d10a05fc6ae3647557fa757a74159b4d`),
whose cleanup agrees with the committed native results. The grant records
`native_owner_stopped=true` and `no_native_hardware_overlap=true`; main confirmed
scheduling work only builds and prepares. Prelaunch inspection found no live
Chrome, emuella-viewer or Xvfb process.

Exactly one runner invocation and one persistent-context launch occurred: zero
usable contexts, Workers, jobs, services or retries. The unchanged four explicit
flags, Playwright defaults, long profile and TMPDIR layout were used. The
60-second launch and 90-second KILL outer bounds remained in place.

The bounded sanitised exception retains this decisive excerpt:

> Socket path too long: /nvme/development/emuella/emuella-testdata/artifacts/rareplanes-expanded-v1/viewer-acceptance-browser-masks-fullscene-20260911t074537z/tmp/org.chromium.Chromium.uC2RnJ/SingletonSocket.

The socket pathname is 183 UTF-8 bytes. Browser PID 3799554 exited with
`exitCode=null, signal=SIGABRT`; the outer process returned 2 without a signal or
timeout. Watcher elapsed time, including final cleanup observation, was
606 ms. Seven distinct owned processes were observed: timeout, Node,
three Chrome processes and two crashpad processes. This sampled count can miss
short-lived processes. None remained; no owner-issued stop was needed.
The runner's `cleanup_complete=false` records that no context was available to
close; the independent process observation confirms cleanup.

The retained exception is an 84-byte header and 4,096-byte tail, sanitised and
truncated from 11,304 UTF-16 code units (10,945 sanitised bytes). No raw event or
debug transcript was retained. Startup library identities do not cover future
`dlopen` calls. Historical environment parity is not established.

Both fresh attributed groups are direct children of
`/nvme/development/emuella/emuella-testdata/artifacts/rareplanes-expanded-v1`:

- Output: `viewer-acceptance-browser-masks-fullscene-20260911t074537z`.
- Execution, capsule, grant and stop receipt: `viewer-acceptance-browser-masks-fullscene-20260911t074537z-execution`.

SHA-256 identities:

| Evidence | SHA-256 |
| --- | --- |
| capsule.json | `a99d9284efe49632fd559baa82ef48c3cd08282fce62e295a3936b1d319dc3a6` |
| grant.json | `9253802a940cd80b7f4ea061dd0c4bced2eb7ed9d04f25555b9687cf2aca90e6` |
| result.json | `2c9f71154175ceabf08c9c444306b88c0655afb4c02f3049738538426bc436ff` |
| process.log | `ca9450bd9b7016a51ff9144e9223d6f86fb9e1ab87cab1ae1c3fb4ac883c2392` |
| stop-receipt.json | `15dde26e054d01f80f739ed04212e8b612234e12084022ec9bf57b3d2ae730af` |
| evidence-manifest.json | `20fb52f61ae7cce3f3a0bb9915c8109137c6f644b9b6f421df84ab657e286afe` |

The execution manifest lists all retained regular files and symlink targets at
closeout; the companion JSON provides full evidence paths and hashes. Original
failed groups were preserved.

One supported proposal: Under a new identity and separate main grant, provide a short writable TMPDIR whose Chromium singleton socket path fits the platform limit, with all writes retained inside an explicitly approved attributed store. Do not reuse or alter this group. No repair implemented.
Main decides any later finite functional proof under separately frozen inputs,
counts, deadlines and criteria. No second browser call or environment repair was
performed. Only these two repository result files were written; no code changes,
commits or pushes occurred. JSON, evidence hashes, error bounds and whitespace
were checked. Full verification would add an unauthorised browser smoke launch
and was excluded.
