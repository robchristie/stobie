use crate::engine::{CATALOGUE_LIMIT, Engine, Event, HTTP_LIMIT, Job, WorkerTiming, pacing_now_ms};
use anyhow::{Result, anyhow, ensure};
use emuella_viewer_source::{Manifest, ResponseReader, SharedClient};
use polyorama_runtime::{RegionalRequest, RequestToken};
use std::{
    collections::BTreeSet,
    io::Read,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

pub struct Executor {
    sender: mpsc::SyncSender<Job>,
    events: mpsc::Receiver<(Event, Option<Arc<std::sync::OnceLock<f64>>>)>,
    cancelled: Arc<Mutex<BTreeSet<RequestToken>>>,
}
impl Executor {
    pub fn new(server: String, compressed: usize, context: egui::Context) -> Self {
        Self::new_diagnostic(
            server,
            compressed,
            context,
            crate::DiagnosticOptions::default(),
        )
    }
    pub fn new_diagnostic(
        server: String,
        compressed: usize,
        context: egui::Context,
        diagnostic: crate::DiagnosticOptions,
    ) -> Self {
        let (sender, jobs) = mpsc::sync_channel::<Job>(1);
        let (events_tx, events) = mpsc::sync_channel(2);
        let cancelled = Arc::new(Mutex::new(BTreeSet::new()));
        let flags = cancelled.clone();
        std::thread::Builder::new()
            .name("emuella-regional-decoder".into())
            .spawn(move || {
                let mut engine = Engine::new(compressed);
                let http = reqwest::blocking::Client::builder()
                    .timeout(Duration::from_secs(15))
                    .build()
                    .expect("HTTP client");
                let catalogue = (|| -> Result<Vec<Manifest>> {
                    let bytes = body(
                        http.get(format!("{server}/catalogue")).send()?,
                        CATALOGUE_LIMIT,
                    )?;
                    let manifests: Vec<Manifest> = serde_json::from_slice(&bytes)?;
                    ensure!(manifests.len() <= 32, "catalogue capacity");
                    for m in &manifests {
                        m.validate()?;
                        if diagnostic.authored_immediate {
                            ensure!(
                                m.identity.encoding_contract == "authored-completion-diagnostic-v1"
                                    && m.identity.source_sha256
                                        == "authored-completion-diagnostic-v1"
                                    && m.identity.validity.is_none(),
                                "immediate results reject non-authored catalogue"
                            );
                        }
                    }
                    Ok(manifests)
                })();
                let event = match catalogue {
                    Ok(c) => Event::Catalogue(c),
                    Err(e) => Event::Failed {
                        request: None,
                        error: e.to_string(),
                        metrics: engine.snapshot(),
                    },
                };
                let _ = events_tx.send((event, None));
                context.request_repaint();
                while let Ok(job) = jobs.recv() {
                    let started = Instant::now();
                    let started_ms = pacing_now_ms();
                    let stopped = || {
                        flags
                            .lock()
                            .expect("cancellation lock")
                            .contains(&job.request.token)
                    };
                    let result = (|| -> Result<_> {
                        ensure!(!stopped(), "cancelled");
                        if diagnostic.authored_immediate {
                            return engine.authored_immediate(&job);
                        }
                        engine.client.register(job.manifest.clone())?;
                        // Metadata admission may evict an earlier tile from this batch.
                        // Recheck the complete bounded region before requesting any bins.
                        for _ in 0..3 {
                            for tile in engine
                                .client
                                .missing_tiles(&job.manifest.tid, &job.region())?
                            {
                                ensure!(!stopped(), "cancelled");
                                let bytes = body(
                                    http.get(format!(
                                        "{server}/descriptor/{}/{tile}?tid={}",
                                        job.manifest.target, job.manifest.tid
                                    ))
                                    .send()?,
                                    HTTP_LIMIT,
                                )?;
                                ensure!(!stopped(), "cancelled");
                                engine.client.install_descriptor(
                                    &job.manifest.tid,
                                    tile,
                                    &bytes,
                                )?;
                            }
                            if engine
                                .client
                                .missing_tiles(&job.manifest.tid, &job.region())?
                                .is_empty()
                            {
                                break;
                            }
                        }
                        ensure!(
                            engine
                                .client
                                .missing_tiles(&job.manifest.tid, &job.region())?
                                .is_empty(),
                            "regional descriptors exceed admitted metadata budget"
                        );
                        engine.begin_request(&job)?;
                        for tile in engine
                            .client
                            .missing_masks(&job.manifest.tid, &job.region())?
                        {
                            ensure!(!stopped(), "cancelled");
                            let bytes = body(
                                http.get(format!(
                                    "{server}/mask/{}/{}/{tile}?tid={}",
                                    job.manifest.target,
                                    job.region().discard,
                                    job.manifest.tid
                                ))
                                .send()?,
                                HTTP_LIMIT,
                            )?;
                            ensure!(!stopped(), "cancelled");
                            engine.client.install_mask(
                                &job.manifest.tid,
                                tile,
                                job.region().discard,
                                &bytes,
                            )?;
                        }
                        ensure!(
                            engine
                                .client
                                .missing_masks(&job.manifest.tid, &job.region())?
                                .is_empty(),
                            "regional masks exceed admitted budget"
                        );
                        if engine.client.ready(&job.manifest.tid, &job.region())? {
                            engine.metrics.cache_hits += 1;
                            return engine.decode(&job);
                        }
                        for attempt in 0..64 {
                            ensure!(!stopped(), "cancelled");
                            let request = engine.client.request(
                                &job.manifest.tid,
                                &job.region(),
                                (256 << 10) as u64,
                            )?;
                            engine.metrics.requests += 1;
                            if attempt > 0 {
                                engine.metrics.retries += 1;
                            }
                            let mut response = http
                                .get(format!(
                                    "{server}/jpip?{}",
                                    emuella_viewer_source::checked(request.query())?
                                ))
                                .send()?
                                .error_for_status()?;
                            ensure!(!stopped(), "cancelled");
                            let mut reader = begin_response(
                                &mut engine.client,
                                &job.manifest.tid,
                                response.headers(),
                            )?;
                            let mut chunk = [0; 8192];
                            let mut received = 0;
                            loop {
                                ensure!(!stopped(), "cancelled");
                                let n = response.read(&mut chunk)?;
                                if n == 0 {
                                    break;
                                }
                                received += n;
                                ensure!(received <= HTTP_LIMIT, "response limit");
                                engine.client.receive(&mut reader, &chunk[..n])?;
                            }
                            engine.client.finish(reader)?;
                            ensure!(!stopped(), "cancelled");
                            if engine.client.ready(&job.manifest.tid, &job.region())? {
                                return engine.decode(&job);
                            }
                        }
                        Err(anyhow!("regional continuation exhausted after 64 rounds"))
                    })();
                    engine.end_request();
                    engine.metrics.timing = Some(WorkerTiming {
                        started_ms,
                        finished_ms: pacing_now_ms(),
                        ..Default::default()
                    });
                    engine.metrics.elapsed_ms += started.elapsed().as_secs_f64() * 1000.;
                    let mut event = if stopped() {
                        engine.metrics.aborted += 1;
                        drop(result);
                        Event::Cancelled {
                            request: job.request.clone(),
                            metrics: engine.snapshot(),
                        }
                    } else {
                        match result {
                            Ok(pixels) => Event::Completed {
                                request: job.request.clone(),
                                pixels,
                                metrics: engine.snapshot(),
                            },
                            Err(e) => Event::Failed {
                                request: Some(job.request.clone()),
                                error: e.to_string(),
                                metrics: engine.snapshot(),
                            },
                        }
                    };
                    flags
                        .lock()
                        .expect("cancellation lock")
                        .remove(&job.request.token);
                    let wakeup = diagnostic
                        .enabled
                        .then(|| Arc::new(std::sync::OnceLock::new()));
                    if let Some(timing) = event.metrics_mut().and_then(|m| m.timing.as_mut()) {
                        timing.published_ms = pacing_now_ms();
                    }
                    if events_tx.send((event, wakeup.clone())).is_err() {
                        break;
                    }
                    if let Some(wakeup) = wakeup {
                        let _ = wakeup.set(pacing_now_ms());
                    }
                    context.request_repaint();
                }
            })
            .expect("decoder thread");
        Self {
            sender,
            events,
            cancelled,
        }
    }
    pub fn submit(&self, job: Job) -> Result<()> {
        self.sender
            .try_send(job)
            .map_err(|e| anyhow!(e.to_string()))
    }
    pub fn cancel(&self, request: &RegionalRequest) {
        self.cancelled
            .lock()
            .expect("cancellation lock")
            .insert(request.token);
    }
    pub fn drain(&self) -> Vec<Event> {
        self.events
            .try_iter()
            .map(|(mut event, wakeup)| {
                if let Some(timing) = event.metrics_mut().and_then(|m| m.timing.as_mut()) {
                    timing.received_ms = Some(pacing_now_ms());
                    timing.wakeup_requested_ms = wakeup.and_then(|w| w.get().copied());
                }
                let request = match &event {
                    Event::Completed { request, .. } | Event::Cancelled { request, .. } => {
                        Some(request)
                    }
                    Event::Failed { request, .. } => request.as_ref(),
                    Event::Catalogue(_) => None,
                };
                if let Some(request) = request {
                    // Cancellation may race with worker publication after its own cleanup.
                    self.cancelled
                        .lock()
                        .expect("cancellation lock")
                        .remove(&request.token);
                }
                event
            })
            .collect()
    }
}
fn body(response: reqwest::blocking::Response, limit: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    response
        .error_for_status()?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= limit, "HTTP body exceeds bound");
    Ok(bytes)
}
fn begin_response(
    client: &mut SharedClient,
    tid: &str,
    headers: &reqwest::header::HeaderMap,
) -> Result<ResponseReader> {
    let mut fields = Vec::new();
    for name in ["JPIP-tid", "JPIP-fsiz", "JPIP-roff", "JPIP-rsiz"] {
        for value in headers.get_all(name) {
            fields.push((name, value.to_str()?));
        }
    }
    client.begin_response_headers(tid, fields)
}

#[cfg(test)]
mod tests {
    use super::*;
    use emuella_viewer_source::{ClientLimits, Identity, Profile, jpip};
    use reqwest::header::{HeaderMap, HeaderValue};

    #[test]
    fn authored_python_catalogue_has_valid_native_identities() {
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/viewer-native-diagnostics.py");
        let output = std::process::Command::new("python3")
            .args(["-B", "-c", "import runpy,json,sys; print(json.dumps(runpy.run_path(sys.argv[1])['catalogue']()))"])
            .arg(script).output().unwrap();
        assert!(output.status.success());
        let manifests: Vec<Manifest> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(manifests.len(), 3);
        for manifest in manifests {
            manifest.validate().unwrap();
        }
    }

    #[test]
    fn immediate_worker_uses_real_receipt_reservations_and_upload_handoff() {
        use polyorama_core::{DemandPriority, RegionConsumerId, RegionDemand};
        use polyorama_runtime::{RegionalCompletion, RegionalRuntime, RegionalRuntimeLimits};
        use std::{io::Write, net::TcpListener};
        let job = crate::engine::diagnostic_tests::job();
        job.manifest.validate().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let server = format!("http://{}", listener.local_addr().unwrap());
        let catalogue = serde_json::to_vec(&vec![job.manifest.clone()]).unwrap();
        let fixture = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = [0; 4096];
            let n = socket.read(&mut request).unwrap();
            assert!(
                std::str::from_utf8(&request[..n])
                    .unwrap()
                    .starts_with("GET /catalogue ")
            );
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                catalogue.len()
            )
            .unwrap();
            socket.write_all(&catalogue).unwrap();
        });
        let executor = Executor::new_diagnostic(
            server,
            64 << 20,
            egui::Context::default(),
            crate::DiagnosticOptions {
                enabled: true,
                authored_immediate: true,
            },
        );
        let receive = || {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let events = executor.drain();
                if !events.is_empty() {
                    return events;
                }
                assert!(
                    Instant::now() < deadline,
                    "authored executor did not publish"
                );
                std::thread::yield_now();
            }
        };
        assert!(matches!(&receive()[0], Event::Catalogue(_)));
        fixture.join().unwrap(); // No descriptor/sample server exists: completion must be authored.
        let mut runtime = RegionalRuntime::new(RegionalRuntimeLimits {
            max_demands: 2,
            max_in_flight: 1,
            decoded_bytes: job.request.max_decoded_bytes,
        });
        let first = RegionDemand {
            consumer: RegionConsumerId(1),
            key: job.request.key.clone(),
            priority: DemandPriority::Visible,
            max_decoded_bytes: job.request.max_decoded_bytes,
        };
        let mut second = first.clone();
        second.key.region.x = 512;
        second.consumer = RegionConsumerId(2);
        runtime.reconcile(1, [first, second]).unwrap();
        let request = runtime.dispatch().remove(0);
        assert!(runtime.dispatch().is_empty());
        executor
            .submit(Job {
                request: request.clone(),
                manifest: job.manifest,
            })
            .unwrap();
        let Event::Completed {
            request: received,
            pixels,
            metrics,
        } = receive().remove(0)
        else {
            panic!("expected real completion event")
        };
        assert_eq!(received, request);
        assert_eq!(runtime.metrics().in_flight, 1); // Publication did not release the UI reservation.
        let timing = metrics.timing.unwrap();
        assert!(
            timing.published_ms >= timing.finished_ms
                && timing.received_ms.unwrap() >= timing.published_ms
        );
        assert_eq!(
            runtime.complete_frame(&received, pixels),
            RegionalCompletion::Accepted
        );
        assert!(runtime.dispatch().is_empty());
        let allocation = runtime.allocation_diagnostics();
        assert_eq!(allocation.decoded_payloads, 1);
        assert_eq!(allocation.sample_capacity_bytes, 512 * 512 * 3 * 2);
        let upload = runtime.take_decoded_frame().unwrap();
        assert!(runtime.dispatch().is_empty());
        assert_eq!(runtime.allocation_diagnostics().sample_capacity_bytes, 0);
        assert_eq!(
            runtime.allocation_diagnostics().upload_bytes,
            512 * 512 * 3 * 2
        );
        drop(upload);
        assert!(runtime.finish_upload(&received.key, received.token, true));
        assert_eq!(runtime.dispatch().len(), 1);
    }

    fn client() -> (SharedClient, String) {
        let mut manifest = Manifest {
            target: "response-test".into(),
            tid: String::new(),
            identity: Identity {
                validity: None,
                source_sha256: "test-source".into(),
                bands: vec![0],
                profile: Profile {
                    width: 256,
                    height: 256,
                    tile_edge: 256,
                    decomposition_levels: 2,
                    bits_per_sample: 8,
                    components: 1,
                    bits_per_pixel: 2.5,
                },
                codec_revision: "test".into(),
                encoding_contract: "test".into(),
                spatial_policy_sha256: "test".into(),
                payload_sha256: "test".into(),
                descriptor_format: "test".into(),
            },
            encoded_bytes: 1024,
            main_header_bytes: 128,
            descriptor_sha256: vec!["test".into()],
        };
        manifest.seal().unwrap();
        let tid = manifest.tid.clone();
        let mut client = SharedClient::new(ClientLimits::default());
        client.register(manifest).unwrap();
        (client, tid)
    }
    fn headers(tid: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in [
            ("jPiP-tId", tid),
            ("JpIp-FsIz", "128,128"),
            ("jPIP-roFF", "3,4"),
            ("JPIP-rsiZ", "100,101"),
        ] {
            headers.append(
                reqwest::header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        headers
    }
    fn payload() -> Vec<u8> {
        let mut bytes = jpip::encode_message(jpip::DataMessage {
            key: jpip::BinKey::new(0, 1).unwrap(),
            offset: 0,
            final_bin: true,
            auxiliary: None,
            bytes: &[7, 8, 9],
        })
        .unwrap();
        bytes.extend(jpip::encode_end(2));
        bytes
    }
    fn rejects_without_admission(client: &mut SharedClient, tid: &str, headers: &HeaderMap) {
        let before = client.resident_bytes();
        let received = client.metrics.received_jpp_bytes;
        let result = begin_response(client, tid, headers);
        assert!(result.is_err(), "invalid headers created a response reader");
        assert_eq!(client.resident_bytes(), before);
        assert_eq!(client.metrics.received_jpp_bytes, received);
    }
    #[test]
    fn response_headers_reject_conflicting_occurrences_before_admission() {
        let (mut client, tid) = client();
        for (name, value) in [
            ("jpip-tid", "another-target"),
            ("jpip-fsiz", "256,256"),
            ("jpip-roff", "0,0"),
            ("jpip-rsiz", "1,1"),
        ] {
            let mut fields = headers(&tid);
            fields.append(name, HeaderValue::from_str(value).unwrap());
            rejects_without_admission(&mut client, &tid, &fields);
        }
    }
    #[test]
    fn response_headers_reject_malformed_or_browser_combined_geometry_before_admission() {
        let (mut client, tid) = client();
        for name in ["jpip-fsiz", "jpip-roff", "jpip-rsiz"] {
            for value in [
                "128,128,256,256",
                "128,128, 256,256",
                "128,128,round-down",
                "128",
                "128,",
                "128,x",
                "-1,1",
                "1.5,1",
                "4294967296,1",
                "1, 1",
            ] {
                let mut fields = headers(&tid);
                fields.insert(name, HeaderValue::from_str(value).unwrap());
                rejects_without_admission(&mut client, &tid, &fields);
            }
        }
        for (name, value) in [
            ("jpip-fsiz", "0,128"),
            ("jpip-rsiz", "0,1"),
            ("jpip-roff", "4294967295,4"),
            ("jpip-rsiz", "128,128"),
        ] {
            let mut fields = headers(&tid);
            fields.insert(name, HeaderValue::from_str(value).unwrap());
            rejects_without_admission(&mut client, &tid, &fields);
        }
    }
    #[test]
    fn response_headers_reject_missing_non_ascii_and_changed_identity_before_admission() {
        let (mut client, tid) = client();
        for name in ["jpip-tid", "jpip-fsiz", "jpip-roff", "jpip-rsiz"] {
            let mut fields = headers(&tid);
            fields.remove(name);
            rejects_without_admission(&mut client, &tid, &fields);
            fields.insert(name, HeaderValue::from_bytes(&[0xff]).unwrap());
            rejects_without_admission(&mut client, &tid, &fields);
            let mut fields = headers(&tid);
            fields.append(name, HeaderValue::from_bytes(&[0xff]).unwrap());
            rejects_without_admission(&mut client, &tid, &fields);
        }
        let mut fields = headers(&tid);
        fields.insert("jpip-tid", HeaderValue::from_static("another-target"));
        rejects_without_admission(&mut client, &tid, &fields);
    }
    #[test]
    fn response_headers_accept_case_insensitive_names_identical_duplicates_and_effective_window() {
        let (mut client, tid) = client();
        let mut headers = headers(&tid);
        headers.append("jpip-fsiz", HeaderValue::from_static("128,128"));
        let fields = emuella_viewer_source::response_fields(
            headers
                .iter()
                .map(|(name, value)| (name.as_str(), value.to_str().unwrap())),
        )
        .unwrap();
        assert_eq!(fields.frame, [128, 128]);
        assert_eq!(fields.offset, [3, 4]);
        assert_eq!(fields.size, [100, 101]);
        let mut reader = begin_response(&mut client, &tid, &headers).unwrap();
        let bytes = payload();
        client.receive(&mut reader, &bytes).unwrap();
        client.finish(reader).unwrap();
        assert_eq!(client.resident_bytes().0, 3);
        assert_eq!(client.metrics.received_jpp_bytes, bytes.len() as u64);
        headers.append("jpip-tid", HeaderValue::from_static("another-target"));
        rejects_without_admission(&mut client, &tid, &headers);
    }
}
