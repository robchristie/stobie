use super::*;
use std::sync::OnceLock;

struct Fixture {
    manifest: Manifest,
    descriptors: Vec<Vec<u8>>,
    payload: Vec<u8>,
    masks: Vec<Vec<Vec<u8>>>,
    index: IndexedLossyHt,
}
fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let profile = Profile {
            width: 768,
            height: 256,
            tile_edge: 256,
            decomposition_levels: 2,
            bits_per_sample: 8,
            components: 1,
            bits_per_pixel: 2.5,
        };
        let mut payload = Vec::new();
        let mut descriptors = Vec::new();
        let summary = codec::ht_indexed::encode_tiled_to_descriptors(
            profile.codec(),
            |rect, planes| {
                for (i, v) in planes[0].iter_mut().enumerate() {
                    *v = ((i * 17 + i / 256 * 31 + rect.x as usize) % 256) as u8;
                }
                Ok(())
            },
            |bytes| {
                payload.extend_from_slice(bytes);
                Ok(())
            },
            |_, bytes| {
                descriptors.push(bytes.to_vec());
                Ok(())
            },
        )
        .unwrap();
        let masks: Vec<_> = (0..3)
            .map(|tile| {
                let native: Vec<_> = (0..256 * 256)
                    .map(|i| u8::from((i + tile * 256) % 19 != 0))
                    .collect();
                validity::encode_tile(256, 256, 2, &[native]).unwrap()
            })
            .collect();
        let mut manifest = Manifest {
            target: "authored-admission".into(),
            tid: String::new(),
            identity: Identity {
                validity: Some(validity::ValidityIdentity {
                    source_sha256: sha256(b"authored validity"),
                    bands: vec![1],
                    policy: validity::POLICY.into(),
                    tile_sha256: (0..3)
                        .map(|d| masks.iter().map(|m| sha256(&m[d])).collect())
                        .collect(),
                }),
                source_sha256: sha256(b"authored pixels"),
                bands: vec![1],
                profile,
                codec_revision: "authored".into(),
                encoding_contract: "authored".into(),
                spatial_policy_sha256: sha256(b"authored"),
                payload_sha256: sha256(&payload),
                descriptor_format: "EHTIDX01".into(),
            },
            encoded_bytes: summary.encoded_bytes,
            main_header_bytes: summary.main_header.end,
            descriptor_sha256: descriptors.iter().map(|b| sha256(b)).collect(),
        };
        manifest.seal().unwrap();
        manifest.validate().unwrap();
        let mut index = manifest.sparse().unwrap();
        for descriptor in &descriptors {
            index.import_tile_descriptor(descriptor).unwrap();
        }
        Fixture {
            manifest,
            descriptors,
            payload,
            masks,
            index,
        }
    })
}
fn region(x: u32) -> Region {
    Region {
        x,
        y: 17,
        width: 32,
        height: 32,
        discard: 0,
        components: vec![0],
    }
}
fn client(limit: usize) -> SharedClient {
    let f = fixture();
    let mut c = SharedClient::new(ClientLimits {
        compressed_bytes: limit,
        ..Default::default()
    });
    c.register(f.manifest.clone()).unwrap();
    for (tile, bytes) in f.descriptors.iter().enumerate() {
        c.install_descriptor(&f.manifest.tid, tile as u16, bytes)
            .unwrap();
    }
    c
}
fn required(r: &Region) -> usize {
    let f = fixture();
    let ranges = bin_ranges(&f.index).unwrap();
    demands(&f.index, r)
        .unwrap()
        .iter()
        .map(|d| {
            let r = &ranges[&d.key];
            (r.end - r.start) as usize
        })
        .sum::<usize>()
        + r.tiles(&f.manifest.identity.profile).unwrap().len() * 8192
}
fn reader(c: &mut SharedClient) -> ResponseReader {
    let tid = &fixture().manifest.tid;
    c.begin_response(
        tid,
        &jpip::ResponseFields::parse(tid, "768,256", "0,0", "768,256").unwrap(),
    )
    .unwrap()
}
fn message(key: jpip::BinKey, offset: u64, final_bin: bool, bytes: &[u8]) -> Vec<u8> {
    jpip::encode_message(jpip::DataMessage {
        key,
        offset,
        final_bin,
        auxiliary: None,
        bytes,
    })
    .unwrap()
}
fn receive(c: &mut SharedClient, wire: &[u8]) -> Result<()> {
    let mut r = reader(c);
    for part in wire.chunks(509) {
        c.receive(&mut r, part)?;
    }
    c.receive(&mut r, &jpip::encode_end(2))?;
    c.finish(r)
}
fn masks(c: &mut SharedClient, r: &Region) {
    let f = fixture();
    for tile in c.missing_masks(&f.manifest.tid, r).unwrap() {
        c.install_mask(
            &f.manifest.tid,
            tile,
            r.discard,
            &f.masks[tile as usize][r.discard as usize],
        )
        .unwrap();
        assert!(c.resident_bytes().0 <= c.limits.compressed_bytes);
    }
}
fn bins(c: &mut SharedClient, r: &Region) {
    let f = fixture();
    let ranges = bin_ranges(&f.index).unwrap();
    let initial = c.request(&f.manifest.tid, r, 1024).unwrap().model;
    let evictions = c.metrics.compressed_bin_evictions;
    let mask_evictions = c.metrics.mask_evictions;
    for d in demands(&f.index, r).unwrap() {
        if initial.get(&d.key) == Some(&jpip::Known::Complete) {
            continue;
        }
        let range = &ranges[&d.key];
        let bytes = &f.payload[range.start as usize..range.end as usize];
        // Separate responses, duplicate fragments and sparse arrival within a
        // lifetime exercise the real owner cache rather than a mocked byte count.
        let split = bytes.len() / 2;
        receive(c, &message(d.key, split as u64, true, &bytes[split..])).unwrap();
        receive(c, &message(d.key, 0, split == bytes.len(), &bytes[..split])).unwrap();
        receive(c, &message(d.key, 0, true, bytes)).unwrap();
        assert_eq!(c.metrics.compressed_bin_evictions, evictions);
        assert_eq!(c.metrics.mask_evictions, mask_evictions);
        assert!(c.resident_bytes().0 <= c.limits.compressed_bytes);
    }
}
#[test]
fn masked_multitile_scope_survives_pressure_then_releases_for_eviction_and_exact_refetch() {
    let f = fixture();
    let tid = &f.manifest.tid;
    let first = region(240);
    let second = region(496);
    let limit = required(&first).max(required(&second));
    assert!(
        limit < f.payload.len() + 3 * 8192,
        "whole representation need not fit"
    );
    let mut c = client(limit);
    // Both unrelated masks in the same representation and unrelated compressed
    // data would displace selected dependencies under the original policies.
    c.install_mask(tid, 2, 0, &f.masks[2][0]).unwrap();
    receive(
        &mut c,
        &message(
            jpip::BinKey::new(0, 90000).unwrap(),
            0,
            true,
            &vec![9; limit - 8192],
        ),
    )
    .unwrap();
    assert_eq!(c.resident_bytes().0, limit);
    let mut reference = client(1 << 20);
    masks(&mut reference, &first);
    bins(&mut reference, &first);
    let expected = reference.decode(tid, &first).unwrap();
    let before_received = c.metrics.received_jpp_bytes;
    for r in [&first, &second, &first] {
        let scope = c.begin_request(tid, r).unwrap();
        let reservation = c.request_reservation();
        assert_eq!(reservation.0, required(r));
        assert!(reservation.1 > 0 && reservation.1 < 4096);
        assert!(c.begin_request(tid, r).is_err());
        assert!(c.install_descriptor(tid, 0, &f.descriptors[0]).is_err());
        // Bins before masks also work; selected bins must survive mask insertion.
        if r == &second {
            masks(&mut c, r);
        }
        bins(&mut c, r);
        masks(&mut c, r);
        assert!(c.ready(tid, r).unwrap());
        let evictions = (c.metrics.mask_evictions, c.metrics.compressed_bin_evictions);
        bins(&mut c, r);
        assert_eq!(
            evictions,
            (c.metrics.mask_evictions, c.metrics.compressed_bin_evictions)
        );
        let output = c.decode(tid, r).unwrap();
        if r == &first {
            assert_eq!(output.planes, expected.planes);
            assert_eq!(output.validity, expected.validity);
        }
        c.end_request(scope);
        assert_eq!(c.request_reservation(), (0, 0));
        assert!(c.resident_bytes().1 <= c.limits.descriptor_bytes);
    }
    assert!(c.metrics.mask_evictions >= 3);
    assert!(c.metrics.compressed_bin_evictions > 1);
    assert!(c.metrics.received_jpp_bytes > before_received);
    assert_eq!(c.metrics.decode_count, 3);
    assert!(c.metrics.peak_compressed_bytes <= limit);
    assert!(c.metrics.peak_descriptor_bytes <= c.limits.descriptor_bytes);
    assert!(c.metrics.peak_codec_workspace_bytes <= c.limits.decode_workspace_bytes);
}
#[test]
fn oversize_combined_set_fails_before_transport_without_pins_or_evictions() {
    let r = region(240);
    let f = fixture();
    let required = required(&r);
    let mut c = client(required - 1);
    assert!(2 * 8192 < required - 1, "masks alone fit");
    let before = c.resident_bytes();
    let error = c
        .begin_request(&f.manifest.tid, &r)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains(&format!("need {required} bytes, limit {}", required - 1)),
        "{error}"
    );
    assert_eq!(c.request_reservation(), (0, 0));
    assert_eq!(c.resident_bytes(), before);
    assert_eq!(c.metrics.received_jpp_bytes, 0);
    assert_eq!(c.metrics.decode_count, 0);
    assert_eq!(
        c.metrics.mask_evictions + c.metrics.compressed_bin_evictions,
        0
    );
}
#[test]
fn errors_cancellation_and_stale_readers_cannot_retain_or_release_new_scopes() {
    let f = fixture();
    let tid = &f.manifest.tid;
    let r = region(240);
    let mut c = client(required(&r));
    let mut unscoped = reader(&mut c);
    let first = c.begin_request(tid, &r).unwrap();
    assert!(
        c.receive(&mut unscoped, &jpip::encode_end(2))
            .unwrap_err()
            .to_string()
            .contains("stale")
    );
    let mut stale = reader(&mut c);
    let mut wrong = f.masks[0][0].clone();
    wrong[0] ^= 1;
    assert!(
        c.install_mask(tid, 0, 0, &wrong)
            .unwrap_err()
            .to_string()
            .contains("digest")
    );
    assert!(
        c.decode(tid, &r)
            .unwrap_err()
            .to_string()
            .contains("required masks")
    );
    // Executors must end on error or cancellation; payload is reusable, pins are not.
    let main = jpip::BinKey::new(6, 0).unwrap();
    let partial = message(main, 3, false, &f.payload[3..7]);
    receive(&mut c, &partial).unwrap();
    assert_eq!(c.resident_bytes().0, 4);
    c.end_request(first);
    assert_eq!(
        c.resident_bytes().0,
        0,
        "cancelled sparse fragments are reclaimable"
    );
    assert_eq!(c.request_reservation(), (0, 0));
    let second = c.begin_request(tid, &r).unwrap();
    c.end_request(first);
    assert!(c.request_reservation().0 > 0);
    let before = c.metrics.received_jpp_bytes;
    assert!(
        c.receive(&mut stale, &jpip::encode_end(2))
            .unwrap_err()
            .to_string()
            .contains("stale")
    );
    assert_eq!(c.metrics.received_jpp_bytes, before);
    assert!(c.finish(stale).unwrap_err().to_string().contains("stale"));
    let bad = message(jpip::BinKey::new(6, 0).unwrap(), 0, true, &[0]);
    assert!(
        receive(&mut c, &bad)
            .unwrap_err()
            .to_string()
            .contains("authenticated demand lengths")
    );
    c.end_request(second);
    c.end_request(second);
    assert_eq!(c.request_reservation(), (0, 0));
    let third = c.begin_request(tid, &r).unwrap();
    masks(&mut c, &r);
    bins(&mut c, &r);
    assert!(c.ready(tid, &r).unwrap());
    c.end_request(third);
}
#[test]
fn legacy_multi_request_admission_remains_opt_in() {
    let f = fixture();
    let tid = &f.manifest.tid;
    let mut c = client(8192);
    c.install_mask(tid, 0, 0, &f.masks[0][0]).unwrap();
    c.install_mask(tid, 1, 0, &f.masks[1][0]).unwrap();
    assert_eq!(c.missing_masks(tid, &region(0)).unwrap(), vec![0]);
    assert_eq!(c.request_reservation(), (0, 0));
    assert_eq!(c.metrics.mask_evictions, 1);
    let mut other = f.manifest.clone();
    other.identity.codec_revision = "another".into();
    other.seal().unwrap();
    c.register(other.clone()).unwrap();
    c.install_mask(&other.tid, 0, 0, &f.masks[0][0]).unwrap();
    assert_eq!(c.resident_bytes().0, 8192);
    assert_eq!(c.request_reservation(), (0, 0));
}

#[test]
fn unrelated_representation_is_reclaimable_but_active_identity_is_exclusive() {
    let f = fixture();
    let r = region(240);
    let tid = &f.manifest.tid;
    let limit = required(&r);
    let mut c = client(limit);
    let mut other = f.manifest.clone();
    other.identity.source_sha256 = sha256(b"other authored representation");
    other.seal().unwrap();
    c.register(other.clone()).unwrap();
    let fields = jpip::ResponseFields::parse(&other.tid, "768,256", "0,0", "768,256").unwrap();
    let mut old = c.begin_response(&other.tid, &fields).unwrap();
    let wire = message(
        jpip::BinKey::new(0, 90000).unwrap(),
        0,
        true,
        &vec![7; limit],
    );
    c.receive(&mut old, &wire).unwrap();
    c.receive(&mut old, &jpip::encode_end(2)).unwrap();
    c.finish(old).unwrap();
    let scope = c.begin_request(tid, &r).unwrap();
    assert_eq!(c.metrics.representation_evictions, 1);
    assert!(c.register(other.clone()).is_err());
    assert!(c.begin_response(&other.tid, &fields).is_err());
    assert!(c.request(tid, &region(496), 1024).is_err());
    masks(&mut c, &r);
    bins(&mut c, &r);
    assert!(c.ready(tid, &r).unwrap());
    c.end_request(scope);
    c.register(other).unwrap();
    assert_eq!(c.request_reservation(), (0, 0));
}

#[test]
fn legacy_sparse_pressure_fails_admission_explicitly_and_leaves_no_scope() {
    // The legacy owner's model deliberately hides non-prefix holes. Such an
    // unscoped cache cannot promise selective reclamation. Worker-owned scoped
    // lifetimes avoid this case by reclaiming incomplete keys on release.
    let r = region(240);
    let f = fixture();
    let limit = required(&r);
    let mut c = client(limit);
    receive(
        &mut c,
        &message(
            jpip::BinKey::new(0, 90000).unwrap(),
            1,
            false,
            &vec![7; limit],
        ),
    )
    .unwrap();
    let error = c
        .begin_request(&f.manifest.tid, &r)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("unrelated sparse cache occupancy"),
        "{error}"
    );
    assert_eq!(c.request_reservation(), (0, 0));
    assert_eq!(c.resident_bytes().0, limit);
    assert_eq!(c.metrics.decode_count, 0);
}
