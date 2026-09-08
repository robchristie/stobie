# Independently authored NITF proof contract

This contract reproduces the public, independently authored NITF proof for the
Emuella viewer. It is a measurement and support contract, not an original-vendor
or real-scene visual qualification. The accepted source is the arithmetic,
Apache-2.0 `independent-nitf-v1` fixture: no external pixels, Kakadu, Geometis,
protected imagery, or derivative input is permitted.

The selected 43,008 by 43,008 U11 mono NITF has SHA-256
`dcf61436814d33c75c876b173f2dc87fcbd61571be7e766489dd884ab9891c07`.
Its [creation record](emuella-viewer-evidence/nitf-calibration/source-43008-provenance.json)
identifies `NITF02.10`, `IC=C8`, `ABPP=11`, UInt16 storage and 11 meaningful
unsigned bits. First profiles are native U11 mono, U16 grey, and RGB8/RGB16
small fixtures; the large frozen profile is U11 mono only.

## Identity and separation of roles

Generation uses an independently built GDAL with only GTiff, NITF and
JP2OpenJPEG registered; it produces and separately OpenJPEG-decodes the raw
codestream. It must not load JP2Emuella. Ingestion uses a separate maintained
GDAL and the JP2Emuella plugin, with source indexing required. The recorded
owner revisions are codec `2568f1c40c83a40f527c7ee8f1600af511e046d0`, plugin
`f5e21e5b9bdd9ffab95f19299e2ceb81a18fee84`, maintained GDAL
`1af54d99959f3b62ba10451a357a969075374663`, and testdata
`2d519ddaf019f10b9e409ea3338d395438486647`; hashes and build-record bindings
are retained in [build-attestation.json](emuella-viewer-evidence/nitf-calibration/build-attestation.json).

Use a new registered scratch child for every generation and measurement. Set
`TESTDATA_REPO`, `GENERATION_GDAL_LIBRARY`, `GENERATION_LD_LIBRARY_PATH`,
`INGESTION_GDAL_LIBRARY`, `INGESTION_LD_LIBRARY_PATH`, `JP2EMUELLA_PLUGIN`,
`CODEC_LIBRARY`, `VIEWER_TOOL`, `VIEWER_REVISION`, and the four `*_BUILD_RECORD`
variables to caller-supplied artefacts:

```sh
export SCRATCH="$CAMPAIGN_SCRATCH/nitf-proof"
export CODEC_REVISION=2568f1c40c83a40f527c7ee8f1600af511e046d0
```
The record, rather than a filename, establishes each binary's source revision and digest.

## Generate and verify the source

```sh
LD_LIBRARY_PATH="$GENERATION_LD_LIBRARY_PATH" GDAL_DRIVER_PATH= \
python3 "$TESTDATA_REPO/recipes/independent-nitf-v1.py" \
  --gdal-library "$GENERATION_GDAL_LIBRARY" \
  --gdal-source-revision 1af54d99959f3b62ba10451a357a969075374663 \
  --case pan11-lossless --width 43008 --height 43008 \
  --output "$SCRATCH/source-43008" --work "$SCRATCH/source-43008-work"

python3 "$TESTDATA_REPO/recipes/check-independent-nitf.py"
(cd "$TESTDATA_REPO" && cargo run -p emuella-corpus -- verify jpeg-2000/independent-nitf)
sha256sum "$SCRATCH/source-43008/pan11-lossless.ntf"
```
The generation recipe calculates every source value from coordinates, validates
the NITF and codestream precision, and records a separate complete OpenJPEG
decode. Its arithmetic pattern compresses more effectively than the selected HT
target; encoded-size results do not support a WorldView, vendor-product, or
general storage-benefit claim.

## Prepare and measure independently

The wrapper requires a retained source index, rejects header/index budget overflow,
and keeps preparation bounded. It invokes the separately built Emuella ingestion
stack; it is not a generation decoder. Use a fresh root and retain its report,
tool result, representation, stderr, build records and observer trace files.

```sh
export NITF_INPUT="$SCRATCH/source-43008/pan11-lossless.ntf"
export OBSERVER_OUTPUT="$SCRATCH/prepare-43008-cold-observer"
export PERFORMANCE_OUTPUT="$SCRATCH/prepare-43008-warm-performance"
export OBSERVER_LIMITS="apps/emuella-viewer/qualification/nitf-observer-thresholds.json"
export PERFORMANCE_LIMITS="apps/emuella-viewer/qualification/nitf-performance-thresholds.json"

LD_LIBRARY_PATH="$INGESTION_LD_LIBRARY_PATH" \
python3 tools/viewer-prepare-nitf.py \
  --tool "$VIEWER_TOOL" --tool-revision "$VIEWER_REVISION" \
  --gdal-library "$INGESTION_GDAL_LIBRARY" --gdal-revision 1af54d99959f3b62ba10451a357a969075374663 \
  --plugin "$JP2EMUELLA_PLUGIN" --plugin-revision f5e21e5b9bdd9ffab95f19299e2ceb81a18fee84 \
  --codec-library "$CODEC_LIBRARY" --codec-revision "$CODEC_REVISION" \
  --build-record "$VIEWER_BUILD_RECORD" --build-record "$GDAL_BUILD_RECORD" \
  --build-record "$PLUGIN_BUILD_RECORD" --build-record "$CODEC_BUILD_RECORD" \
  --input "$NITF_INPUT" --output "$OBSERVER_OUTPUT" --target scientific-u11 \
  --width 43008 --height 43008 --bits 11 --bands 1 --tile 512 --levels 6 --bpp 2 \
  --measurement-mode observer --source-cache-state cold-os --limits "$OBSERVER_LIMITS"

LD_LIBRARY_PATH="$INGESTION_LD_LIBRARY_PATH" \
python3 tools/viewer-prepare-nitf.py \
  --tool "$VIEWER_TOOL" --tool-revision "$VIEWER_REVISION" \
  --gdal-library "$INGESTION_GDAL_LIBRARY" --gdal-revision 1af54d99959f3b62ba10451a357a969075374663 \
  --plugin "$JP2EMUELLA_PLUGIN" --plugin-revision f5e21e5b9bdd9ffab95f19299e2ceb81a18fee84 \
  --codec-library "$CODEC_LIBRARY" --codec-revision "$CODEC_REVISION" \
  --build-record "$VIEWER_BUILD_RECORD" --build-record "$GDAL_BUILD_RECORD" \
  --build-record "$PLUGIN_BUILD_RECORD" --build-record "$CODEC_BUILD_RECORD" \
  --input "$NITF_INPUT" --output "$PERFORMANCE_OUTPUT" --target scientific-u11 \
  --width 43008 --height 43008 --bits 11 --bands 1 --tile 512 --levels 6 --bpp 2 \
  --measurement-mode performance --source-cache-state warm-os --limits "$PERFORMANCE_LIMITS"
```

[nitf-observer-thresholds.json](../apps/emuella-viewer/qualification/nitf-observer-thresholds.json)
freezes the cold observer gate; [nitf-performance-thresholds.json](../apps/emuella-viewer/qualification/nitf-performance-thresholds.json)
freezes the warm uninstrumented gate. They extrapolate the 32,768 warm baseline,
include a 25% allowance, and use 8,192 merged-owner cold/warm confirmations for
payload and bounded-resource behaviour. They are prequalification predictions,
not 43,008 results. The retained calibration manifest links the exact baselines
and merged confirmations. A source hash scan warms decoder input and stays in
the measured preparation boundary; observer timing includes `strace` overhead.
Observer syscall totals include that scan and rereads, and describe syscalls,
not physical-device or NFS traffic.

Both full-size preparation modes and the native/browser/recovery journeys pass
their unchanged freezes. See the [qualification record](emuella-viewer-qualification.md)
for exact builds, input identities, observed limits, failed environment attempts
and the remaining real-scene boundary.

## Serve and exercise the viewer

After a successful preparation, build a same-origin static root and serve the
single prepared NITF representation on loopback for interactive inspection:

```sh
export VIEWER_WEB_ROOT="$SCRATCH/viewer-web"
POLYORAMA_VIEWER_WEB_DIR="$VIEWER_WEB_ROOT" cargo xtask build-viewer-web

# Terminal 1: leave this service running.
cargo run --release -p emuella-viewer-tools -- serve \
  --representation "$OBSERVER_OUTPUT/representation" \
  --listen 127.0.0.1:8123 --verify-payload true --web "$VIEWER_WEB_ROOT"

# Terminal 2:
cargo run --release -p emuella-viewer -- \
  --server http://127.0.0.1:8123
```

Open `http://127.0.0.1:8123` in a WebGPU-capable browser. A one-parent catalogue
supports interactive inspection but cannot reproduce the multiple-image
comparison/bookmark workload or its scripted journey. For the frozen composed
nine-parent journey, with the
NITF parent first and eight public auxiliary representations, follow the
[calibration run and evidence commands](emuella-viewer-calibration.md#run-and-retain-evidence).
That route records the requested GPU path and observed adapter; software or an
unavailable adapter proves no physical-GPU performance. Source-index
construction/open cost and header caps belong to preparation; a reopened
representation does not reopen the source. Original-vendor and real-scene visual
qualification remain deliberately deferred.
