# Frozen scheduling screen results

**Reject.** Exactly one screen completed: conditioning A, then AB BA AB BA AB; **11 fresh starts**, no warmups, retries, captures or replacements. Screen exit **4**. Every app and harness exited **0** and completed **29 actions / 30 phases**. The frozen harness records nonzero app exit as a trace failure; all traces completed without failures.

Protocol `5bcb8cce766625b41b296b2e82f3627addf93fe1` ran from clean detached `/nvme/development/emuella/.build-targets/viewer-acceptance/scheduling-screen-protocol-5bcb8cc`. Source `de742e4a9c17dd4cf082b38c40a1f681785cc1aa`; immutable binary SHA-256 `58bf7ab338c8242132e035a645dd15ae66c81f15519dd8db675c3fc5171dfa35`. Source-only preflight passed before app creation. Actual run identities prove the committed harness; loaded library hashes and RTX 3090 / NVIDIA 610.43.03 / Vulkan match the freeze. Exclusive Xvfb :178 was 1440×900×24; service port 8195, unchanged verified Mansfield PAN4/RGB12 catalogue. No builds or other timing launches were made.

| Slot | App / harness / benchmark exits | App HWM | Sampled RSS peak | Observed PID HWM sum |
| --- | --- | ---: | ---: | ---: |
| conditioning-A | 0 / 0 / 4 | 149,417,984 | 151,478,272 | 157,773,824 |
| pair-01-A | 0 / 0 / 0 | 146,034,688 | 148,123,648 | 153,997,312 |
| pair-01-B | 0 / 0 / 0 | 149,614,592 | 151,703,552 | 156,913,664 |
| pair-02-B | 0 / 0 / 0 | 149,757,952 | 151,814,144 | 156,946,432 |
| pair-02-A | 0 / 0 / 0 | 146,739,200 | 147,628,032 | 153,600,000 |
| pair-03-A | 0 / 0 / 0 | 149,696,512 | 151,781,376 | 157,274,112 |
| pair-03-B | 0 / 0 / 0 | 149,839,872 | 158,441,472 | 158,441,472 |
| pair-04-B | 0 / 0 / 0 | 149,024,768 | 151,126,016 | 157,396,992 |
| pair-04-A | 0 / 0 / 0 | 149,209,088 | 151,302,144 | 157,118,464 |
| pair-05-A | 0 / 0 / 0 | 149,663,744 | 151,764,992 | 157,470,720 |
| pair-05-B | 0 / 0 / 0 | 149,643,264 | 158,031,872 | 158,031,872 |

**Absolute failure:** conditioning target detail **133.652245 ms > 133.58994625 ms**. All eleven traces were admitted; conditioning was unqualified, ten measured cells qualified. Conditioning is excluded from paired statistics, but the runner explicitly requires every absolute gate across all eleven. Its decision therefore short-circuited to reject with no ratios. The following supplementary arithmetic includes every measured cell and applies the unchanged predicates.

| Metric (ms) | A mean | B mean | Mean paired (B−A)/A |
| --- | ---: | ---: | ---: |
| first_useful_ms | 86.789559 | 89.557896 | +3.9562% |
| whole_overview_ms | 784.150503 | 518.321565 | -33.9156% |
| target_detail_ms | 102.808695 | 108.699376 | +8.7952% |
| visible_thumbnail_completion_ms | 557.075097 | 344.275943 | -38.1973% |
| warm_revisit_ms | 49.812831 | 53.688948 | +7.6416% |
| warm_compressed_ms | 645.494797 | 395.492571 | -38.7620% |

Warm-compressed exceeds the descriptive 5% improvement gate. **22 non-regression fields fail +5%**, including detail, warm revisit and phase first-useful/complete-visible observations. Full paired values, arm means, ratios and failed predicates are in [the JSON report](viewer-acceptance-scheduling-results.json). These are descriptive ratios, not 99% confirmation bounds; no successful subset or outlier exclusion.

| Global metric | Maximum across all eleven | Unchanged ceiling |
| --- | ---: | ---: |
| process_peak_rss_bytes | 149,839,872 | 243,269,632 |
| decoded_peak_bytes | 1,572,864 | 16,777,216 |
| gpu_peak_bytes | 17,026,200 | 67,108,864 |
| peak_compressed_bytes | 2,566,649 | 67,108,864 |
| peak_descriptor_bytes | 1,083,232 | 16,777,216 |
| peak_codec_workspace_bytes | 6,489,012 | 67,108,864 |
| worker_concurrency | 1 | 1 |
| original_storage_read_bytes | 0 | 0 |

All three RSS observations in every cell stay below **243,269,632 bytes**. Historical **278,794,240 bytes** remains a failed observation. Worker concurrency measures actual outstanding reservations: **one**, not inferred execution-thread utilisation. Retained diagnostic maxima show **1,572,864 reserved bytes**, **786,432 upload bytes**, and the JSON preserves decoded accounting. Mask peak is **606,740 bytes**; these overlapping logical counts are not additional RSS. One execution worker and unchanged 4 MiB upload scratch are frozen source limits.

Every B run observed maximum pump batch **4** (cap 64). B maximum turn durations were **4.097565, 4.098004, 4.129146, 4.350216, 4.114705 ms**; maximum receive duration **4.331175 ms**. A pump maxima were zero. The four-millisecond requested budget is not a hard real-time ceiling; these maxima do not prove per-receive deadline admission or isolate kernel wait time. No cancellations or stale results were observed.

Retained shared-clock publication-to-receipt means were **17.321799 ms A / 11.433126 ms B**, diagnostic event-weighted values. No negative pacing differences or adjacent trace inversions were observed in retained records; values remain unclipped in approved-store evidence. Maximum event overwrites were 128 per run; trace overwrites reached 510 for A (including conditioning) and 461 for B. No cross-clock subtraction or complete causal coverage is claimed.

No separate mandatory seven-event recovery evidence, Tok regression or five-start/ten-cycle resource cohort was generated. Historical Tok failures and source-quality rejection remain. No production speed or acceptance claim; **no twenty-pair confirmation, candidate retuning or second candidate**. Main owns production candidate removal.

Evidence group: `viewer-acceptance-scheduling-screen-mansfield-01`. `execution.json` SHA-256 `8bada17acce28a78e05b7a6ebee9eaef6a8c0fc063e5707474884a5b45986678`; `evidence-manifest.json` SHA-256 `fa0866e412aa5b88c8512b72f995ec32c32c67ec4ea151861725a49bfe183d5f`. The manifest binds every retained screen file except itself; the separate attributed infrastructure group binds preflight, grant, commands, machine and cleanup evidence. Input inventories match before/after.

Service and outer bwrap exited -15. Xvfb child was briefly alive at immediate lifecycle check, then absent before follow-up; no additional signal was sent. Its independent exit status is unavailable. Stale socket/lock retained under no-deletion instruction; both endpoints refuse connections.

All owned native processes are absent. No files were deleted, no commits/pushes or downstream agents used, and concurrent browser-code edits were preserved. Only the two bounded results files were written in the main repository. Report derivations, source identities, JSON and whitespace were checked; full verification would exceed the authorised launch/build budget.
