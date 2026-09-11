# Repaired fixed native cohort results

Exactly **five fresh native starts** completed **40 phases, ten release/revisit
cycles and 102 allocation markers each**: 200 phases, 50 cycles, 510 markers.
All applications and harnesses exited **0**, with no trace or application errors;
all ten scheduled captures exited **0**. Cohort exit **4** remains: runs 2/5 fail
target-detail latency and run 3 lacks the final external current-RSS after-bracket.
No warmups, retries, replacements or extra captures occurred.

Protocol `6f1965043f6317e8655c5dce4c2bc8a27775c5d4`; runtime
`8633754fa28f2ca34f159a7368e0b8e7e953d205`; native SHA-256
`3624000ce2d78ff1b646db572102df1a028800278d6fdb661fde43d36a10f0a6`; web-manifest SHA-256
`63cf2e1529b16e715edaf63747a901466e67f67318062968c76672d909e1da5d`. The unchanged runner explicitly selected
`--protocol docs/viewer-acceptance-native-repaired-cohort.json`.
A clean detached worktree at registered `repaired-protocol-6f19650` was privately
bound read-only at the protocol's literal repository root. The seven committed
protocol-file identities and all frozen input identities passed source-only checks.

The browser invocation/service/namespace had stopped before native setup. This
cohort reused the prior successful bwrap/Xvfb/xkbcomp infrastructure and existing
benchmark CLI. Read-only display allocation found `:178` occupied before any
native start; exclusive **:179, 1440×900×24** and port **8197** were then granted.
No existing display was stopped. The fresh service used the two original Mansfield
representations and separately identified old source
`4a1594b0840eadae26a4d6005d50d9201734a1a7`, binary
`7ea60902498fbbfcac67f7d7785e42a38b4ce06741895ad916f8996c0f137102`.
Run 1 is `uncontrolled-first-observation`; runs 2–5 are `warm-server`, with actual
service counters retained. This does not assert fully warm OS/server caches.

Every application reported **NVIDIA GeForce RTX 3090 / NVIDIA 610.43.03 / Vulkan**,
PCI `0000:01:00.0`. Each run hashed **42 observed mapped files**, matching eight
frozen graphics identities; three NVIDIA device nodes were unhashable. glibc
`mallinfo2` **2.43** was available at all markers. Geometry, library, binary,
service, environment, source and output identities are bound in the JSON and
approved evidence. Mapping coverage is sampled, not a complete loading history.

All following memory values are absolute bytes; the unchanged ceiling is
**243,269,632**. All 15 RSS/HWM observations below pass that ceiling.

| Run | App HWM | Sampled descendant RSS peak | Observed per-PID HWM sum | Benchmark exit |
| --- | --- | --- | --- | --- |
| native-01 | 190,349,312 | 195,854,336 | 201,568,256 | 0 |
| native-02 | 191,279,104 | 193,056,768 | 203,255,808 | 4 |
| native-03 | 191,127,552 | 192,897,024 | 203,710,464 | 0 |
| native-04 | 189,775,872 | 191,672,320 | 201,482,240 | 0 |
| native-05 | 190,029,824 | 191,811,584 | 200,138,752 | 4 |

All five traces were admitted. Runs 1/3/4 qualified mechanically; runs 2/5 did not:
**168.964275 / 170.736860 ms > 133.58994625000037 ms** target-detail maximum.
These are observed failed predicates. Missing external/visual evidence is separate.

| Run | Graphics current | Overview HWM | Cycle 1 current | Cycle 10 current | Marker growth |
| --- | --- | --- | --- | --- | --- |
| native-01 | 161,132,544 | 175,697,920 | 189,534,208 | 190,349,312 | +815,104 |
| native-02 | 162,840,576 | 177,418,240 | 190,496,768 | 191,279,104 | +782,336 |
| native-03 | 161,976,320 | 176,463,872 | 190,357,504 | 191,127,552 | +770,048 |
| native-04 | 161,087,488 | 176,037,888 | 189,001,728 | 189,775,872 | +774,144 |
| native-05 | 161,566,720 | 176,230,400 | 189,239,296 | 190,029,824 | +790,528 |

Startup current RSS was **4,890,624 / 4,751,360 / 4,808,704 / 4,714,496 /
4,763,648** bytes. Direct marker growth supplements the declared external brackets.
External after-bracket growth was **+7,966,720 / −9,084,928 / unavailable /
−26,578,944 / −11,673,600** bytes. Run 3 cycle 10 has no same-identity read wholly
after its marker within 1,500 ms. Other runs' final brackets can share samples
across cycles and include export/teardown; they do not establish a settled plateau.
All phase HWM/current observations and detailed brackets stay in the approved store.

Each run ended with active request working set **0**, peak **217,363 bytes**;
pin metadata **0**, peak **5,034 bytes**. Compressed residency/peak was
**2,566,649** against **67,108,864 bytes**, including **606,740 mask bytes**.
Mask evictions and compressed-bin/representation evictions were **0**; identical-key
mask refetch was not established. There were no application pressure errors.
The separate **1,048,576-byte browser pressure context was unperformed**.

At all ten release markers per run, runtime entries, outstanding reservations,
worker-reserved bytes, payloads, upload/sample/validity capacity were zero;
revisit restored 32 resident entries with no outstanding payload ownership.
The maxima sampled at allocation markers were also zero for reservations/payload
capacities; these settled-boundary observations do not measure transient maxima.
Cumulative display release was **416 items / 27,288,064 logical bytes** per run.
Retained stages grew **30→39**; event/trace rings retained **128/256** slots, with
trace drops **2,170 / 2,173 / 2,169 / 2,168 / 2,173**. Detailed allocator and smaps
observations remain retained. RSS, smaps, allocator and logical resources overlap;
no additive model, physical-GPU attribution or historical RSS-repair claim.

| Run | First useful | Overview | Detail | Thumbnails | GPU revisit | Compressed revisit |
| --- | --- | --- | --- | --- | --- | --- |
| native-01 | 77.150888 | 703.645848 | 127.770930 | 564.731479 | 48.246721 | 638.273246 |
| native-02 | 67.503782 | 750.366396 | 168.964275 | 578.244099 | 49.466353 | 641.564885 |
| native-03 | 73.882151 | 702.679179 | 129.761234 | 563.108082 | 57.614535 | 629.402493 |
| native-04 | 77.709322 | 704.825229 | 110.682140 | 568.002338 | 49.939703 | 625.950667 |
| native-05 | 76.202279 | 734.199456 | 170.736860 | 551.643719 | 43.890868 | 647.470984 |

Latencies are milliseconds, including capture, sampler and diagnostic overhead;
none is subtracted. Phase/native deadlines stayed **60/660 seconds**, captures
**2/5 seconds** with their unchanged bounds. The user excluded other measurements
and builds; source preflight retains the process snapshot. This owner launched no
builds or competitors. Ordinary system activity was not continuously isolated.

All detailed evidence and screenshots remain beneath:

`/nvme/development/emuella/emuella-testdata/artifacts/rareplanes-expanded-v1/viewer-acceptance-native-cohort-mansfield-repaired-01`

| Run | Screenshot paths relative to that group |
| --- | --- |
| native-01 | `native-01-captures/capture-02s.png`, `native-01-captures/capture-05s.png` |
| native-02 | `native-02-captures/capture-02s.png`, `native-02-captures/capture-05s.png` |
| native-03 | `native-03-captures/capture-02s.png`, `native-03-captures/capture-05s.png` |
| native-04 | `native-04-captures/capture-02s.png`, `native-04-captures/capture-05s.png` |
| native-05 | `native-05-captures/capture-02s.png`, `native-05-captures/capture-05s.png` |

All PNGs are 1440×900; durations **123.628–244.731 ms**, lateness
**1.043–12.512 ms**. The [JSON report](viewer-acceptance-native-repaired-cohort-results.json)
binds full paths, exact hashes/timestamps, every run's trace/summary and evidence
manifest SHA-256 `a79b01e77025a9d7a5ebf1083880f73d26890225fab9a270ae6f97884e899a2a`.
Main must inspect the images; this owner made no visual finding.

Before/after representation inventories match. Native/service/display identities
are absent, service exited **−15**, Xvfb **0**, port refuses connections and owned
display socket/lock are absent. All protected groups, logs, captures and detached
worktree remain. Only four authorised result files were written in the main repo;
no source changes, commits, pushes, deletion or downstream agents.

Historical **278,794,240 > 243,269,632 bytes**, old 40 pressure failures, launch
failures, quality and scheduling rejections remain unchanged. Browser exact-mask
proof, main image inspection, separate seven-event recovery and final merged
qualification remain missing. No accepted configuration, human/ML or production-speed
claim. Canonical verification, independent review and landing remain main-owned.
