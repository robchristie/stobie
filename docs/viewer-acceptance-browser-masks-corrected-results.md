# Corrected private-path browser mask proof

Status: **partial**, `quality-rejected-diagnostic-only`. Exactly one corrected
proof invocation executed all **nine serial contexts/Workers and 2,309 jobs**,
with zero warmups, extra browser probes or runner retries. It exited **2** after
**41.808784 seconds**, without a timeout. The unchanged deadlines were
60/660/6,000/6,100 seconds per job/context/proof/process.

All five scene contexts and all three required fault contexts completed their
diagnostic assertions. Pressure observed eviction and refetch, but failed the
required complete forward/revisit agreement predicate. No workload, cache limit
or retry was added after this result. This is missing proof, not terminal product
rejection or viewer acceptance.

| Context | Planned / executed jobs | Completed / Failed | Catalogue masks | Result |
| --- | ---: | ---: | ---: | --- |
| Mansfield PAN16 | 252 / 252 | 252 / 0 | 420 | Complete diagnostic |
| Mansfield RGB16 | 504 / 504 | 504 / 0 | 42 | Complete diagnostic |
| Boca PAN16 | 280 / 280 | 280 / 0 | 770 | Complete diagnostic |
| Boca RGB16 | 672 / 672 | 672 / 0 | 63 | Complete diagnostic |
| Tok RGB8 | 504 / 504 | 504 / 0 | 588 | Complete diagnostic |
| Missing / corrupt / stale mask | 3 / 3 | 0 / 3, expected | 0 | All three proved |
| Pressure forward | 47 / 47 | 27 / 20 | 0 | Partial |
| Pressure revisit | 47 / 47 | 27 / 20 | 0 | Partial |

The 2,212 scene jobs directly compared **23,501,248 sample values** and
**17,037,656 validity values**, including 2,333,016 invalid cells. Including the
54 successful pressure jobs, totals were **28,327,712 samples** and **19,349,648
validity values**, with **zero sample differences, false-valid, false-invalid
or nonbinary values**. All returned request identities and observed resource
bounds agreed. Failed jobs supplied no compared arrays; zero disagreement does
not establish agreement for those jobs.

All **1,883 catalogue masks**, spanning 269 tiles and seven levels, passed.
Worker traffic added 502 comparisons: **2,385 total**, **32,697,050 mask bytes**,
and **8,634 oracle-plane comparisons**, with zero byte differences, padding
errors, all-without-any errors or oracle disagreements. Catalogue bodies totalled
20,780,351 bytes. Boca catalogue masks contained 1,033 PAN and 2,832 RGB partial
band-cells. Native validity transitions were observed for Boca PAN and each Boca
RGB component; the retained Mansfield/Tok oracles have no transitions. All
component selections, reduced/native levels, cross-tile and clipped-edge coverage
predicates passed.

Missing returned HTTP 404; corrupt preserved 98,304 bytes and produced
`mask digest mismatch`; stale reached the service and returned HTTP 400 with
its stale-identity rejection. Each produced the expected actual Worker Failed
envelope, matching request, zero decode count and zero decoded publications.
All nine contexts had the expected terminal count, no unexpected envelopes,
page errors or Worker error events.

Pressure recorded **79 mask evictions**, **54 previously loaded keys refetched**,
and only **27/47 successful exact revisits**. Each phase retained 11 Failed
envelopes with `TypeError: Cannot read properties of undefined (reading 'samples')`
and nine with `Error: regional masks exceed admitted budget`. The precise missing
predicate is that **every one of the 47 forward and 47 revisit jobs must complete
with exact samples/validity, matching request and bounded metrics**. Eviction and
refetch alone are insufficient. Error strings are observations, not a newly
established root-cause diagnosis.

Proof HTTP counters recorded **8,704 requests**, **192,773,751 non-static body
bytes**, maximum concurrency two and zero proxy errors. Largest mask body was
98,304 bytes; largest context body total was 95,506,228 bytes. Each service
preflight fetched a separate 148,937-byte catalogue. The Worker transport
`retries` counter ended at five summed across scene contexts and 1,412 in
pressure; these are inherited bounded continuation rounds, not runner retries.

Normal peak compressed residency was **3,765,101 bytes** against 64 MiB;
pressure peaked at **1,048,575 bytes** against 1 MiB. Maximum descriptor residency
was **336,578 bytes** against 16 MiB; codec workspace **1,953,079 bytes** against
64 MiB. Peak mask residency was 2,091,750 bytes normally and 589,824 in pressure.
Largest observed WASM linear memory was 9,043,968 bytes. Output reservations
retained the 4 MiB ceiling. Sampled summed process RSS peaked at 1,659,211,776
bytes; this includes service/runner and shared pages, can miss peaks, and is not
cache or RSS acceptance. No GPU resource limit was exercised.

Protocol HEAD: `3d0565f66003605a86d2a118c1d683864649ff64`, containing correction
`8301bc9` and canonical wiring `084b1a3`. Committed/local SHA-256 pins were:

| Protocol file | SHA-256 |
| --- | --- |
| tools/viewer-acceptance-browser-masks.mjs | `eee41f139e61ef90f67eebf0256fca06a8c13f64d2b1313469c5fd5b068afdef` |
| tools/tests/viewer-acceptance-browser-masks.test.mjs | `9d808d8150216c73af96c6544bfef89e8c7e73240e396e16023601fe19c929a0` |
| docs/viewer-acceptance-browser-masks.md | `1bc779d8868793a2a2a6f218fa92aba5010faccbd7c351b66a7ddbbf702b3adb` |
| docs/viewer-acceptance-browser-environment.md | `b0351725457347e2b8ae50b16dc30c6a705732744aa1f296fb8a58397ceb6bb1` |

Runtime remained `fe82f85984fba5c75d7a5a8f10b2d914b773a838`, using the frozen
`native-frozen-v1/web` assets. Service remained the distinct
`4a1594b0840eadae26a4d6005d50d9201734a1a7` binary and five pinned representations.
Rights, native references, static assets, service/build/catalogue, Node,
Chromium 1234, Playwright 1.62.1 and 71 startup-library pins were checked before
execution. Observed Chromium version was **151.0.7922.34**. Final immutable-input
rechecks passed; startup library enumeration does not cover later `dlopen`.

All evidence resides beneath the approved
`/nvme/development/emuella/emuella-testdata/artifacts/rareplanes-expanded-v1`:
output `viewer-acceptance-browser-masks-private-tmp-02` and its `-execution`
group. The capsule SHA-256 is
`6ecfcd49231c0b5ed12ada056a35f8a70ebba1eb4fd80f92b48c9af02d3d1403`;
grant `f0204423c41474fcef78ee2a5f22e279ddab7a9ad4de120460e2c5b41db720ba`;
canonical plan `45db6fd835feafebcb88aefc410a248943a32a2caf5893288c30f592b6d5def4`;
result `d9415622e431c256b7d5528ab1378ee580c2b37aea7f40bf5d4028bc629a8e58`.

The private `unshare -Urm` invocation retained networking. `/tmp` and approved
backing both had device **60**, inode **287838754**; effective mount IDs were
**685/669**. Namespace changed from `mnt:[4026531832]` to
`mnt:[4026534680]`. Mapping passed before Playwright import and all nine launches.
Host `/tmp` identity and parent namespace remained unchanged.

A preceding metadata-preparation error is retained in
`viewer-acceptance-browser-masks-private-tmp-01-execution`: incorrect JavaScript
string escaping stopped validation before any proof invocation or browser
launch. Its owned service stopped; a fresh group held the corrected capsule and
grant. There were two owned service starts across preparation/execution, both
stopped with SIGTERM (exit -15), and exactly one browser proof invocation.
The wrapper exited 0 after recording the runner's exit 2. No owned live process
remained; individual successful Chromium exit codes were not captured.

The original nine failures and failed long-path diagnosis remain immutable;
38 historical evidence pins were rechecked. Profiles, logs, temporary backing and
detailed outcomes are retained; the companion JSON links their file manifest.
This operation wrote only the two corrected-results reports in the repository;
no commits or pushes. Evidence/count/identity and whitespace checks replace full verification
here because browser smoke would exceed the authorised invocation.

Hardware UI viewing, GPU timing, normal latency and seven-event recovery remain
separate and unmeasured. OS cache state was uncontrolled. Same-realm diagnostic
intervals are retained without a speed claim. No human, ML, configuration or
product acceptance is asserted.
