# Native diagnostic image inspection

The coordinator opened all ten unmodified 1440×900 captures from the five-start
[resource cohort](viewer-acceptance-native-cohort-results.md), at the declared
2-second and 5-second capture points. Their exact identities and actual capture
times are retained by that cohort's evidence manifest. No new application start,
image transformation or display retuning was performed for this inspection.

The native interface renders its controls, primary pane, linked overview or
comparison pane, and detection gallery. Mansfield PAN16 is visible in all five
2-second captures; the first 5-second capture shows RGB16 in both viewing panes.
Road/runway geometry and buildings can be discerned, but the scene and thumbnails
are very dark at the recorded display gamma (1.00 at 2 seconds; 1.40 at 5 seconds).
This is a visible limitation, not an accepted display-quality result. The frozen
numerical U8-display comparison is a separate measurement boundary; these
screenshots do not establish equivalent display stretching or its quality gates.

The fixed wall-clock captures intersect different workload transitions. At
5 seconds, runs 3–5 show partially populated or coarse comparison/detection
content, with the interface reporting respectively 0, 3 and 1 regions ready.
These are observed intermediate frames, not proof of settled-view failure or
successful complete-visible presentation. Phase receipts and benchmark
admissions retain their own definitions; no screenshot is relabelled a settled
phase. Run 1's measured target-detail failure remains.

This is agent inspection of the actual native hardware-rendered application on
the recorded RTX 3090/Vulkan configuration. It is not human acceptance, calibrated
monitor/scanout measurement, source-array fidelity, analytical/ML suitability,
Boca/Tok inspection, browser inspection or an accepted viewing configuration.
Mansfield source masks contain no invalid transitions; these captures cannot
prove visible behaviour at Boca's valid/invalid boundaries.

## Repaired runtime cohort

The coordinator also opened all ten original captures from
[the repaired five-start cohort](viewer-acceptance-native-repaired-cohort-results.md),
using its recorded 2-second and 5-second files. No additional application start
or image transformation was performed. At 2 seconds all five show the Mansfield
PAN16 primary and linked overview, at gamma 1.00; the displayed ready counts are
7, 24, 7, 7 and 24. Roads, runway and buildings remain discernible but very dark.
At 5 seconds the primary is PAN16 and the comparison is RGB16 at gamma 1.40.
The ready counts are 4, 24, 4, 5 and 2. Runs 1, 3 and 5 have visibly coarse
comparison pixels; run 5's primary is also coarse. Run 2 has populated gallery
thumbnails; the other 5-second captures have mostly unpopulated gallery cells.
These fixed-time images intersect workload transitions and do not establish
settlement, a new quality pass or a display-retuning result. The actual native
hardware/backend evidence belongs to the linked cohort. Its two measured detail
failures and the distinctions from human acceptance and analytical suitability
remain unchanged.
