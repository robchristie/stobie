//! Local preparation and stateless HTTP assembly for the Emuella viewer.
use anyhow::{Context, Result, anyhow, ensure};
use emuella_viewer_source::{codec::ht_indexed::IndexedLossyHt, *};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    net::{IpAddr, Ipv6Addr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
pub mod gdal;
pub mod measure;
pub mod reference;

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct IoMetrics {
    pub logical_read_bytes: u64,
    pub logical_read_operations: u64,
    pub descriptor_bytes: u64,
    pub jpp_bytes: u64,
    pub elapsed_ms: f64,
    pub peak_rss_kib: Option<u64>,
    pub process_read_bytes: Option<u64>,
    pub source_hash_read_bytes: u64,
    pub source_hash_ms: f64,
    pub peak_tile_index_bytes: u64,
    pub descriptor_read_bytes: u64,
    pub descriptor_read_operations: u64,
    pub source_outer_driver: Option<String>,
    pub source_nitf_ic: Option<String>,
    pub source_index_required: Option<bool>,
    pub source_decoder_policy: Option<String>,
    pub process_rchar: Option<u64>,
    pub process_syscr: Option<u64>,
}
pub fn process_measurements() -> (Option<u64>, Option<u64>) {
    let rss = fs::read_to_string("/proc/self/status").ok().and_then(|s| {
        s.lines()
            .find(|l| l.starts_with("VmHWM:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|s| s.parse().ok())
    });
    let read = fs::read_to_string("/proc/self/io").ok().and_then(|s| {
        s.lines()
            .find(|l| l.starts_with("read_bytes:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|s| s.parse().ok())
    });
    (rss, read)
}
pub fn process_io() -> (Option<u64>, Option<u64>, Option<u64>) {
    let text = fs::read_to_string("/proc/self/io").ok();
    let field = |name: &str| {
        text.as_ref()
            .and_then(|s| {
                s.lines()
                    .filter_map(|l| l.split_once(':'))
                    .find(|(key, _)| *key == name)
            })
            .and_then(|(_, value)| value.trim().parse().ok())
    };
    (field("read_bytes"), field("rchar"), field("syscr"))
}
pub fn hash_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut bytes = vec![0; 1 << 20];
    loop {
        let n = file.read(&mut bytes)?;
        if n == 0 {
            break;
        }
        hash.update(&bytes[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
/// Single-threaded synchronous pipeline: source window -> tile encoder -> file.
/// A directory rename publishes the manifest, descriptors and payload together.
pub fn prepare(
    output: &Path,
    target: &str,
    mut identity: Identity,
    mut read_tile: impl FnMut(codec::TileRect, &mut [Vec<u8>]) -> Result<()>,
) -> Result<(Manifest, IoMetrics)> {
    ensure!(
        !output.exists(),
        "output exists; representations are immutable"
    );
    let parent = output.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".{}-{}.incomplete",
        output
            .file_name()
            .context("output basename")?
            .to_string_lossy(),
        std::process::id()
    ));
    fs::create_dir(&temporary)?;
    let result = (|| {
        let start = Instant::now();
        let before = process_measurements().1;
        let mut file = File::create(temporary.join("payload.j2c"))?;
        let mut hash = Sha256::new();
        let mut source_error = None;
        let mut output_error = None;
        let mut descriptor_error = None;
        let mut metrics = IoMetrics::default();
        fs::create_dir(temporary.join("descriptors"))?;
        let mut hashes = Vec::new();
        let index = checked(codec::ht_indexed::encode_tiled_to_descriptors(
            identity.profile.codec(),
            |rect, planes| {
                if let Err(e) = read_tile(rect, planes) {
                    source_error = Some(e);
                    return Err(codec::CodestreamError::SizeOverflow);
                }
                metrics.logical_read_operations += planes.len() as u64;
                metrics.logical_read_bytes += planes.iter().map(|p| p.len() as u64).sum::<u64>();
                Ok(())
            },
            |bytes| {
                if let Err(e) = file.write_all(bytes) {
                    output_error = Some(e);
                    return Err(codec::CodestreamError::SizeOverflow);
                }
                hash.update(bytes);
                Ok(())
            },
            |tile, bytes| {
                if let Err(e) = fs::write(
                    temporary.join("descriptors").join(format!("{tile}.bin")),
                    bytes,
                ) {
                    descriptor_error = Some(e);
                    return Err(codec::CodestreamError::SizeOverflow);
                }
                hashes.push(sha256(bytes));
                Ok(())
            },
        ));
        if let Some(e) = source_error {
            return Err(e);
        }
        if let Some(e) = output_error {
            return Err(e.into());
        }
        if let Some(e) = descriptor_error {
            return Err(e.into());
        }
        let index = index?;
        file.sync_all()?;
        identity.payload_sha256 = format!("{:x}", hash.finalize());
        metrics.descriptor_bytes = index.descriptor_bytes;
        metrics.peak_tile_index_bytes = index.peak_tile_index_bytes;
        let mut manifest = Manifest {
            target: target.into(),
            tid: String::new(),
            identity,
            encoded_bytes: index.encoded_bytes,
            main_header_bytes: index.main_header.end,
            descriptor_sha256: hashes,
        };
        manifest.seal()?;
        manifest.validate()?;
        fs::write(
            temporary.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest)?,
        )?;
        metrics.elapsed_ms = start.elapsed().as_secs_f64() * 1000.;
        let (rss, after) = process_measurements();
        metrics.peak_rss_kib = rss;
        metrics.process_read_bytes = after.zip(before).map(|(a, b)| a.saturating_sub(b));
        fs::write(
            temporary.join("preparation.json"),
            serde_json::to_vec_pretty(&metrics)?,
        )?;
        fs::rename(&temporary, output)?;
        Ok((manifest, metrics))
    })();
    if result.is_err() {
        fs::remove_dir_all(&temporary).context("remove own incomplete preparation")?;
    }
    result
}
/// Public project-authored deterministic scientific pattern; no source imagery.
pub fn synthetic_tile(p: &Profile, rect: codec::TileRect, planes: &mut [Vec<u8>]) -> Result<()> {
    let max = (1u32 << p.bits_per_sample) - 1;
    for (c, plane) in planes.iter_mut().enumerate() {
        for y in 0..rect.height {
            for x in 0..rect.width {
                let gx = x + rect.x;
                let gy = y + rect.y;
                let background =
                    (gx.wrapping_mul(13) + gy.wrapping_mul(7) + c as u32 * 719) % max.max(1);
                let mut value = max / 8 + background / 2;
                if (gx % 257).abs_diff(123) < 3 && (gy % 263).abs_diff(111) < 2 {
                    value = max;
                }
                if gx.is_multiple_of(509) || gy.is_multiple_of(503) {
                    value = max * 3 / 4;
                }
                if gx % 97 < 5 && gy % 89 < 5 {
                    value = (value + max / 100).min(max);
                }
                let i = (y * rect.width + x) as usize * usize::from(p.bits_per_sample.div_ceil(8));
                if p.bits_per_sample == 8 {
                    plane[i] = value as u8;
                } else {
                    plane[i..i + 2].copy_from_slice(&(value as u16).to_le_bytes());
                }
            }
        }
    }
    Ok(())
}
pub fn fixture(
    output: &Path,
    target: &str,
    profile: Profile,
    revision: &str,
) -> Result<(Manifest, IoMetrics)> {
    let identity = Identity {
        source_sha256: sha256(
            format!(
                "emuella-scientific-pattern-v1:{}:{}:{}:{}",
                profile.width, profile.height, profile.components, profile.bits_per_sample
            )
            .as_bytes(),
        ),
        bands: (1..=profile.components).collect(),
        profile: profile.clone(),
        codec_revision: revision.into(),
        encoding_contract: "indexed-htonly-no-mct-irreversible97-one-layer-rate-search-v1".into(),
        spatial_policy_sha256: sha256(b"uniform-v1"),
        payload_sha256: String::new(),
        descriptor_format: "EHTIDX01".into(),
    };
    prepare(output, target, identity, |rect, planes| {
        synthetic_tile(&profile, rect, planes)
    })
}
struct Representation {
    manifest: Manifest,
    root: PathBuf,
}
pub struct Service {
    representations: BTreeMap<String, Representation>,
    web: Option<PathBuf>,
    pub metrics: IoMetrics,
}
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}
impl HttpResponse {
    fn new(status: u16, mime: &str, body: Vec<u8>) -> Self {
        Self {
            status,
            headers: vec![("Content-Type".into(), mime.into())],
            body,
        }
    }
}

/// Parse the deliberately narrow HTTP authority used by this loopback service.
/// Names never undergo DNS resolution, including names that resolve to loopback.
fn loopback_authority(value: &str) -> Result<(String, u16)> {
    let (host, port) = if let Some(rest) = value.strip_prefix('[') {
        let (host, suffix) = rest.split_once(']').context("invalid IPv6 authority")?;
        let address: Ipv6Addr = host.parse().context("invalid IPv6 address")?;
        ensure!(address.is_loopback(), "loopback Host required");
        (address.to_string(), suffix)
    } else {
        let (host, suffix) = value
            .split_once(':')
            .map_or((value, ""), |(host, _)| (host, &value[host.len()..]));
        let host = if host.eq_ignore_ascii_case("localhost") {
            "localhost".to_owned()
        } else {
            let address: IpAddr = host.parse().context("invalid loopback Host")?;
            ensure!(
                address.is_ipv4() && address.is_loopback(),
                "loopback Host required"
            );
            address.to_string()
        };
        (host, suffix)
    };
    let port = if port.is_empty() {
        80
    } else {
        let port = port.strip_prefix(':').context("invalid authority suffix")?;
        ensure!(
            !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()),
            "invalid authority port"
        );
        let port: u16 = port.parse().context("invalid authority port")?;
        ensure!(port != 0, "invalid authority port");
        port
    };
    Ok((host, port))
}

fn local_request_target(header: &[u8]) -> Result<&str> {
    let text = std::str::from_utf8(header)?;
    let mut lines = text.split("\r\n");
    let mut parts = lines.next().context("request line")?.split(' ');
    ensure!(parts.next() == Some("GET"), "GET required");
    let url = parts.next().context("URL")?;
    ensure!(
        url.starts_with('/') && !url.starts_with("//") && !url.chars().any(char::is_control),
        "origin-form URL required"
    );
    ensure!(
        parts.next() == Some("HTTP/1.1") && parts.next().is_none(),
        "HTTP version"
    );
    let mut host = None;
    let mut origin = None;
    let mut fetch_site = None;
    for line in lines.take_while(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':').context("invalid HTTP field")?;
        ensure!(
            !name.is_empty()
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
                && value
                    .bytes()
                    .all(|byte| byte == b'\t' || (32..=126).contains(&byte)),
            "invalid HTTP field"
        );
        let value = value.trim_matches([' ', '\t']);
        let field = if name.eq_ignore_ascii_case("Host") {
            &mut host
        } else if name.eq_ignore_ascii_case("Origin") {
            &mut origin
        } else if name.eq_ignore_ascii_case("Sec-Fetch-Site") {
            &mut fetch_site
        } else {
            continue;
        };
        ensure!(field.replace(value).is_none(), "duplicate boundary field");
    }
    let host = loopback_authority(host.context("Host required")?)?;
    if let Some(origin) = origin {
        let authority = origin
            .strip_prefix("http://")
            .context("HTTP Origin required")?;
        ensure!(
            loopback_authority(authority)? == host,
            "same-origin request required"
        );
    }
    if let Some(site) = fetch_site {
        ensure!(
            matches!(site, "same-origin" | "none"),
            "same-origin fetch required"
        );
    }
    Ok(url)
}

struct FileBins {
    file: File,
    ranges: BTreeMap<jpip::BinKey, std::ops::Range<u64>>,
    bytes: u64,
    operations: u64,
}
impl jpip::BinSource for FileBins {
    fn length(&self, key: jpip::BinKey) -> std::result::Result<u64, jpip::Error> {
        self.ranges
            .get(&key)
            .map(|r| r.end - r.start)
            .ok_or(jpip::Error::Source)
    }
    fn read(
        &mut self,
        key: jpip::BinKey,
        offset: u64,
        out: &mut [u8],
    ) -> std::result::Result<(), jpip::Error> {
        let range = self.ranges.get(&key).ok_or(jpip::Error::Source)?;
        if offset
            .checked_add(out.len() as u64)
            .is_none_or(|n| n > range.end - range.start)
        {
            return Err(jpip::Error::Source);
        }
        self.file
            .seek(SeekFrom::Start(range.start + offset))
            .map_err(|_| jpip::Error::Source)?;
        self.file.read_exact(out).map_err(|_| jpip::Error::Source)?;
        self.bytes += out.len() as u64;
        self.operations += 1;
        Ok(())
    }
    fn completed_packets(&self, key: jpip::BinKey, prefix: u64) -> Option<u64> {
        (key.class == 0).then(|| {
            u64::from(
                self.ranges
                    .get(&key)
                    .is_some_and(|r| prefix >= r.end - r.start),
            )
        })
    }
}
impl Service {
    /// Explicit catalogue whitelist. Payload length is checked at restart, with an
    /// optional full integrity pass outside interactive request handling.
    pub fn open(roots: &[PathBuf], web: Option<PathBuf>, verify_payload: bool) -> Result<Self> {
        let mut representations = BTreeMap::new();
        for root in roots {
            let manifest: Manifest =
                serde_json::from_slice(&fs::read(root.join("manifest.json"))?)?;
            manifest.validate()?;
            ensure!(
                fs::metadata(root.join("payload.j2c"))?.len() == manifest.encoded_bytes,
                "payload length mismatch"
            );
            if verify_payload {
                ensure!(
                    hash_file(&root.join("payload.j2c"))? == manifest.identity.payload_sha256,
                    "payload digest mismatch"
                );
            }
            ensure!(
                !representations.contains_key(&manifest.target),
                "duplicate target"
            );
            representations.insert(
                manifest.target.clone(),
                Representation {
                    manifest,
                    root: root.clone(),
                },
            );
        }
        Ok(Self {
            representations,
            web,
            metrics: IoMetrics::default(),
        })
    }
    fn index(rep: &Representation, region: &Region) -> Result<(IndexedLossyHt, u64, u64)> {
        let mut index = rep.manifest.sparse()?;
        let mut read_bytes = 0;
        let mut reads = 0;
        let tiles = region.tiles(&rep.manifest.identity.profile)?;
        ensure!(tiles.len() <= 256, "request tile limit");
        for tile in tiles {
            let bytes = fs::read(rep.root.join("descriptors").join(format!("{tile}.bin")))?;
            ensure!(
                sha256(&bytes) == rep.manifest.descriptor_sha256[usize::from(tile)],
                "descriptor integrity"
            );
            checked(index.import_tile_descriptor(&bytes))?;
            read_bytes += bytes.len() as u64;
            reads += 1;
        }
        Ok((index, read_bytes, reads))
    }
    pub fn route(&mut self, url: &str) -> Result<HttpResponse> {
        ensure!(url.len() <= 65536, "request URL limit");
        if url == "/catalogue" {
            return Ok(HttpResponse::new(
                200,
                "application/json",
                serde_json::to_vec(
                    &self
                        .representations
                        .values()
                        .map(|r| &r.manifest)
                        .collect::<Vec<_>>(),
                )?,
            ));
        }
        if url == "/metrics" {
            let (rss, read) = process_measurements();
            self.metrics.peak_rss_kib = rss;
            self.metrics.process_read_bytes = read;
            return Ok(HttpResponse::new(
                200,
                "application/json",
                serde_json::to_vec(&self.metrics)?,
            ));
        }
        if let Some(target) = url.strip_prefix("/manifest/") {
            let rep = self.representations.get(target).context("unknown target")?;
            return Ok(HttpResponse::new(
                200,
                "application/json",
                serde_json::to_vec(&rep.manifest)?,
            ));
        }
        if let Some(path) = url.strip_prefix("/descriptor/") {
            let (path, tid) = path
                .split_once("?tid=")
                .context("descriptor identity required")?;
            let (target, tile) = path.split_once('/').context("descriptor route")?;
            let tile: usize = tile.parse()?;
            let rep = self.representations.get(target).context("unknown target")?;
            ensure!(rep.manifest.tid == tid, "stale target identity");
            let digest = rep
                .manifest
                .descriptor_sha256
                .get(tile)
                .context("unknown tile")?;
            let bytes = fs::read(rep.root.join("descriptors").join(format!("{tile}.bin")))?;
            ensure!(
                bytes.len() <= 1 << 20 && sha256(&bytes) == *digest,
                "descriptor integrity"
            );
            self.metrics.descriptor_bytes += bytes.len() as u64;
            self.metrics.descriptor_read_bytes += bytes.len() as u64;
            self.metrics.descriptor_read_operations += 1;
            return Ok(HttpResponse::new(200, "application/octet-stream", bytes));
        }
        if let Some(query) = url.strip_prefix("/jpip?") {
            let start = Instant::now();
            let request = checked(jpip::Request::parse(query, 65536, 4096))?;
            ensure!(
                (128..=8 << 20).contains(&request.max_length),
                "response len outside service limits"
            );
            let rep = self
                .representations
                .get(&request.target)
                .context("unknown target")?;
            let (region, fields) = effective_region(&rep.manifest, &request)?;
            let (index, descriptor_bytes, descriptor_reads) = Self::index(rep, &region)?;
            self.metrics.descriptor_read_bytes += descriptor_bytes;
            self.metrics.descriptor_read_operations += descriptor_reads;
            let demands = demands(&index, &region)?;
            let mut bins = FileBins {
                file: File::open(rep.root.join("payload.j2c"))?,
                ranges: bin_ranges(&index)?,
                bytes: 0,
                operations: 0,
            };
            let mut body = Vec::new();
            checked(jpip::deliver(
                &mut bins,
                &demands,
                &request.model_for_identity(&rep.manifest.tid),
                jpip::DeliveryLimits {
                    response_bytes: request.max_length,
                    message_bytes: 64 << 10,
                    messages: 65536,
                },
                request.extended,
                |bytes| {
                    body.extend_from_slice(bytes);
                    Ok(())
                },
            ))?;
            self.metrics.logical_read_bytes += bins.bytes;
            self.metrics.logical_read_operations += bins.operations;
            self.metrics.jpp_bytes += body.len() as u64;
            self.metrics.elapsed_ms += start.elapsed().as_secs_f64() * 1000.;
            let mut response = HttpResponse::new(200, "image/jpp-stream", body);
            response.headers.extend(checked(fields.headers())?);
            return Ok(response);
        }
        if let Some(root) = &self.web {
            let path = if url == "/" {
                "index.html"
            } else {
                url.trim_start_matches('/')
            };
            ensure!(
                path.split('/')
                    .all(|p| !p.is_empty() && p != "." && p != "..")
                    && !path.contains(['\\', '%', '?']),
                "invalid static path"
            );
            let file = root.join(path);
            let canonical = file.canonicalize()?;
            ensure!(
                canonical.starts_with(root.canonicalize()?),
                "static path escapes root"
            );
            ensure!(
                fs::metadata(&canonical)?.len() <= 64 << 20,
                "static asset limit"
            );
            let mime = match file.extension().and_then(|s| s.to_str()) {
                Some("wasm") => "application/wasm",
                Some("js") => "text/javascript",
                Some("css") => "text/css",
                Some("html") => "text/html",
                _ => "application/octet-stream",
            };
            return Ok(HttpResponse::new(200, mime, fs::read(canonical)?));
        }
        Err(anyhow!("unknown route"))
    }
    pub fn handle(&mut self, mut stream: TcpStream) -> Result<()> {
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        let mut header = Vec::new();
        let mut byte = [0];
        while !header.ends_with(b"\r\n\r\n") {
            ensure!(header.len() < 65536, "HTTP header limit");
            stream.read_exact(&mut byte)?;
            header.push(byte[0]);
        }
        let response = local_request_target(&header)
            .and_then(|url| self.route(url))
            .unwrap_or_else(|error| {
                HttpResponse::new(400, "text/plain", format!("{error:#}\n").into_bytes())
            });
        write!(
            stream,
            "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\nCross-Origin-Resource-Policy: same-origin\r\nX-Content-Type-Options: nosniff\r\n",
            response.status,
            if response.status == 200 {
                "OK"
            } else {
                "Bad Request"
            },
            response.body.len()
        )?;
        for (name, value) in response.headers {
            write!(stream, "{name}: {value}\r\n")?;
        }
        write!(stream, "\r\n")?;
        stream.write_all(&response.body)?;
        Ok(())
    }
    pub fn serve(&mut self, listener: TcpListener) -> Result<()> {
        ensure!(
            listener.local_addr()?.ip().is_loopback(),
            "local service binds loopback only"
        );
        for stream in listener.incoming() {
            if let Err(e) = self.handle(stream?) {
                eprintln!("request: {e:#}");
            }
        }
        Ok(())
    }
}
