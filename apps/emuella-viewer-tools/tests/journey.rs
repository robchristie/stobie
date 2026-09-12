use emuella_viewer_source::*;
use emuella_viewer_tools::*;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};
fn profile() -> Profile {
    Profile {
        width: 516,
        height: 260,
        tile_edge: 256,
        decomposition_levels: 5,
        bits_per_sample: 11,
        components: 1,
        bits_per_pixel: 3.,
    }
}
fn region() -> Region {
    Region {
        x: 247,
        y: 7,
        width: 263,
        height: 249,
        discard: 2,
        components: vec![0],
    }
}
fn install(client: &mut SharedClient, service: &mut Service, manifest: &Manifest, region: &Region) {
    client.register(manifest.clone()).unwrap();
    for tile in client.missing_tiles(&manifest.tid, region).unwrap() {
        let response = service
            .route(&format!(
                "/descriptor/{}/{tile}?tid={}",
                manifest.target, manifest.tid
            ))
            .unwrap();
        client
            .install_descriptor(&manifest.tid, tile, &response.body)
            .unwrap();
    }
}
fn fields(response: &HttpResponse) -> jpip::ResponseFields {
    let get = |name: &str| {
        response
            .headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .unwrap()
            .1
            .as_str()
    };
    checked(jpip::ResponseFields::parse(
        get("JPIP-tid"),
        get("JPIP-fsiz"),
        get("JPIP-roff"),
        get("JPIP-rsiz"),
    ))
    .unwrap()
}
#[test]
fn restart_fragment_retry_reuse_and_reference() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("rep");
    let (manifest, _) = fixture(&root, "test", profile(), "fixture-revision").unwrap();
    let mut service = Service::open(std::slice::from_ref(&root), None, true).unwrap();
    let mut client = SharedClient::new(ClientLimits::default());
    let region = region();
    install(&mut client, &mut service, &manifest, &region);
    let request = client.request(&manifest.tid, &region, 512).unwrap();
    let response = service
        .route(&format!("/jpip?{}", checked(request.query()).unwrap()))
        .unwrap();
    let mut reader = client
        .begin_response(&manifest.tid, &fields(&response))
        .unwrap();
    client
        .receive(&mut reader, &response.body[..response.body.len() / 2])
        .unwrap();
    assert!(client.finish(reader).is_err());
    drop(service);
    let mut service = Service::open(std::slice::from_ref(&root), None, false).unwrap();
    for attempt in 0..100 {
        let request = client.request(&manifest.tid, &region, 1024).unwrap();
        let response = service
            .route(&format!("/jpip?{}", checked(request.query()).unwrap()))
            .unwrap();
        let mut reader = client
            .begin_response(&manifest.tid, &fields(&response))
            .unwrap();
        for bytes in response.body.chunks(7) {
            client.receive(&mut reader, bytes).unwrap();
        }
        client.finish(reader).unwrap();
        if client.decode(&manifest.tid, &region).is_ok() {
            break;
        }
        assert!(attempt < 99, "retry did not converge");
    }
    let decoded = client.decode(&manifest.tid, &region).unwrap();
    assert_eq!((decoded.width, decoded.height), (66, 62));
    let mut index = manifest.sparse().unwrap();
    for tile in region.tiles(&profile()).unwrap() {
        checked(index.import_tile_descriptor(
            &std::fs::read(root.join("descriptors").join(format!("{tile}.bin"))).unwrap(),
        ))
        .unwrap();
    }
    let bytes = std::fs::read(root.join("payload.j2c")).unwrap();
    let expected = checked(index.plan(region.rect(), region.discard, &region.components))
        .unwrap()
        .decode(
            |offset, out| {
                out.copy_from_slice(&bytes[offset as usize..offset as usize + out.len()]);
                Ok(())
            },
            &mut codec::ht_lossy::LossyHtSpatialRegionWorkspace::new(),
        )
        .unwrap();
    assert_eq!(decoded.planes, expected);
    let before = service.metrics.logical_read_bytes;
    let response = service
        .route(&format!(
            "/jpip?{}",
            checked(
                client
                    .request(&manifest.tid, &region, 1024)
                    .unwrap()
                    .query()
            )
            .unwrap()
        ))
        .unwrap();
    assert_eq!(response.body.len(), 3);
    assert_eq!(service.metrics.logical_read_bytes, before);
    assert!(service.route("/descriptor/../../bad/0?tid=x").is_err());
    let mut corrupted = manifest.clone();
    corrupted.identity.profile.bits_per_pixel += 1.;
    assert!(corrupted.validate().is_err());
}
#[test]
fn actual_http_loopback_catalogue_and_jpp() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("rep");
    let (manifest, _) = fixture(&root, "wire", profile(), "fixture-revision").unwrap();
    let mut service = Service::open(&[root], None, true).unwrap();
    let region = region();
    let mut client = SharedClient::new(ClientLimits::default());
    install(&mut client, &mut service, &manifest, &region);
    let query = checked(
        client
            .request(&manifest.tid, &region, 1 << 20)
            .unwrap()
            .query(),
    )
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        for _ in 0..2 {
            let (stream, _) = listener.accept().unwrap();
            service.handle(stream).unwrap();
        }
    });
    for path in ["/catalogue".to_string(), format!("/jpip?{query}")] {
        let mut stream = TcpStream::connect(address).unwrap();
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: {address}\r\nOrigin: http://{address}\r\nSec-Fetch-Site: same-origin\r\n\r\n"
        )
        .unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        let end = bytes.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        let header = std::str::from_utf8(&bytes[..end]).unwrap();
        assert!(header.starts_with("HTTP/1.1 200"));
        assert!(!header.to_ascii_lowercase().contains("access-control-"));
        assert!(header.contains("Cross-Origin-Resource-Policy: same-origin\r\n"));
        assert!(header.contains("X-Content-Type-Options: nosniff\r\n"));
        if path == "/catalogue" {
            let catalogue: Vec<Manifest> = serde_json::from_slice(&bytes[end..]).unwrap();
            assert_eq!(catalogue[0], manifest);
        } else {
            let get = |name: &str| {
                header
                    .lines()
                    .filter_map(|l| l.split_once(": "))
                    .find(|(n, _)| n.eq_ignore_ascii_case(name))
                    .unwrap()
                    .1
            };
            let fields = checked(jpip::ResponseFields::parse(
                get("JPIP-tid"),
                get("JPIP-fsiz"),
                get("JPIP-roff"),
                get("JPIP-rsiz"),
            ))
            .unwrap();
            let mut reader = client.begin_response(&manifest.tid, &fields).unwrap();
            client.receive(&mut reader, &bytes[end..]).unwrap();
            client.finish(reader).unwrap();
            client.decode(&manifest.tid, &region).unwrap();
        }
    }
    handle.join().unwrap();
}

#[test]
fn actual_http_rejects_foreign_and_malformed_browser_authorities_before_routing() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("rep");
    let (manifest, _) = fixture(&root, "private-wire", profile(), "fixture-revision").unwrap();
    let web = dir.path().join("web");
    std::fs::create_dir(&web).unwrap();
    std::fs::write(web.join("index.html"), "local viewer").unwrap();
    let mut service = Service::open(&[root], Some(web), true).unwrap();
    let mut client = SharedClient::new(ClientLimits::default());
    install(&mut client, &mut service, &manifest, &region());
    service.metrics = IoMetrics::default();
    let query = checked(
        client
            .request(&manifest.tid, &region(), 1024)
            .unwrap()
            .query(),
    )
    .unwrap();
    let paths = [
        "/catalogue".to_owned(),
        format!("/descriptor/private-wire/0?tid={}", manifest.tid),
        format!("/jpip?{query}"),
        "/".to_owned(),
    ];
    let headers = [
        "Host: rebound.example:8123\r\n",
        "Host: 192.0.2.1\r\n",
        "Host: [2001:db8::1]\r\n",
        "Host: localhost.attacker.example\r\n",
        "Host: localhost\r\nOrigin: https://attacker.example\r\n",
        "Host: localhost\r\nOrigin: null\r\n",
        "Host: localhost\r\nOrigin: http://localhost:8123\r\n",
        "Host: localhost\r\nOrigin: http://127.0.0.1\r\n",
        "Host: localhost\r\nOrigin: https://localhost\r\n",
        "Host: localhost\r\nOrigin: http://localhost/\r\n",
        "Host: localhost\r\nOrigin: http://localhost http://localhost\r\n",
        "Host: localhost\r\nOrigin: http://localhost\r\norigin: http://localhost\r\n",
        "Host: localhost\r\nSec-Fetch-Site: cross-site\r\n",
        "Host: localhost\r\nSec-Fetch-Site: same-site\r\n",
        "Host: localhost\r\nSec-Fetch-Site: invalid\r\n",
        "Host: localhost\r\nSec-Fetch-Site: none\r\nsec-fetch-site: same-origin\r\n",
        "Host: localhost\r\nhOsT: localhost\r\n",
        "Host: localhost, localhost\r\n",
        "Host: localhost:\r\n",
        "Host: localhost:65536\r\n",
        "Host: localhost:0\r\n",
        "Host: localhost:+80\r\n",
        "Host: localhost:80:80\r\n",
        "Host: localhost/path\r\n",
        "Host: user@localhost\r\n",
        "Host: localhost#fragment\r\n",
        "Host: [::1\r\n",
        "Host: [::1]suffix\r\n",
        "Host: ::1\r\n",
        "Host: [::1]:bad\r\n",
        "Host: \r\n",
        "",
        "Host : localhost\r\n",
        "Host: localhost\r\n folded: value\r\n",
        "Host: localhost\nOrigin: http://localhost\r\n",
    ];
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let count = headers.len() * paths.len();
    let handle = std::thread::spawn(move || {
        for _ in 0..count {
            let (stream, _) = listener.accept().unwrap();
            service.handle(stream).unwrap();
        }
        assert_eq!(service.metrics.logical_read_bytes, 0);
        assert_eq!(service.metrics.descriptor_read_operations, 0);
        assert_eq!(service.metrics.jpp_bytes, 0);
    });
    for header in headers {
        for path in &paths {
            let mut stream = TcpStream::connect(address).unwrap();
            write!(stream, "GET {path} HTTP/1.1\r\n{header}\r\n").unwrap();
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).unwrap();
            let response = std::str::from_utf8(&bytes).unwrap();
            assert!(
                response.starts_with("HTTP/1.1 400"),
                "{header:?}: {response}"
            );
            assert!(!response.to_ascii_lowercase().contains("access-control-"));
            assert!(!response.contains(&manifest.tid));
            assert!(!response.contains("local viewer"));
        }
    }
    handle.join().unwrap();
}

#[test]
fn actual_http_accepts_native_and_same_origin_loopback_profiles() {
    let mut service = Service::open(&[], None, false).unwrap();
    let headers = [
        "Host: localhost\r\n",
        "Host: 127.0.0.1:8123\r\n",
        "Host: [::1]:8123\r\n",
        "Host: 127.1.2.3\r\n",
        "hOsT: LOCALHOST:8123\r\noRiGiN: http://localhost:8123\r\n",
        "Host: localhost:80\r\nOrigin: http://localhost\r\n",
        "Host: [::1]\r\nOrigin: http://[::1]:80\r\n",
        "Host: localhost\r\nSec-Fetch-Site: same-origin\r\n",
        "Host: localhost\r\nSec-Fetch-Site: none\r\n",
        "Host: 127.0.0.1:8123\r\nOrigin: http://127.0.0.1:8123\r\nSec-Fetch-Site: same-origin\r\n",
    ];
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        for _ in 0..headers.len() {
            let (stream, _) = listener.accept().unwrap();
            service.handle(stream).unwrap();
        }
    });
    for header in headers {
        let mut stream = TcpStream::connect(address).unwrap();
        write!(stream, "GET /catalogue HTTP/1.1\r\n{header}\r\n").unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(
            response.starts_with("HTTP/1.1 200"),
            "{header:?}: {response}"
        );
        assert!(response.ends_with("\r\n\r\n[]"));
    }
    handle.join().unwrap();
}
#[test]
fn incomplete_preparation_never_publishes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("bad");
    let good = dir.path().join("good");
    let (m, _) = fixture(&good, "source", profile(), "fixture-revision").unwrap();
    assert!(
        prepare(&root, "bad", m.identity, |_, _| anyhow::bail!(
            "injected source failure"
        ))
        .is_err()
    );
    assert!(!root.exists());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
#[test]
fn global_identity_eviction_and_descriptor_authentication() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("rep");
    let (m, _) = fixture(&root, "first", profile(), "fixture-revision").unwrap();
    let mut client = SharedClient::new(ClientLimits {
        representations: 1,
        ..ClientLimits::default()
    });
    client.register(m.clone()).unwrap();
    let mut other = m.clone();
    other.identity.spatial_policy_sha256 = sha256(b"future-policy");
    other.seal().unwrap();
    client.register(other.clone()).unwrap();
    assert!(client.missing_tiles(&m.tid, &region()).is_err());
    let bytes = std::fs::read(root.join("descriptors/0.bin")).unwrap();
    let mut bad = bytes.clone();
    bad[0] ^= 1;
    assert!(client.install_descriptor(&other.tid, 0, &bad).is_err());
    client.install_descriptor(&other.tid, 0, &bytes).unwrap();
    assert!(client.resident_bytes().1 <= ClientLimits::default().descriptor_bytes);
}

#[test]
fn compressed_eviction_counts_payload_not_response_framing() {
    let dir = tempfile::tempdir().unwrap();
    let (first, _) = fixture(
        &dir.path().join("rep"),
        "first",
        profile(),
        "fixture-revision",
    )
    .unwrap();
    let mut second = first.clone();
    second.identity.spatial_policy_sha256 = sha256(b"distinct-source-policy");
    second.seal().unwrap();
    let mut client = SharedClient::new(ClientLimits {
        compressed_bytes: 100,
        ..ClientLimits::default()
    });
    client.register(first.clone()).unwrap();
    client.register(second.clone()).unwrap();
    let response_fields =
        |tid: &str| jpip::ResponseFields::parse(tid, "516,260", "0,0", "516,260").unwrap();
    let message = |id, size| {
        let payload = vec![7; size];
        let mut wire = jpip::encode_message(jpip::DataMessage {
            key: jpip::BinKey::new(0, id).unwrap(),
            offset: 0,
            final_bin: true,
            auxiliary: None,
            bytes: &payload,
        })
        .unwrap();
        wire.extend_from_slice(&jpip::encode_end(2));
        wire
    };
    for (manifest, size) in [(&first, 50), (&second, 30)] {
        let mut reader = client
            .begin_response(&manifest.tid, &response_fields(&manifest.tid))
            .unwrap();
        for fragment in message(0, size).chunks(3) {
            client.receive(&mut reader, fragment).unwrap();
        }
        client.finish(reader).unwrap();
    }
    assert_eq!(client.resident_bytes().0, 80);
    // A legal nonempty EOR body must not evict unrelated compressed data.
    let mut eor = vec![0, 2, 80];
    eor.extend_from_slice(&[42; 80]);
    let mut reader = client
        .begin_response(&second.tid, &response_fields(&second.tid))
        .unwrap();
    client.receive(&mut reader, &eor).unwrap();
    client.finish(reader).unwrap();
    assert_eq!(client.resident_bytes().0, 80);
    assert_eq!(client.metrics.representation_evictions, 0);
    let mut reader = client
        .begin_response(&second.tid, &response_fields(&second.tid))
        .unwrap();
    client.receive(&mut reader, &message(1, 80)).unwrap();
    client.finish(reader).unwrap();
    assert_eq!(client.metrics.representation_evictions, 1);
    assert_eq!(client.metrics.compressed_bin_evictions, 1);
    assert_eq!(client.resident_bytes().0, 80);
    assert!(client.metrics.peak_compressed_bytes <= 100);
}

#[test]
fn descriptor_eviction_requires_bounded_window_readiness_recheck() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("rep");
    let (manifest, _) = fixture(
        &root,
        "metadata",
        Profile {
            width: 1024,
            height: 512,
            ..profile()
        },
        "fixture-revision",
    )
    .unwrap();
    let descriptor = |tile| std::fs::read(root.join(format!("descriptors/{tile}.bin"))).unwrap();
    let mut index = manifest.sparse().unwrap();
    let mut raw_bytes = 0;
    for tile in [4, 5, 0] {
        let bytes = descriptor(tile);
        index.import_tile_descriptor(&bytes).unwrap();
        raw_bytes += bytes.len();
    }
    let mut probe = SharedClient::new(ClientLimits::default());
    probe.register(manifest.clone()).unwrap();
    // Preserve the three-descriptor pressure boundary while paying the fixed
    // mask-cache container even for this representation without source masks.
    let budget =
        raw_bytes + index.retained_heap_bytes() as usize + probe.mask_cache_metadata_bytes();
    let mut client = SharedClient::new(ClientLimits {
        descriptor_bytes: budget,
        ..ClientLimits::default()
    });
    client.register(manifest.clone()).unwrap();
    for tile in [4, 5] {
        client
            .install_descriptor(&manifest.tid, tile, &descriptor(tile))
            .unwrap();
    }
    let window = Region {
        x: 247,
        y: 7,
        width: 263,
        height: 249,
        discard: 2,
        components: vec![0],
    };
    for tile in client.missing_tiles(&manifest.tid, &window).unwrap() {
        client
            .install_descriptor(&manifest.tid, tile, &descriptor(tile))
            .unwrap();
    }
    // Admission of the last descriptor reclaimed an earlier descriptor in this
    // window. Missing metadata is ordinary cache state, not corrupt imagery.
    assert!(
        !client
            .missing_tiles(&manifest.tid, &window)
            .unwrap()
            .is_empty()
    );
    assert!(!client.ready(&manifest.tid, &window).unwrap());
    for tile in client.missing_tiles(&manifest.tid, &window).unwrap() {
        client
            .install_descriptor(&manifest.tid, tile, &descriptor(tile))
            .unwrap();
    }
    assert!(
        client
            .missing_tiles(&manifest.tid, &window)
            .unwrap()
            .is_empty()
    );
    assert!(client.metrics.peak_descriptor_bytes <= budget);
}

#[test]
fn stored_payload_and_descriptor_corruption_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("rep");
    let (manifest, _) = fixture(&root, "integrity", profile(), "fixture-revision").unwrap();
    let payload = root.join("payload.j2c");
    let mut bytes = std::fs::read(&payload).unwrap();
    bytes[8] ^= 1;
    std::fs::write(&payload, &bytes).unwrap();
    assert!(Service::open(std::slice::from_ref(&root), None, true).is_err());
    // Explicit trusted-store mode skips payload hashing, but descriptor hashes remain mandatory.
    let mut service = Service::open(std::slice::from_ref(&root), None, false).unwrap();
    let descriptor = root.join("descriptors/0.bin");
    let mut bytes = std::fs::read(&descriptor).unwrap();
    bytes[10] ^= 1;
    std::fs::write(descriptor, bytes).unwrap();
    assert!(
        service
            .route(&format!("/descriptor/integrity/0?tid={}", manifest.tid))
            .is_err()
    );
}
#[test]
fn effective_resolution_rounding_and_quality_admission() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("rep");
    let (manifest, _) = fixture(&root, "rounding", profile(), "fixture-revision").unwrap();
    let request = jpip::Request {
        target: manifest.target.clone(),
        tid: manifest.tid.clone(),
        frame: [200, 100],
        offset: [91, 2],
        size: [109, 98],
        components: vec![0],
        layers: Some(1),
        max_length: 1024,
        extended: true,
        model: Default::default(),
    };
    let (region, fields) = effective_region(&manifest, &request).unwrap();
    assert_eq!(fields.frame, [129, 65]);
    assert_eq!(fields.offset, [58, 1]);
    assert_eq!(fields.size, [71, 64]);
    assert_eq!(
        (
            region.x,
            region.y,
            region.width,
            region.height,
            region.discard
        ),
        (232, 4, 284, 256, 2)
    );
    let mut unsupported = request;
    unsupported.layers = Some(2);
    assert!(effective_region(&manifest, &unsupported).is_err());
}

#[test]
fn representative_u16_cross_tile_reference() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("large");
    let p = Profile {
        width: 2048,
        height: 2048,
        tile_edge: 512,
        decomposition_levels: 5,
        bits_per_sample: 16,
        components: 1,
        bits_per_pixel: 2.,
    };
    let (manifest, _) = fixture(&root, "large", p, "fixture-revision").unwrap();
    let region = Region {
        x: 247,
        y: 117,
        width: 769,
        height: 513,
        discard: 2,
        components: vec![0],
    };
    let mut index = manifest.sparse().unwrap();
    for tile in region.tiles(&manifest.identity.profile).unwrap() {
        checked(index.import_tile_descriptor(
            &std::fs::read(root.join("descriptors").join(format!("{tile}.bin"))).unwrap(),
        ))
        .unwrap();
    }
    let bytes = std::fs::read(root.join("payload.j2c")).unwrap();
    let plan = checked(index.plan(region.rect(), region.discard, &region.components)).unwrap();
    let mut reads = 0;
    let expected = plan.decode(
        |offset, out| {
            reads += 1;
            out.copy_from_slice(&bytes[offset as usize..offset as usize + out.len()]);
            Ok(())
        },
        &mut codec::ht_lossy::LossyHtSpatialRegionWorkspace::new(),
    );
    assert!(
        expected.is_ok(),
        "direct file decode: {:?}, reads={reads}, blocks={}, workspace={}",
        expected.err(),
        plan.selected_code_blocks(),
        plan.required_workspace_bytes()
    );
}
