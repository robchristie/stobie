//! Repeatable real HTTP client journey with explicit cache-state labels.
use anyhow::{Context, Result, ensure};
use emuella_viewer_source::*;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::TcpStream,
    time::{Duration, Instant},
};
#[derive(Serialize)]
pub struct Measurement {
    pub stages: Vec<Stage>,
    pub client: ClientMetrics,
    pub response_network_bytes: u64,
    pub request_network_bytes: u64,
    pub compressed_resident_bytes: usize,
    pub descriptor_resident_bytes: usize,
    pub peak_rss_kib: Option<u64>,
    pub source_cache_state: String,
    pub physical_storage_bytes: Option<u64>,
}
#[derive(Serialize)]
pub struct Stage {
    pub state: String,
    pub elapsed_ms: f64,
    pub data_ready_ms: f64,
    pub decode_ms: f64,
    pub requests: u64,
    pub received_bytes: u64,
    pub decoded_width: u32,
    pub decoded_height: u32,
    pub decoded_sha256: String,
}
struct Wire {
    address: String,
    received: u64,
    sent: u64,
}
impl Wire {
    fn get(&mut self, path: &str) -> Result<(BTreeMap<String, String>, Vec<u8>)> {
        let mut stream = TcpStream::connect(&self.address)?;
        ensure!(
            stream.peer_addr()?.ip().is_loopback(),
            "measurement target must be localhost"
        );
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        let request = format!(
            "GET {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            self.address
        );
        stream.write_all(request.as_bytes())?;
        self.sent += request.len() as u64;
        let mut bytes = Vec::new();
        stream.take(16 << 20).read_to_end(&mut bytes)?;
        self.received += bytes.len() as u64;
        let split = bytes
            .windows(4)
            .position(|p| p == b"\r\n\r\n")
            .context("HTTP header terminator")?;
        let text = std::str::from_utf8(&bytes[..split])?;
        ensure!(
            text.starts_with("HTTP/1.1 200 "),
            "HTTP request failed: {text}"
        );
        let mut headers = BTreeMap::new();
        for line in text.lines().skip(1) {
            let (name, value) = line.split_once(':').context("HTTP header")?;
            let key = name.to_ascii_lowercase();
            let value = value.trim().to_string();
            ensure!(
                headers.get(&key).is_none_or(|v| v == &value),
                "conflicting duplicate response header"
            );
            headers.insert(key, value);
        }
        let body = bytes[split + 4..].to_vec();
        ensure!(
            headers
                .get("content-length")
                .context("content length")?
                .parse::<usize>()?
                == body.len(),
            "HTTP truncated body"
        );
        Ok((headers, body))
    }
}
pub fn measure(
    address: &str,
    target: &str,
    region: Region,
    rounds: usize,
    response_len: u64,
) -> Result<Measurement> {
    ensure!((1..=20).contains(&rounds), "round count limit");
    let mut wire = Wire {
        address: address.into(),
        received: 0,
        sent: 0,
    };
    let (_, bytes) = wire.get("/catalogue")?;
    let catalogue: Vec<Manifest> = serde_json::from_slice(&bytes)?;
    let manifest = catalogue
        .into_iter()
        .find(|m| m.target == target)
        .context("target absent")?;
    let mut client = SharedClient::new(ClientLimits::default());
    client.register(manifest.clone())?;
    let mut stages = Vec::new();
    for round in 0..rounds {
        let start = Instant::now();
        let before = wire.received;
        let mut requests = 0;
        for tile in client.missing_tiles(&manifest.tid, &region)? {
            let (_, bytes) = wire.get(&format!(
                "/descriptor/{}/{tile}?tid={}",
                manifest.target, manifest.tid
            ))?;
            requests += 1;
            client.install_descriptor(&manifest.tid, tile, &bytes)?;
        }
        for tile in client.missing_masks(&manifest.tid, &region)? {
            let (_, bytes) = wire.get(&format!(
                "/mask/{}/{}/{tile}?tid={}",
                manifest.target, region.discard, manifest.tid
            ))?;
            requests += 1;
            client.install_mask(&manifest.tid, tile, region.discard, &bytes)?;
        }
        ensure!(
            client.missing_masks(&manifest.tid, &region)?.is_empty(),
            "regional masks exceed admitted budget"
        );
        loop {
            if client.ready(&manifest.tid, &region)? {
                break;
            }
            ensure!(requests < 10_000, "request convergence limit");
            let request = client.request(&manifest.tid, &region, response_len)?;
            let (headers, bytes) = wire.get(&format!("/jpip?{}", checked(request.query())?))?;
            requests += 1;
            let field = |name: &str| {
                headers
                    .get(name)
                    .map(String::as_str)
                    .context("missing JPIP response field")
            };
            let fields = checked(jpip::ResponseFields::parse(
                field("jpip-tid")?,
                field("jpip-fsiz")?,
                field("jpip-roff")?,
                field("jpip-rsiz")?,
            ))?;
            let mut reader = client.begin_response(&manifest.tid, &fields)?;
            for fragment in bytes.chunks(4093) {
                client.receive(&mut reader, fragment)?;
            }
            client.finish(reader)?;
        }
        let data_ready_ms = start.elapsed().as_secs_f64() * 1000.;
        let decode_start = Instant::now();
        let decoded = client.decode(&manifest.tid, &region)?;
        let decode_ms = decode_start.elapsed().as_secs_f64() * 1000.;
        let mut planes = Vec::new();
        for plane in &decoded.planes {
            planes.extend_from_slice(plane);
        }
        stages.push(Stage {
            state: if round == 0 {
                "cold-compressed; server/OS cache uncontrolled"
            } else {
                "warm-compressed; fresh reconstruction"
            }
            .into(),
            elapsed_ms: start.elapsed().as_secs_f64() * 1000.,
            data_ready_ms,
            decode_ms,
            requests,
            received_bytes: wire.received - before,
            decoded_width: decoded.width,
            decoded_height: decoded.height,
            decoded_sha256: sha256(&planes),
        });
    }
    let (compressed, descriptors) = client.resident_bytes();
    Ok(Measurement {
        stages,
        client: client.metrics,
        response_network_bytes: wire.received,
        request_network_bytes: wire.sent,
        compressed_resident_bytes: compressed,
        descriptor_resident_bytes: descriptors,
        peak_rss_kib: crate::process_measurements().0,
        source_cache_state: "uncontrolled: no privileged OS cache reset performed".into(),
        physical_storage_bytes: None,
    })
}
