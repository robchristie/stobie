# Compact exact source validity

This representation change preserves original source validity, image bytes and
all historical quality exclusions. It is scoped mask qualification, independent
of complete viewer acceptance. The browser protocol retains the unchanged
[47-window/two-pass pressure journey](representation-efficiency-browser-masks.md).

## Version and immutable identity

Legacy `source-validity-v1` manifests and raw bitmap sidecars remain readable;
their serialised identities and public `ValidityIdentity` struct literals are
unchanged. New CLI preparation uses `source-validity-v2`. The source SHA-256,
selected original bands in component order, source geometry, policy/version and
complete level/tile catalogue remain inside the authenticated representation ID.
Missing required catalogue entries or bodies fail before decoded publication.

V2 entries in the existing level-major `tile_sha256` catalogue have exactly the
ASCII grammar `STATE:SHA256`: STATE is `0`, `1` or `2`; SHA256 is 64 lowercase
hexadecimal digits. There are no optional spaces, alternate tags or inferred
states. The sidecar's first byte must equal the declared numeric state:

| State | Meaning | Exact sidecar length |
| --- | --- | --- |
| 0 | Every component's ALL and ANY cells are invalid | 1 byte |
| 1 | Every component's ALL and ANY cells are valid | 1 byte |
| 2 | Component-major canonical legacy bitmap follows | 1 + legacy length |

The SHA-256 covers the state byte and complete body. Both manifest catalogue and
payload therefore bind the state. State 2 retains legacy LSB-first planes,
clipped geometry, zero padding and ALL-implies-ANY validation. Unknown tags,
noncanonical tags, wrong state, wrong lengths, corrupt digests, stale identity,
padding and invalid ALL/ANY relationships fail closed.

The deliberately small scope is an entire tile/level across all components.
A mixed tile retains bitmap planes, including entirely valid individual bands.
Its one-byte overhead is counted. There is no per-plane selector framework,
codec change or altered source-mask policy. Band order, globally anchored
reductions, ceil regional endpoints and combined display AND remain unchanged.

## Memory and storage

Uniform preparation scans the bounded native GDAL tile and emits one-byte
states without allocating packed mask planes. The service authenticates exact
length before reading. Cache and reservation admission retain/count the encoded
bytes; constants never expand into full-tile masks in these paths. Regional
combination reads the constant directly and creates only the existing requested
one-byte-per-output-pixel combined validity array. Existing decoded and GPU
budgets include that regional array and renderer packing as before.

The additional retained catalogue string capacities above the legacy 64-byte
hashes count against the existing descriptor budget and are exposed as
`compact_catalogue_metadata_bytes`. Manifest bytes and serialised two-byte tags
are reported separately. No limit is increased. A request rejection reports
image-bin bytes, mask bytes, their sum and the unchanged compressed limit.
The mask cache precharges one dense boxed slot array per authenticated tile/level
catalogue before allocating or publishing the representation. Its exact logical
metadata charge is `slots × size_of::<MaskSlot>() + size_of::<MaskCache>()`,
including the container header, occupancy counter, implicit key position, enum
state and bitmap pointer/length. Slot count and byte arithmetic are checked and
admitted against the existing descriptor limit. Unmasked legacy representations
allocate no slots. Constants occupy inline enum variants and return static
encoded bytes; mixed and legacy bitmaps use exact-length boxed slices. The
one-byte compressed charge for a constant conservatively overlaps its inline
state storage. No per-mask map node, key or growable payload capacity remains.

`mask_cache_metadata_bytes`, `peak_mask_cache_metadata_bytes`,
`mask_cache_entries`, `mask_cache_slots`, `mask_cache_slot_bytes` and
`mask_cache_container_bytes` expose the current/peak charge and its exact
composition on each native/WASM target. Payload eviction releases bitmap bytes
and occupancy but retains the precharged empty slot. Refetch does not grow
metadata; representation eviction releases the entire slot array and container
charge. Descriptor admission includes these fixed floors even when reclaiming
other descriptor caches. An impossible registration fails before eviction or
slot allocation. Existing request/pin metadata metrics expose active, peak and
terminal release.
These logical allocations do not claim process RSS or allocator overhead proof.

`emuella-viewer-tools compact-validity --input LEGACY --output FRESH` writes a
fresh derivative, authenticates legacy masks, and copies image payload and
descriptors without changing any bytes. It retains incomplete output on error;
it never modifies/deletes inputs. Its conversion receipt reports every mask's
state/hash/length, payload, descriptors, manifest, total representation bytes and
new metadata separately. Protected callers must keep derivatives and receipts
in their already authorised store with notice and lineage.

## Frozen real-scene method

`tools/representation-efficiency-masks.py` requires a clean exact committed
candidate and explicit converter/probe paths. It creates five fresh attributed
`representation-efficiency-masks-<scene>-01` groups and a native evidence group
inside the reviewed RarePlanes store, preserving all original groups. It hashes
original source files before and after, validates the current source-coverage
receipt and rehashes each inherited mask/native/request/agreement receipt.

All 1,883 sidecars are converted. An independent bounded NumPy comparator proves
legacy/new bit equality and recomputes each original per-band ALL/ANY digest
against the retained GDAL oracle receipt. This is **transitive original-source
oracle agreement**, not a new GDAL read or quality evaluation. The comparator's
bounded tile-array working bytes are reported separately from client residency.

The native probe compares the first and last retained region for every level
and component selection, with original request geometry and precision, against
retained original native samples/validity. It uses the actual service/cache and
scoped request lifetime, and records byte metrics plus reservation/pin release.
The separate actual browser protocol covers the complete inherited regional
matrix, delivery faults, cancellation/recovery and unchanged pressure journey.
Native GPU proof uses the unchanged renderer's explicit binary-validity readback
journey; exact compact-to-regional-array agreement connects it to this format.
There is no image-quality, timing, independent-codec or RSS acceptance claim.

Authored tests cover all seven levels; uniform valid/invalid and mixed tiles;
reordered original bands; clipped/global/cross-tile regions; preparation and
conversion equivalence; missing/corrupt/stale errors; cancellation/failure release,
reservation recovery, eviction/refetch and metadata admission. Canonical
verification owns the Python and browser invariants as well as Rust tests.
Results are recorded separately after execution on the committed candidate.
