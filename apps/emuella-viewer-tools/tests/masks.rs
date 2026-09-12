use emuella_viewer_source::{
    ClientLimits, Identity, Manifest, Profile, Region, SharedClient, checked, sha256,
    validity::{self, ValidityIdentity},
};
use emuella_viewer_tools::{Service, prepare_with_validity, synthetic_tile};
fn profile() -> Profile {
    Profile {
        width: 517,
        height: 261,
        tile_edge: 256,
        decomposition_levels: 2,
        bits_per_sample: 8,
        components: 3,
        bits_per_pixel: 3.0,
    }
}
fn valid(x: u32, y: u32, c: u16) -> bool {
    x != 255 && y != 256 && !(c == 1 && x == 256) && !(c == 2 && x < 3 && y < 3)
}
fn prepare(root: &std::path::Path) -> Manifest {
    prepare_policy(root, validity::POLICY)
}
fn prepare_policy(root: &std::path::Path, policy: &str) -> Manifest {
    let p = profile();
    let identity = Identity {
        validity: Some(ValidityIdentity {
            source_sha256: sha256(b"authored original masks"),
            bands: vec![3, 2, 1],
            policy: policy.into(),
            tile_sha256: Vec::new(),
        }),
        source_sha256: sha256(b"authored samples"),
        bands: vec![1, 2, 3],
        profile: p.clone(),
        codec_revision: "authored".into(),
        encoding_contract: "test".into(),
        spatial_policy_sha256: sha256(b"uniform"),
        payload_sha256: String::new(),
        descriptor_format: "EHTIDX01".into(),
    };
    let mut masks = |r: emuella_viewer_source::codec::TileRect| {
        Ok((0..3)
            .map(|c| {
                (0..r.height)
                    .flat_map(|y| {
                        (0..r.width).map(move |x| if valid(x + r.x, y + r.y, c) { 255 } else { 0 })
                    })
                    .collect()
            })
            .collect())
    };
    prepare_with_validity(
        root,
        "authored",
        identity,
        |r, b| synthetic_tile(&p, r, b),
        false,
        Some(&mut masks),
    )
    .unwrap()
    .0
}
fn load(client: &mut SharedClient, service: &mut Service, m: &Manifest, r: &Region) {
    client.register(m.clone()).unwrap();
    for t in client.missing_tiles(&m.tid, r).unwrap() {
        let b = service
            .route(&format!("/descriptor/{}/{t}?tid={}", m.target, m.tid))
            .unwrap()
            .body;
        client.install_descriptor(&m.tid, t, &b).unwrap();
    }
    for t in client.missing_masks(&m.tid, r).unwrap() {
        let b = service
            .route(&format!(
                "/mask/{}/{}/{t}?tid={}",
                m.target, r.discard, m.tid
            ))
            .unwrap()
            .body;
        client.install_mask(&m.tid, t, r.discard, &b).unwrap();
    }
    for _ in 0..64 {
        if client.ready(&m.tid, r).unwrap() {
            return;
        }
        let q = client.request(&m.tid, r, 256 << 10).unwrap();
        let response = service
            .route(&format!("/jpip?{}", checked(q.query()).unwrap()))
            .unwrap();
        let mut reader = client
            .begin_response_headers(
                &m.tid,
                response
                    .headers
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str())),
            )
            .unwrap();
        client.receive(&mut reader, &response.body).unwrap();
        client.finish(reader).unwrap();
    }
    panic!("dependencies did not converge");
}
#[test]
fn compact_preparation_conversion_and_scoped_lifecycle_preserve_exact_pixels() {
    let temp = tempfile::tempdir().unwrap();
    let old_root = temp.path().join("old");
    let new_root = temp.path().join("new");
    let prepared_root = temp.path().join("prepared");
    let old = prepare(&old_root);
    let report = emuella_viewer_tools::compact::convert(&old_root, &new_root).unwrap();
    assert_eq!(report["payload_and_descriptors_unchanged"], true);
    let new: Manifest =
        serde_json::from_slice(&std::fs::read(new_root.join("manifest.json")).unwrap()).unwrap();
    let prepared = prepare_policy(&prepared_root, validity::COMPACT_POLICY);
    assert_eq!(new, prepared);
    assert_ne!(old.tid, new.tid);
    assert_eq!(old.identity.payload_sha256, new.identity.payload_sha256);
    for root in [&new_root, &prepared_root] {
        let mut service = Service::open(std::slice::from_ref(root), None, true).unwrap();
        let mut client = SharedClient::new(ClientLimits::default());
        client.register(new.clone()).unwrap();
        for d in 0..=2 {
            for components in [vec![0], vec![1], vec![0, 1, 2]] {
                let r = Region {
                    x: 251,
                    y: 251,
                    width: 266,
                    height: 10,
                    discard: d,
                    components,
                };
                for t in client.missing_tiles(&new.tid, &r).unwrap() {
                    let bytes = std::fs::read(root.join(format!("descriptors/{t}.bin"))).unwrap();
                    client.install_descriptor(&new.tid, t, &bytes).unwrap();
                }
                let scope = client.begin_request(&new.tid, &r).unwrap();
                assert!(client.request_reservation().0 > 0);
                assert!(client.request_reservation().1 > 0);
                let before = client.resident_bytes();
                assert!(
                    client
                        .install_mask(
                            &new.tid,
                            r.tiles(&new.identity.profile).unwrap()[0],
                            d,
                            &[9]
                        )
                        .is_err()
                );
                assert_eq!(client.resident_bytes(), before);
                client.end_request(scope); // Failure/cancellation releases pins before recovery.
                assert_eq!(client.request_reservation(), (0, 0));
                let scope = client.begin_request(&new.tid, &r).unwrap();
                load(&mut client, &mut service, &new, &r);
                let decoded = client.decode(&new.tid, &r).unwrap();
                client.end_request(scope);
                assert_eq!(client.request_reservation(), (0, 0));
                let mut legacy_client = SharedClient::new(ClientLimits::default());
                let mut legacy_service =
                    Service::open(std::slice::from_ref(&old_root), None, true).unwrap();
                load(&mut legacy_client, &mut legacy_service, &old, &r);
                let legacy = legacy_client.decode(&old.tid, &r).unwrap();
                assert_eq!(decoded.planes, legacy.planes);
                assert_eq!(decoded.validity, legacy.validity);
            }
        }
        assert!(client.compact_catalogue_metadata_bytes() > 0);
        let bytes = std::fs::read(root.join("masks/0/0.bin")).unwrap();
        let mut pressure = SharedClient::new(ClientLimits {
            compressed_bytes: bytes.len(),
            ..Default::default()
        });
        pressure.register(new.clone()).unwrap();
        pressure.install_mask(&new.tid, 0, 0, &bytes).unwrap();
        let second = std::fs::read(root.join("masks/0/1.bin")).unwrap();
        pressure.install_mask(&new.tid, 1, 0, &second).unwrap();
        assert_eq!(pressure.metrics.mask_evictions, 1);
        pressure.install_mask(&new.tid, 0, 0, &bytes).unwrap();
        assert_eq!(pressure.metrics.mask_evictions, 2);
        assert_eq!(pressure.request_reservation(), (0, 0));
    }
}
#[test]
fn exact_native_reduced_regional_masks_cross_tiles_without_changing_samples() {
    let temp = tempfile::tempdir().unwrap();
    let root = std::env::var_os("EMUELLA_MASK_FIXTURE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| temp.path().join("rep"));
    let m = prepare(&root);
    let mut service = Service::open(std::slice::from_ref(&root), None, true).unwrap();
    let mut client = SharedClient::new(ClientLimits::default());
    let mut evidence = Vec::new();
    for d in 0..=2 {
        for components in [vec![0], vec![1], vec![0, 1, 2]] {
            let r = Region {
                x: 251,
                y: 251,
                width: 266,
                height: 10,
                discard: d,
                components,
            };
            load(&mut client, &mut service, &m, &r);
            let decoded = client.decode(&m.tid, &r).unwrap();
            let scale = 1 << d;
            let x0 = r.x.div_ceil(scale);
            let y0 = r.y.div_ceil(scale);
            let expected: Vec<_> = (0..decoded.height)
                .flat_map(|y| {
                    let r = &r;
                    (0..decoded.width).map(move |x| {
                        u8::from(r.components.iter().all(|&c| {
                            ((y + y0) * scale
                                ..((y + y0 + 1) * scale).min(m.identity.profile.height))
                                .all(|sy| {
                                    ((x + x0) * scale
                                        ..((x + x0 + 1) * scale).min(m.identity.profile.width))
                                        .all(|sx| valid(sx, sy, c))
                                })
                        }))
                    })
                })
                .collect();
            assert_eq!(decoded.validity.as_ref().unwrap(), &expected);
            let mut bare = m.clone();
            bare.identity.validity = None;
            bare.seal().unwrap();
            // Codec planes are independently reconstructed without consulting validity.
            let reference = emuella_viewer_tools::reference::Reference::open(&root)
                .unwrap()
                .decode(&r, &mut Default::default())
                .unwrap();
            let samples: Vec<_> = (0..(decoded.width * decoded.height) as usize)
                .flat_map(|i| decoded.planes.iter().map(move |p| u16::from(p[i])))
                .collect();
            assert_eq!(samples, reference.samples);
            evidence.push(serde_json::json!({"region":r,"width":decoded.width,"height":decoded.height,"validity":expected,"fnv1a64_u16le":emuella_viewer_tools::reference::checksum(&samples)}));
        }
    }
    if std::env::var_os("EMUELLA_MASK_FIXTURE").is_some() {
        std::fs::write(
            root.join("native-regions.json"),
            serde_json::to_vec_pretty(&evidence).unwrap(),
        )
        .unwrap();
    }
    assert!(client.metrics.received_mask_bytes > 0);
    assert_eq!(client.mask_bytes(), client.metrics.peak_mask_bytes);
}
#[test]
fn missing_corrupt_stale_and_evicted_masks_fail_explicitly() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("rep");
    let m = prepare(&root);
    let mut service = Service::open(std::slice::from_ref(&root), None, true).unwrap();
    let r = Region {
        x: 0,
        y: 0,
        width: 256,
        height: 256,
        discard: 0,
        components: vec![0, 1, 2],
    };
    let mut client = SharedClient::new(ClientLimits {
        compressed_bytes: 24576,
        ..ClientLimits::default()
    });
    client.register(m.clone()).unwrap();
    assert!(
        client
            .decode(&m.tid, &r)
            .unwrap_err()
            .to_string()
            .contains("required masks")
    );
    let bytes = service
        .route(&format!("/mask/authored/0/0?tid={}", m.tid))
        .unwrap()
        .body;
    let before = client.resident_bytes();
    let mut corrupt = bytes.clone();
    corrupt[0] ^= 1;
    assert!(client.install_mask(&m.tid, 0, 0, &corrupt).is_err());
    assert_eq!(before, client.resident_bytes());
    assert!(client.install_mask("stale", 0, 0, &bytes).is_err());
    client.install_mask(&m.tid, 0, 0, &bytes).unwrap();
    assert_eq!(client.mask_bytes(), 24576);
    let second = service
        .route(&format!("/mask/authored/0/1?tid={}", m.tid))
        .unwrap()
        .body;
    client.install_mask(&m.tid, 1, 0, &second).unwrap();
    assert_eq!(client.metrics.mask_evictions, 1);
    assert_eq!(client.mask_bytes(), 24576);
    assert_eq!(client.missing_masks(&m.tid, &r).unwrap(), vec![0]);
    assert!(service.route("/mask/authored/0/0?tid=stale").is_err());
    std::fs::write(root.join("masks/0/0.bin"), corrupt).unwrap();
    assert!(
        service
            .route(&format!("/mask/authored/0/0?tid={}", m.tid))
            .is_err()
    );
    std::fs::remove_file(root.join("masks/0/0.bin")).unwrap();
    assert!(
        service
            .route(&format!("/mask/authored/0/0?tid={}", m.tid))
            .is_err()
    );
}
#[test]
fn optional_masks_preserve_legacy_serialised_identity_and_failed_preparation_is_atomic() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("rep");
    let m = prepare(&root);
    let mut bare = m.clone();
    bare.identity.validity = None;
    bare.seal().unwrap();
    let json = serde_json::to_value(&bare).unwrap();
    assert!(json["identity"].get("validity").is_none());
    let round: Manifest = serde_json::from_value(json).unwrap();
    round.validate().unwrap();
    assert_eq!(bare.tid, round.tid);
    let p = profile();
    let mut reader = |_| anyhow::bail!("authored mask read failure");
    let target = temp.path().join("failed");
    assert!(
        prepare_with_validity(
            &target,
            "failed",
            m.identity,
            |r, b| synthetic_tile(&p, r, b),
            false,
            Some(&mut reader)
        )
        .is_err()
    );
    assert!(!target.exists());
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1);
}

#[test]
#[ignore = "requires EMUELLA_TEST_GDAL_LIBRARY; uses only an authored temporary TIFF"]
fn original_gdal_selected_band_masks_follow_nodata_without_lossy_sample_testing() {
    use std::ffi::{CString, c_char, c_int, c_void};
    let path = std::path::PathBuf::from(
        std::env::var_os("EMUELLA_TEST_GDAL_LIBRARY").expect("GDAL library"),
    );
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("authored.tif");
    let p = profile();
    emuella_viewer_tools::gdal::write_fixture(&path, &input, &p).unwrap();
    // SAFETY: the authored dataset is exclusively opened for metadata update;
    // GDAL's stable C ABI is used synchronously while the library remains live.
    unsafe {
        let library = libloading::Library::new(&path).unwrap();
        type Handle = *mut c_void;
        let open = library
            .get::<unsafe extern "C" fn(*const c_char, c_int) -> Handle>(b"GDALOpen\0")
            .unwrap();
        let name = CString::new(input.as_os_str().as_encoded_bytes()).unwrap();
        let dataset = open(name.as_ptr(), 1);
        assert!(!dataset.is_null());
        let band = library
            .get::<unsafe extern "C" fn(Handle, c_int) -> Handle>(b"GDALGetRasterBand\0")
            .unwrap();
        let nodata = library
            .get::<unsafe extern "C" fn(Handle, f64) -> c_int>(b"GDALSetRasterNoDataValue\0")
            .unwrap();
        for c in 1..=3 {
            assert_eq!(nodata(band(dataset, c), 191.), 0);
        }
        library
            .get::<unsafe extern "C" fn(Handle)>(b"GDALClose\0")
            .unwrap()(dataset);
    }
    let mut raster =
        emuella_viewer_tools::gdal::Raster::open(&path, &input, vec![3, 2, 1], 8, 1 << 20).unwrap();
    let rect = emuella_viewer_source::codec::TileRect {
        tile_index: 0,
        tile_x: 0,
        tile_y: 0,
        x: 0,
        y: 0,
        width: 10,
        height: 10,
    };
    let mut samples = vec![vec![0; 100]; 3];
    raster.read_tile(rect, &mut samples).unwrap();
    let masks = raster.read_masks(rect).unwrap();
    assert!(masks.iter().flatten().any(|&v| v == 0));
    assert!(masks.iter().flatten().any(|&v| v != 0));
    for (plane, mask) in samples.iter().zip(&masks) {
        for (&sample, &valid) in plane.iter().zip(mask) {
            assert_eq!(valid != 0, sample != 191);
        }
    }
}

#[test]
fn compact_constant_cache_is_one_byte_and_metadata_has_a_budget() {
    let temp = tempfile::tempdir().unwrap();
    let old_root = temp.path().join("old");
    let mut m = prepare(&old_root);
    let p = &m.identity.profile;
    let bytes = vec![1];
    let v = m.identity.validity.as_mut().unwrap();
    v.policy = validity::COMPACT_POLICY.into();
    v.tile_sha256 = vec![
        vec![
            validity::catalogue_entry(validity::COMPACT_POLICY, &bytes).unwrap();
            p.tiles() as usize
        ];
        usize::from(p.decomposition_levels) + 1
    ];
    m.seal().unwrap();
    let mut client = SharedClient::new(ClientLimits {
        compressed_bytes: 1,
        ..Default::default()
    });
    client.register(m.clone()).unwrap();
    client.install_mask(&m.tid, 0, 0, &bytes).unwrap();
    assert_eq!(client.mask_bytes(), 1);
    client.install_mask(&m.tid, 1, 0, &bytes).unwrap();
    assert_eq!(client.mask_bytes(), 1);
    assert_eq!(client.metrics.mask_evictions, 1);
    client.install_mask(&m.tid, 0, 0, &bytes).unwrap();
    assert_eq!(client.metrics.mask_evictions, 2);
    let mut tiny = SharedClient::new(ClientLimits {
        descriptor_bytes: 1,
        ..Default::default()
    });
    assert!(
        tiny.register(m)
            .unwrap_err()
            .to_string()
            .contains("metadata")
    );
}

#[test]
fn compact_catalogue_slots_are_precharged_before_allocation_and_released_with_representation() {
    let temp = tempfile::tempdir().unwrap();
    let mut manifest = prepare_policy(&temp.path().join("slots"), validity::COMPACT_POLICY);
    let validity = manifest.identity.validity.as_mut().unwrap();
    for level in &mut validity.tile_sha256 {
        for entry in level {
            *entry = validity::catalogue_entry(validity::COMPACT_POLICY, &[1]).unwrap();
        }
    }
    manifest.seal().unwrap();
    let mut probe = SharedClient::new(ClientLimits::default());
    probe.register(manifest.clone()).unwrap();
    let required = probe.resident_bytes().1;
    let expected_slots = manifest.identity.profile.tiles() as usize
        * (usize::from(manifest.identity.profile.decomposition_levels) + 1);
    assert_eq!(probe.mask_cache_slots(), expected_slots);
    let metadata = probe.mask_cache_metadata_bytes();
    assert_eq!(
        metadata,
        expected_slots * probe.mask_cache_slot_bytes() + probe.mask_cache_container_bytes()
    );
    assert_eq!(probe.mask_cache_entries(), 0);
    assert_eq!(probe.mask_bytes(), 0);
    let mut too_small = SharedClient::new(ClientLimits {
        descriptor_bytes: required - 1,
        ..Default::default()
    });
    assert!(
        too_small
            .register(manifest.clone())
            .unwrap_err()
            .to_string()
            .contains("metadata")
    );
    assert_eq!(too_small.resident_bytes(), (0, 0));
    assert_eq!(too_small.mask_cache_metadata_bytes(), 0);
    assert_eq!(too_small.peak_mask_cache_metadata_bytes(), 0);
    let mut client = SharedClient::new(ClientLimits {
        compressed_bytes: 1,
        descriptor_bytes: required,
        representations: 1,
        ..Default::default()
    });
    client.register(manifest.clone()).unwrap();
    for tile in [0, 1, 0] {
        client.install_mask(&manifest.tid, tile, 0, &[1]).unwrap();
        assert_eq!(client.mask_bytes(), 1);
        assert_eq!(client.mask_cache_entries(), 1);
        assert_eq!(client.mask_cache_metadata_bytes(), metadata);
        assert_eq!(client.resident_bytes().1, required);
        assert_eq!(client.request_reservation(), (0, 0));
    }
    assert_eq!(client.metrics.mask_evictions, 2);
    let before = client.resident_bytes();
    assert!(client.install_mask(&manifest.tid, 1, 0, &[0]).is_err());
    assert_eq!(client.resident_bytes(), before);
    assert_eq!(client.mask_cache_entries(), 1);
    // An impossible replacement is rejected before evicting an existing identity.
    let mut larger = manifest.clone();
    larger.identity.source_sha256 = sha256(b"different authenticated source");
    for entry in larger
        .identity
        .validity
        .as_mut()
        .unwrap()
        .tile_sha256
        .iter_mut()
        .flatten()
    {
        entry.reserve_exact(1024);
    }
    larger.seal().unwrap();
    assert!(client.register(larger).is_err());
    assert_eq!(client.metrics.representation_evictions, 0);
    assert_eq!(client.resident_bytes(), before);
    // Removing the optional contract retains only the fixed container, no catalogue slots.
    let mut unmasked = manifest;
    unmasked.identity.validity = None;
    unmasked.seal().unwrap();
    client.register(unmasked).unwrap();
    assert_eq!(client.metrics.representation_evictions, 1);
    assert_eq!(client.mask_cache_slots(), 0);
    assert_eq!(client.mask_cache_entries(), 0);
    assert_eq!(client.mask_bytes(), 0);
    assert_eq!(
        client.mask_cache_metadata_bytes(),
        client.mask_cache_container_bytes()
    );
    assert_eq!(client.peak_mask_cache_metadata_bytes(), metadata);
}
