//! Complete selected-tile reconstruction from the immutable file, then crop.
//! This verifies application composition, not an independent codec algorithm.
use anyhow::{Context, Result, ensure};
use emuella_viewer_source::{Manifest, Region, checked, codec, sha256};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Read, path::Path, time::Instant};

const WORKSPACE_BYTES: u64 = 64 << 20;
const OUTPUT_BYTES: u64 = 16 << 20;
const METADATA_BYTES: u64 = 16 << 20;

/// Wire-compatible with the application's `DecodedEvidence` records.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Request {
    pub tid: String,
    pub region: Region,
    pub width: u32,
    pub height: u32,
    pub precision: u8,
    pub fnv1a64_u16le: String,
}
#[derive(Default, Debug, Serialize)]
pub struct Metrics {
    pub direct_file_bytes: u64,
    pub direct_file_operations: u64,
    pub descriptor_bytes: u64,
    pub descriptor_operations: u64,
    pub full_tile_decodes: u64,
    pub full_tile_pixels: u64,
    pub selected_code_blocks: u64,
    pub selected_block_coefficients: u64,
    pub synthesis_coefficients_loaded: u64,
    pub synthesis_horizontal_values: u64,
    pub synthesis_vertical_values: u64,
    pub synthesis_lifting_updates: u64,
    pub synthesis_output_samples: u64,
    pub peak_codec_workspace_bytes: u64,
    pub peak_output_bytes: u64,
    pub peak_tile_plane_bytes: u64,
    pub peak_descriptor_and_index_bytes: u64,
}
#[derive(Debug, Serialize)]
pub struct Comparison {
    pub observed: Request,
    pub reference_fnv1a64_u16le: String,
    pub reference_width: u32,
    pub reference_height: u32,
    pub matches: bool,
}
#[derive(Debug, Serialize)]
pub struct Report {
    pub target: String,
    pub tid: String,
    pub codec_revision: String,
    pub method: &'static str,
    pub cost_boundary: &'static str,
    pub payload_integrity: &'static str,
    pub compared_pixels: u64,
    pub mismatched_records: u64,
    pub elapsed_ms: f64,
    pub metrics: Metrics,
    pub comparisons: Vec<Comparison>,
}
/// Native interleaved samples, bounded independently of source image dimensions.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub samples: Vec<u16>,
}
pub fn checksum(samples: &[u16]) -> String {
    let hash = samples
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
        });
    format!("{hash:016x}")
}
fn bounded_read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    ensure!(file.metadata()?.len() <= limit, "metadata exceeds limit");
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "metadata exceeds limit");
    Ok(bytes)
}

pub struct Reference {
    pub manifest: Manifest,
    root: std::path::PathBuf,
    payload: File,
}
impl Reference {
    pub fn open(root: &Path) -> Result<Self> {
        let manifest: Manifest =
            serde_json::from_slice(&bounded_read(&root.join("manifest.json"), METADATA_BYTES)?)?;
        manifest.validate()?;
        let payload = File::open(root.join("payload.j2c"))?;
        ensure!(
            payload.metadata()?.len() == manifest.encoded_bytes,
            "payload length mismatch"
        );
        Ok(Self {
            manifest,
            root: root.into(),
            payload,
        })
    }
    /// Admit and reconstruct only one complete clipped tile at a time. File reads
    /// use absolute codestream offsets and never pass through the JPP cache.
    pub fn decode(&self, region: &Region, metrics: &mut Metrics) -> Result<Frame> {
        let p = &self.manifest.identity.profile;
        region.validate(p)?;
        let scale = 1u32 << region.discard;
        ensure!(
            p.tile_edge.is_multiple_of(scale),
            "tile origin must align to resolution"
        );
        let x0 = region.x.div_ceil(scale);
        let y0 = region.y.div_ceil(scale);
        let x1 = (region.x + region.width).div_ceil(scale);
        let y1 = (region.y + region.height).div_ceil(scale);
        let width = x1 - x0;
        let height = y1 - y0;
        let components = region.components.len();
        let output_bytes = u64::from(width) * u64::from(height) * components as u64 * 2;
        ensure!(
            output_bytes <= OUTPUT_BYTES,
            "reference output exceeds 16 MiB"
        );
        metrics.peak_output_bytes = metrics.peak_output_bytes.max(output_bytes);
        let mut samples = vec![0u16; output_bytes as usize / 2];
        let columns = p.width.div_ceil(p.tile_edge);
        let sample_bytes = usize::from(p.bits_per_sample.div_ceil(8));
        for tile in region.tiles(p)? {
            let tx = u32::from(tile) % columns * p.tile_edge;
            let ty = u32::from(tile) / columns * p.tile_edge;
            let full_tile = codec::TileRegionRequest {
                x: tx,
                y: ty,
                width: p.tile_edge.min(p.width - tx),
                height: p.tile_edge.min(p.height - ty),
            };
            let descriptor = bounded_read(
                &self.root.join("descriptors").join(format!("{tile}.bin")),
                METADATA_BYTES,
            )?;
            ensure!(
                self.manifest.descriptor_sha256.get(usize::from(tile))
                    == Some(&sha256(&descriptor)),
                "descriptor digest mismatch"
            );
            metrics.descriptor_bytes += descriptor.len() as u64;
            metrics.descriptor_operations += 1;
            let mut index = self.manifest.sparse()?;
            checked(index.import_tile_descriptor(&descriptor))?;
            ensure!(
                index.precincts().iter().all(|v| v.tile == tile),
                "wrong descriptor tile"
            );
            let descriptor_and_index = descriptor.len() as u64 + index.retained_heap_bytes();
            ensure!(
                descriptor_and_index <= METADATA_BYTES,
                "descriptor/index exceeds 16 MiB"
            );
            metrics.peak_descriptor_and_index_bytes = metrics
                .peak_descriptor_and_index_bytes
                .max(descriptor_and_index);
            let plan = checked(index.plan(full_tile, region.discard, &region.components))?;
            let output = plan.output_region();
            let mut workspace =
                codec::ht_lossy::LossyHtSpatialRegionWorkspace::with_maximum_bytes(WORKSPACE_BYTES);
            let mut read_error = None;
            let decoded = plan.decode_with_report(
                |offset, out| {
                    #[cfg(unix)]
                    let result =
                        std::os::unix::fs::FileExt::read_exact_at(&self.payload, out, offset);
                    #[cfg(not(unix))]
                    let result = {
                        use std::io::{Seek, SeekFrom};
                        self.payload.try_clone().and_then(|mut file| {
                            file.seek(SeekFrom::Start(offset))?;
                            file.read_exact(out)
                        })
                    };
                    result.map_err(|e| {
                        read_error = Some(e);
                        codec::CodestreamError::SizeOverflow
                    })?;
                    metrics.direct_file_bytes += out.len() as u64;
                    metrics.direct_file_operations += 1;
                    Ok(())
                },
                &mut workspace,
            );
            if let Some(error) = read_error {
                return Err(error.into());
            }
            let (planes, report) = checked(decoded)?;
            ensure!(
                planes.len() == components
                    && planes.iter().all(|plane| plane.len()
                        == output.width as usize * output.height as usize * sample_bytes),
                "invalid tile output"
            );
            metrics.full_tile_decodes += 1;
            metrics.full_tile_pixels += u64::from(output.width) * u64::from(output.height);
            metrics.selected_code_blocks += plan.selected_code_blocks() as u64;
            metrics.selected_block_coefficients += plan.selected_block_coefficients();
            metrics.peak_codec_workspace_bytes = metrics
                .peak_codec_workspace_bytes
                .max(plan.required_workspace_bytes());
            metrics.peak_tile_plane_bytes = metrics
                .peak_tile_plane_bytes
                .max(planes.iter().map(|v| v.len() as u64).sum());
            metrics.synthesis_coefficients_loaded += report.work.coefficients_loaded;
            metrics.synthesis_horizontal_values += report.work.horizontal_values;
            metrics.synthesis_vertical_values += report.work.vertical_values;
            metrics.synthesis_lifting_updates += report.work.lifting_updates;
            metrics.synthesis_output_samples += report.work.output_samples;
            let tile_x = tx / scale;
            let tile_y = ty / scale;
            for y in y0.max(tile_y)..y1.min(tile_y + output.height) {
                for x in x0.max(tile_x)..x1.min(tile_x + output.width) {
                    let source = ((y - tile_y) * output.width + x - tile_x) as usize;
                    let dest = ((y - y0) * width + x - x0) as usize * components;
                    for (component, plane) in planes.iter().enumerate() {
                        samples[dest + component] = if sample_bytes == 1 {
                            u16::from(plane[source])
                        } else {
                            u16::from_le_bytes([plane[source * 2], plane[source * 2 + 1]])
                        };
                    }
                }
            }
        }
        Ok(Frame {
            width,
            height,
            samples,
        })
    }
}

pub fn compare(root: &Path, requests: &[Request]) -> Result<Report> {
    ensure!(
        !requests.is_empty() && requests.len() <= 1024,
        "expected 1..1024 evidence records"
    );
    let start = Instant::now();
    let reference = Reference::open(root)?;
    let mut metrics = Metrics::default();
    let mut comparisons = Vec::new();
    let mut compared_pixels = 0;
    for request in requests {
        ensure!(
            request.tid == reference.manifest.tid,
            "evidence belongs to a different representation"
        );
        ensure!(
            request.fnv1a64_u16le.len() == 16
                && request.fnv1a64_u16le.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid FNV-1a checksum"
        );
        let frame = reference.decode(&request.region, &mut metrics)?;
        let hash = checksum(&frame.samples);
        let matches = request.width == frame.width
            && request.height == frame.height
            && request.precision == reference.manifest.identity.profile.bits_per_sample
            && request.fnv1a64_u16le.eq_ignore_ascii_case(&hash);
        compared_pixels += u64::from(frame.width) * u64::from(frame.height);
        comparisons.push(Comparison {
            observed: request.clone(),
            reference_fnv1a64_u16le: hash,
            reference_width: frame.width,
            reference_height: frame.height,
            matches,
        });
    }
    Ok(Report {
        target: reference.manifest.target,
        tid: reference.manifest.tid,
        codec_revision: reference.manifest.identity.codec_revision,
        method: "complete selected-tile direct-file decode, crop, native interleaved U16LE FNV-1a comparison; shared codec algorithm",
        cost_boundary: "reference decode cost only; not viewport performance metrics",
        payload_integrity: "immutable local store trusted after length check; selected descriptor SHA-256 verified; no whole-payload hash scan",
        compared_pixels,
        mismatched_records: comparisons.iter().filter(|v| !v.matches).count() as u64,
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.,
        metrics,
        comparisons,
    })
}
pub fn compare_file(root: &Path, requests: &Path) -> Result<Report> {
    let requests =
        serde_json::from_slice::<Vec<Request>>(&bounded_read(requests, METADATA_BYTES)?)?;
    compare(root, &requests)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HttpResponse, Service, fixture};
    use emuella_viewer_source::{ClientLimits, DecodedRegion, Profile, SharedClient, jpip};
    use std::{
        io::Write,
        net::{TcpListener, TcpStream},
    };

    fn request(service: &mut Service, path: &str, tcp: bool) -> HttpResponse {
        if !tcp {
            return service.route(path).unwrap();
        }
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::scope(|scope| {
            let server = scope.spawn(|| service.handle(listener.accept().unwrap().0).unwrap());
            let mut stream = TcpStream::connect(address).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(30)))
                .unwrap();
            write!(
                stream,
                "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).unwrap();
            server.join().unwrap();
            let split = bytes.windows(4).position(|v| v == b"\r\n\r\n").unwrap();
            let header = std::str::from_utf8(&bytes[..split]).unwrap();
            assert!(header.starts_with("HTTP/1.1 200 "));
            HttpResponse {
                status: 200,
                headers: header
                    .lines()
                    .skip(1)
                    .map(|line| {
                        let (key, value) = line.split_once(':').unwrap();
                        (key.into(), value.trim().into())
                    })
                    .collect(),
                body: bytes[split + 4..].to_vec(),
            }
        })
    }
    fn client_decode(
        service: &mut Service,
        client: &mut SharedClient,
        manifest: &Manifest,
        region: &Region,
        tcp: bool,
    ) -> DecodedRegion {
        for tile in client.missing_tiles(&manifest.tid, region).unwrap() {
            let response = request(
                service,
                &format!(
                    "/descriptor/{}/{tile}?tid={}",
                    manifest.target, manifest.tid
                ),
                tcp,
            );
            client
                .install_descriptor(&manifest.tid, tile, &response.body)
                .unwrap();
        }
        for attempt in 0..100 {
            if client.ready(&manifest.tid, region).unwrap() {
                break;
            }
            assert!(attempt < 99, "HTTP client did not converge");
            let query = checked(
                client
                    .request(&manifest.tid, region, 1 << 20)
                    .unwrap()
                    .query(),
            )
            .unwrap();
            let response = request(service, &format!("/jpip?{query}"), tcp);
            let get = |name: &str| {
                response
                    .headers
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case(name))
                    .unwrap()
                    .1
                    .as_str()
            };
            let fields = checked(jpip::ResponseFields::parse(
                get("JPIP-tid"),
                get("JPIP-fsiz"),
                get("JPIP-roff"),
                get("JPIP-rsiz"),
            ))
            .unwrap();
            let mut reader = client.begin_response(&manifest.tid, &fields).unwrap();
            for fragment in response.body.chunks(4093) {
                client.receive(&mut reader, fragment).unwrap();
            }
            client.finish(reader).unwrap();
        }
        client.decode(&manifest.tid, region).unwrap()
    }
    // Convert the independently reconstructed cached region using the same
    // public native-plane contract that the application's Engine consumes.
    fn client_samples(decoded: &DecodedRegion) -> Vec<u16> {
        let planes: Vec<Vec<u16>> = decoded
            .planes
            .iter()
            .map(|plane| {
                if decoded.bits_per_sample == 8 {
                    plane.iter().map(|&v| u16::from(v)).collect()
                } else {
                    plane
                        .chunks_exact(2)
                        .map(|v| u16::from_le_bytes([v[0], v[1]]))
                        .collect()
                }
            })
            .collect();
        (0..decoded.width as usize * decoded.height as usize)
            .flat_map(|pixel| planes.iter().map(move |plane| plane[pixel]))
            .collect()
    }
    #[test]
    fn complete_tiles_match_cached_regions_all_formats_windows_and_seven_levels() {
        let temp = tempfile::tempdir().unwrap();
        for (bits, components) in [(8, 3), (11, 1), (16, 1), (16, 3)] {
            let root = temp.path().join(format!("{bits}-{components}"));
            let p = Profile {
                width: 529,
                height: 531,
                tile_edge: 256,
                decomposition_levels: 6,
                bits_per_sample: bits,
                components,
                bits_per_pixel: 3.,
            };
            let (manifest, _) = fixture(&root, "authored", p, "fixture-revision").unwrap();
            let reference = Reference::open(&root).unwrap();
            let mut service = Service::open(std::slice::from_ref(&root), None, true).unwrap();
            let mut client = SharedClient::new(ClientLimits::default());
            client.register(manifest.clone()).unwrap();
            let windows = [
                (0, 0, 65, 67),
                (3, 7, 129, 131),
                (247, 11, 81, 97),
                (11, 247, 97, 81),
                (247, 247, 83, 85),
                (440, 450, 89, 81),
                (256, 256, 129, 129),
                (461, 3, 68, 135),
            ];
            let mut records = Vec::new();
            for discard in 0..=6 {
                for (window, &(x, y, width, height)) in windows.iter().enumerate() {
                    let region = Region {
                        x,
                        y,
                        width,
                        height,
                        discard,
                        components: (0..components).collect(),
                    };
                    // One cold complete journey really traverses TCP; the full
                    // matrix also exercises the service and fragmented JPP cache.
                    let tcp = bits == 8 && discard == 0 && window == 0;
                    let decoded = client_decode(&mut service, &mut client, &manifest, &region, tcp);
                    let actual = client_samples(&decoded);
                    let mut metrics = Metrics::default();
                    let expected = reference.decode(&region, &mut metrics).unwrap();
                    assert_eq!(
                        (expected.width, expected.height),
                        (decoded.width, decoded.height)
                    );
                    assert_eq!(
                        actual, expected.samples,
                        "bits={bits}, components={components}, discard={discard}, window={window}"
                    );
                    assert!(
                        metrics.full_tile_pixels
                            >= u64::from(decoded.width) * u64::from(decoded.height)
                    );
                    assert!(metrics.peak_codec_workspace_bytes <= WORKSPACE_BYTES);
                    records.push(Request {
                        tid: manifest.tid.clone(),
                        region,
                        width: decoded.width,
                        height: decoded.height,
                        precision: bits,
                        fnv1a64_u16le: checksum(&actual),
                    });
                }
            }
            // Evidence comparison must consume the cache/client result, including
            // actual sample and dimension failures, rather than its own checksum.
            let report = compare(&root, &records[..1]).unwrap();
            assert_eq!(report.mismatched_records, 0);
            records[0].fnv1a64_u16le = "0000000000000000".into();
            assert_eq!(compare(&root, &records[..1]).unwrap().mismatched_records, 1);
            records[0].tid = "wrong-representation".into();
            assert!(compare(&root, &records[..1]).is_err());
            let descriptor = root.join("descriptors/0.bin");
            std::fs::write(descriptor, b"corrupted").unwrap();
            assert!(
                reference
                    .decode(&records[0].region, &mut Metrics::default())
                    .is_err()
            );
        }
    }

    #[test]
    fn checksum_is_native_interleaved_u16_little_endian() {
        // Known byte sequence [1, 0, 2, 0, 255, 0], independently calculated.
        assert_eq!(checksum(&[1, 2, 255]), "39d182d7e6dc6bb1");
    }
}
