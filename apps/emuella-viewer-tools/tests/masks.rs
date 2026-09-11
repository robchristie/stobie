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
    let p = profile();
    let identity = Identity {
        validity: Some(ValidityIdentity {
            source_sha256: sha256(b"authored original masks"),
            bands: vec![3, 2, 1],
            policy: validity::POLICY.into(),
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
