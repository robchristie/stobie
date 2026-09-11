# Real-scene viewer integration

The final viewer binds merged codec
`6586e3d50f95429b242cb2e3535742b002784f2d` (codec PR 106). Historical B/C
builds bind `dc8ff1f432e132c0fddbbd94d8bcccfd6f5cf7db`; prepared assets retain
that original encoder identity. The frozen campaign plan belongs to
the private workspace at checkpoint `1c469f7f21ef1355e47ef33a6a8b2e8ad605aa35`.
The source-coordinate freeze is [the factual JSON record](real-scene-viewing-source-views.json),
committed before compressed output inspection. B rejects the display profile; C
rejects its sole presentation candidate. Neither result is reopened by integration.
The [reproduction commands](real-scene-viewing-reproduction.md) use checked-in
tools. [The binding correction](real-scene-viewing-binding-correction.json)
separates two historical traces' observed codec checkout HEAD from their actual
linked decoder. Original records remain unchanged.

## Source and quality

`tools/viewer-real-scene-source.py` verifies the reviewed notice and prepared
manifest identities, reads original TIFFs in bounded strips, and derives exact
per-band nearest-rank 2nd/98th percentiles excluding nodata. It retains source
range and nodata facts without treating UInt16 storage as sensor precision.
The JSON freeze specifies four 256-square views per selected product and both
full-storage and percentile stretches, at 1:1 and arithmetic 4× reduction.
The reduced numerical comparison uses the same box average on original and
reconstructed full-resolution samples; it does not conflate that display
comparison with DWT-discard reconstruction performance.

The locally opened Mansfield object views contain small aircraft. The initially
labelled Boca PAN object view contains vehicles and their shadows; that factual
classification corrects the label without changing coordinates or thresholds.
Tok's fixed object candidate view contains small bright roadside/yard features;
no aircraft identity is asserted there. Nodata is present at Boca and absent
from the selected complete Mansfield/Tok products.

`tools/viewer-real-scene-prepare.py` invokes the existing native GTiff route on
the complete original source, with PAN band 1, RGB16 MS bands 5,3,2 and RGB8
PS-RGB bands 1,2,3. It records command, binary/library hashes, source identity,
wall time, callback bytes, RSS, encoded payload and every descriptor hash.
No raw input bridge is used. The new explicit `--retain-incomplete true` option
preserves failed preparation output for stores that prohibit deletion; default
preparation retains its existing cleanup behaviour. Failed groups are never reused.

`tools/viewer-real-scene-quality.py` uses bounded `reference-export` windows and
original TIFF bands. Source-domain errors are per-band with storage peaks,
observed ranges and separate nodata/valid subsets. Display errors and clipping
are separate at both stretches/scales; every fixed view and band must meet
RMSE ≤3 and 99th-percentile absolute error ≤12 in U8 display units. This is
same-codec direct-file reconstruction, not independent decoder evidence.
Opened reconstructed images, independent decoder agreement, human acceptance
and analytical/ML suitability are separate observations.

All image payloads, reference samples and captures stay in fresh
`real-scene-viewing-viewer-*` children of the existing approved RarePlanes store.
Each group retains the unchanged notice, attribution and modification/lineage
record. Only scripts, coordinates and numerical evidence belong in Git.

## Application workload

Native `--workload PATH` and the browser harness's optional third argument
install the same bounded JSON action sequence before its first action. The
inherited workload remains the default. The real-scene workload cycles the two
primary product identities while retaining five bookmarks, pan/zoom, clustered
and scattered detections, gallery movement, warm compressed reconstruction,
warm GPU revisits and simultaneous distinct image views.

`viewer-composed-journey.py --workload PATH --catalogue-contract PATH` requires
an exact ordered `sources` array of source SHA-256, bands, width and height.
It then checks complete observed primary-view residency for the full geometry;
the inherited default still requires nine parents and 121 large overview chunks.
The trace hashes the actual workload and retains the catalogue contract. Global
budgets, one worker, virtualisation, phase deadlines, stale-result and recovery
behaviour remain inherited from the [composed contract](emuella-viewer-calibration.md).

Development evaluates exactly 1, 2 and 4 bpp on Mansfield. Five native and five
actual hardware-browser development journeys precede the application threshold
freeze. A validation failure cannot retune the thresholds. If no rate passes
quality, retain an explicit rejection and measure application limitations on a
clearly identified rejected diagnostic representation; do not call that validation.
At the initial B checkpoint C remained unselected: entropy, synthesis, requested output and repeated-demand
observations inform the coordinator's later choice, without an optimisation here.

## Measured B result

**Reject the existing HT profile for the frozen real-scene display criteria.**
No common development rate passes PAN16 and RGB16. These are full original
scenes, not substituted crops; only the predeclared quality observations are
regional. The [numerical evidence](real-scene-viewing-evidence.json) binds source,
binaries, preparation, traces, complete references and protected evidence hashes.

| Development product | Rate (bpp) | Preparation wall (s) | Payload + descriptors (bytes) | Worst display RMSE / p99 (U8) | Result |
|---|---:|---:|---:|---:|---|
| Mansfield PAN16 | 1 | 1.866 | 1,694,558 | 15.42 / 41 | Reject |
| Mansfield PAN16 | 2 | 2.517 | 3,364,384 | 8.91 / 23 | Reject |
| Mansfield PAN16 | 4 | 2.867 | 6,703,018 | 2.75 / 7 | Numerical pass |
| Mansfield RGB16 | 1 | 0.314 | 108,761 | 42.43 / 112 | Reject |
| Mansfield RGB16 | 2 | 0.364 | 213,312 | 27.16 / 73 | Reject |
| Mansfield RGB16 | 4 | 0.414 | 422,195 | 18.70 / 50 | Reject |

Preparation peaks were 58,504–69,064 KiB process RSS on development. Complete
Boca PAN/RGB16 preparation at the rejected diagnostic 4 bpp rate took 3.418/0.564 s,
with 7,775,401/528,445 payload-plus-descriptor bytes and 99,136/73,936 KiB RSS.
Complete Tok RGB8 took 8.224 s, 8,491,936 bytes and 98,548 KiB RSS. These are
individual uninstrumented invocations with uncontrolled source cache state,
not statistical performance claims. Original hashing, GTiff callbacks, process
read counters and encoded bytes retain their distinct boundaries in the report.

Five native and five actual hardware-browser development runs completed all
30 phases without workload or global budget failures on the rejected 4 bpp pair.
The five earlier native attempts failed before rendering because a Vulkan loader
was absent from the library path; all remain retained. Adding the existing
Chromium Vulkan loader enabled NVIDIA RTX 3090/Vulkan native rendering. Chromium
151.0.7922.34 reported NVIDIA/Ampere without a device model. No software GPU result
is used as hardware evidence. The exact final native/browser runtime paths and
loader dependencies belong to the retained build/setup identity.

The application ceilings were frozen at `79038ea` after those ten runs and before
reserved scene execution, using no more than 1.25 times each development maximum
and preserving stricter inherited ceilings. Boca native/browser diagnostics meet
them. Tok native/browser complete but fail the frozen detail ceilings:
296.04 > 133.59 ms native and 334.50 > 168.50 ms browser. No rate or ceiling was
retuned. Since development rejected RGB16 at every rate, these reserved executions
are diagnostics, not validation of a selected display configuration.

All 36 independent Kakadu view invocations completed with matching dimensions
and explicit original precision. They differ from the viewer's complete-tile
reference by at most one stored code value; byte-exact independent agreement is
not claimed. Boca RGB16 also changes source nodata-zero samples: fixed nodata subsets show
maximum absolute errors up to 573 storage codes. The source TIFF and its nodata
metadata remain unchanged, but the lossy representation has no separate exact
nodata mask. Exact nodata preservation is therefore not established.

Same-codec direct-file reference checks match all 500 distinct
retained application records over 12,830,508 pixels across five representations.
This does not turn FNV checksums into exhaustive per-pixel comparison evidence.

Recovery observed real interrupted receipt, additional-request retry, reconnection,
cancellation acknowledgement and rejection of a delayed actual completion on
both development and Boca. The initial Boca probe is retained as failed: later
overlapping requests had already supplied the interrupted cache data before
Retry. The adapted probe holds subsequent JPP traffic offline until explicit
Retry and cycles the actual catalogue. All inherited recovery ceilings and the
mandatory actual-eviction condition remain unchanged. At that initial B checkpoint recovery
**failed**: the two-image pressure journeys did not exhaust the 1/4/16 MiB budgets,
so neither representation nor GPU eviction occurred. This is an unsupported
pressure case, not a waived gate or an application crash.

Opened source and reconstructed object views show retained PAN aircraft detail
at 4 bpp and colour distortion/ringing in RGB16, strongest at 1/2 bpp. Opened
actual browser final captures show the complete scene shapes and original Boca
nodata, but the UInt16 full-storage display is very dark. These observations do
not establish exhaustive per-view visual acceptance, human acceptance or
analytical/ML suitability. The numeric failure alone is sufficient for rejection.

## Initial C observations, before instrumentation

Warm compressed replay repeats exactly 30 native or 24 browser regions, with no
new JPP or descriptor bytes. It repeats all entropy and synthesis work; a warm
GPU revisit adds neither decode nor upload. Native warm replay requests 162,052
output samples while decoding 1,083,652 entropy coefficients and loading 287,470
synthesis coefficients (6.69× and 1.77× output). Browser requests 137,476 samples,
with ratios 5.38× and 1.69×. Legitimate wavelet support is not labelled a defect.

Across the five native runs, worker execution is 73.25–75.90 ms while whole-view
settlement takes 626.82–683.59 ms. Browser worker execution is 65.10–69.30 ms while
settlement takes 400.30–400.70 ms. The dominant measured elapsed boundary is
outside worker execution. Frame-paced serial dispatch/presentation is a concrete
next probe, while repeated reconstruction is also directly evidenced. Separate
entropy and synthesis wall times and physical GPU execution timing are unavailable;
the residual cannot all be attributed to one mechanism from these counters.
No C candidate was selected or implemented by this worker.

## Initial B verification and hand-off

Full `cargo xtask verify` passed: formatting, native/WASM Clippy, workspace tests,
architecture, release native/WASM builds, both browser smoke suites, five frozen
UI fixtures and native lab/gallery smoke. The source report retains the exact
command, successful log hash and earlier environment failures. Canonical
GL/llvmpipe smoke remains separate from B's actual NVIDIA measurements.
An isolated bind mount keeps the canonical logical `.tools` boundary while
retaining captures in the approved persistent store; existing UI path and ownership
checks were not weakened. Generated web package paths may be symlinks to the
registered build store and remain ignored.

The original benchmark owner at `f78c9c4edc2c606a0037b1445753830781726646` assessed
all 22 traces with its actual `journey` CLI. The ten successful development traces
are admitted calibration-only (exit 4 without frozen thresholds). Boca normal
traces are application-qualified (exit 0). Tok normal traces are admitted but
unqualified for detail latency; recovery remains unqualified for actual eviction.
The five initial native startup failures remain separate failed attempts.
All nine immutable representations retained identical payload and descriptor
hashes after viewing.

Preparation binds the installed GDAL 3.13.0 library by SHA-256; no maintained-GDAL
source-revision equivalence is asserted for that installed binary. The GTiff path
does not require the NITF plugin. Timed native binaries are retained separately
from canonical workspace builds, whose feature unification changes binary hashes.
No C implementation, independent review, Git publication, merge or terminal
campaign delivery was performed by this bounded worker.


## C instrumentation checkpoint

The bounded pacing probe was frozen by the workspace at
`024b514098ed9697b1846d17fc45a0d7f6d5fa43`.
The baseline retains the existing application behaviour, one worker and all cache
budgets. Worker execution now records start, reconstruction finish and publication;
UI diagnostics separately record message receipt, frame drain and next dispatch
in that same frame. Native timestamps share one monotonic process origin. Browser
realms use `performance.timeOrigin + performance.now()`; clock inversions remain
counted as invalid observations. The existing 128-event ring contains request
samples, while constant-size cumulative totals survive ring truncation. These
measure CPU-observed boundaries, not scanout or physical GPU execution.

The authorised execution phase built and verified this diagnostic baseline,
then ran five fresh native and five hardware-browser development journeys with
the existing full-scene workload and budgets. Candidate selection followed
those attributed observations.
Any selected candidate must be frozen before its five alternating development
pairs; promising results require the campaign's 20 fresh-process AB/BA pairs,
99% confidence and 5% practical warm-reconstruction gate, first-region and
complete-visible non-regression, unchanged resources and recovery. Selection
must precede reserved Boca/Tok confirmation. Production defaults remain intact, including if an opt-in candidate qualifies.

The explicit `--recovery-pressure real-scene-pan-sweep` workload replaces the
unsuccessful small-catalogue image/gallery cycle with
an eight-by-eight serpentine sweep of full-resolution primary windows, whose
longer dimension is 512 samples, over the largest single-component parent.
Existing select-image, zoom and pan intents establish each settled view. The
fixed 64-view cap and existing per-phase/outer deadlines bound the work. The
1 MiB compressed, 4 MiB decoded and 16 MiB GPU caps and mandatory representation
and GPU eviction checks are unchanged. Earlier failed attempts remain retained.
The inherited image/gallery default remains available. The selected pressure
name and both harness sources contribute to its recorded identity. This workload
change requires a new workload hash with the same frozen recovery
bounds before development/Boca execution using the existing frozen B binary.


## C attribution and candidate freeze

Five native and five hardware-browser baselines at `1b01bbad62850fd749bc9ac6aee4af596d5dae19`
completed all phases. Native warm reconstruction takes 627.61–666.41 ms:
543.74–584.90 ms is worker-publication-to-UI-receipt waiting, 70.93–78.48 ms
is worker execution, and 3.33–5.55 ms is same-frame drain-to-next-dispatch work.
All 150 native warm timing samples are valid. Browser settlement is 400.30–400.70 ms.
Browser cross-realm timestamps invert by up to 0.1003 ms in 1–15 warm samples
per run; strict aggregate rejection remains visible. All 24 warm request samples
are retained with zero ring drops. Their same-realm UI receipt-to-frame wait is
295.10–310.90 ms; precise browser cross-realm delivery totals are not established.
[The C evidence](real-scene-viewing-c-evidence.json) retains every baseline.

The historical freeze selected exactly one **native-only opt-in** candidate: `--present-mode auto-no-vsync`
changes only the WGPU surface present mode to `AutoNoVsync`, keeping the existing
LOW_LATENCY frame-latency setting. Default native and browser configuration,
repaint reasons, one-worker scheduling, cache resets, resource ceilings and
cancellation/stale/recovery rules remain unchanged. The attribution supports
probing this native presentation boundary; it is not already a qualified benefit
and does not establish GPU execution or scanout latency.

The freeze required five alternating fresh-process development pairs for cheap rejection:
reject at ≤5% mean warm-reconstruction benefit, any incomplete journey or resource
regression. If promising, run 20 new AB/BA fresh-process development pairs with
the same instrumented binary and explicit default/opt-in configuration. Apply the
benchmark owner's 99.5% marginal Student t intervals with conservative df19
critical value 3.287 and Bonferroni 99% ratio bounds: warm reconstruction's upper
relative bound must be below −5%; first-primary-region and complete-visible upper
bounds must be ≤+5%. All inherited native acceptance and recovery gates still
apply. Preserve failures and use no successful subset. This choice and these
criteria precede Boca/Tok confirmation; do not retune. Original defaults remain
intact even if the opt-in qualifies.

The explicit pressure workload now passes development and Boca on the original
frozen B binaries. Both record one representation eviction; GPU evictions are
47 and 119. Compressed peaks reach exactly 1 MiB, decoded peaks remain below
1.26 MiB and GPU peaks remain below 16 MiB. The benchmark owner admits both
against the new workload identity and unchanged bounds (exit 0). All seven
required recovery events occur. The two earlier pressure failures remain retained.


Each paired phase first retains one separately labelled default-configuration
service-conditioning journey, so both arms can truthfully use the inherited
`warm-server` classification. Conditioning is excluded from the five/20 pairs
by design, before measurement; it cannot replace a failed trial. Every measured
arm is a fresh application process with empty client caches, followed by the
same within-process warm-compressed reset in the frozen workload.


## C bounded decision

**Reject the native `AutoNoVsync` candidate.** The frozen five AB/BA screening
pairs complete every journey and pass all unchanged native admissions, but mean
warm reconstruction rises from 625.47 to 642.77 ms (+2.77%). First-region means
are 78.39/78.10 ms and complete-visible means are 748.61/729.88 ms. The screen's
conservative warm ratio interval is −11.63% to +18.61%; this is not a significant
regression claim, but it plainly fails the predeclared >5% benefit screen.
The 20-pair qualification and reserved C Boca/Tok confirmation are therefore not
run. No second candidate, presentation retuning or codec change is attempted.

The experimental native option and harness forwarding are removed from current
production source. Its exact source, binary and all ten measured traces plus
separate service conditioning remain retained. Timing diagnostics and the
explicit recovery pressure repair remain. The publication-to-UI wait is directly
observed; removing requested presentation synchronisation did not reduce it in
this bounded experiment. A narrower causal explanation remains unresolved.
This negative result closes the bounded C probe, not the integration campaign.


Final instrumented recovery also passes development and Boca, including actual
representation/GPU eviction and every required recovery event; both owner
admissions exit 0. Across the ten paired native trials, both arms reconstruct
exactly 30 warm regions and 162,052 samples with zero additional JPP bytes.
Their compressed, decoded, GPU and codec-workspace peaks are identical, and
worker concurrency remains one. All nine retained representations still match
payload, manifest and descriptor hashes after C viewing.

`cargo xtask verify` passes at restored-production source `faa26f62411c57467d02eec8dcc015565d0db39d`:
35 Python regressions, formatting, native/WASM lint, workspace tests, architecture,
release native/WASM builds, response-header checks, browser smoke, all five UI
fixtures and native lab/gallery smoke. Canonical software-GPU smoke is separate
from the NVIDIA application evidence. Build and capture paths, log hashes,
process completion and the exact candidate checkpoints are retained in the C JSON.
Final changes after that verification only reconcile evidence and this receipt.
At C closeout no owned measurement, display, service or build process remained.
Those builds used frozen `dc8ff1f432e132c0fddbbd94d8bcccfd6f5cf7db`. The final
integration now pins merged `6586e3d50f95429b242cb2e3535742b002784f2d`; review,
landing and post-merge consumer confirmation remain coordinator-owned.

## Final merged-decoder representative integration

The native viewer, service/reference tool and complete browser static root were
built from clean commit `211a714753da5c7c7b9bc959bfa6840ac0fd35b7`, tree
`e7e657dd1df50d116d88cd77d77e8c890be5edf2`, with merged codec
`6586e3d50f95429b242cb2e3535742b002784f2d`. The retained build record binds
Cargo.lock, commands and every runtime hash. Report consolidation follows these
journeys with unchanged application and harness source; these binaries are not
claimed to have been built at the later report commit.

[Final numerical evidence](real-scene-viewing-final-evidence.json) records one
development native journey, one actual hardware-browser journey, and explicit
development/Boca PAN pressure journeys. All four complete and pass their unchanged
benchmark-owner limits. Native reports RTX 3090/Vulkan; Chromium 151.0.7922.34
reports NVIDIA/Ampere without its device model. No rate bracket, historical Tok
matrix, second C candidate or repeated statistical qualification was run.

| CPU-observed boundary (ms) | Native | Hardware browser |
|---|---:|---:|
| First primary region resident | 89.55 | 137.40 |
| Complete overview and visible gallery | 720.28 | 520.10 |
| Detection detail | 86.04 | 134.50 |
| Gallery completion | 532.08 | 433.80 |
| Warm GPU/bookmark revisit | 48.35 | 34.00 |
| Warm compressed reconstruction | 619.93 | 400.20 |

These are absolute representative observations, not a statistical speed claim.
Both pressure journeys observe all seven required events, including interrupted
receipt, retry, connection loss, reconnect, actual stale-result rejection,
cancellation acknowledgement and eviction. Both evict a representation and GPU
textures within unchanged 1/4/16 MiB caps. The inherited image/gallery default is
unchanged, and initial failed pressure evidence remains retained.

The final decoder's complete selected-tile reference matches all 346 distinct
retained application records over 42,942,495 pixels across the four exercised
representations, including pressure windows. This remains same-codec FNV,
dimension and precision agreement. The reference report's `codec_revision` is
the representation's original encoder identity, not the linked decoder. All nine
historical representations retain identical payload, manifest and descriptor
hashes before and after this integration. Prepared assets were not regenerated.

The B display rejection, Tok detail failures, C candidate rejection, nodata and
browser clock limits, independent-decoder one-code boundary, unavailable physical
GPU timing and unestablished human/analytical acceptance remain unchanged.
All journey displays and services stopped. The final report-bearing commit is
the input to canonical `cargo xtask verify`; its exact commit and successful log
hash belong in delivery metadata. Independent review, landing and the final
post-merge consumer confirmation remain coordinator-owned.
