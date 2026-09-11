use anyhow::{Context, Result, ensure};
use emuella_viewer_source::{Identity, Profile, sha256};
use emuella_viewer_tools::{Service, fixture, gdal::Raster, hash_file, prepare_with_validity};
use std::{collections::BTreeMap, net::TcpListener, path::PathBuf};
fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let command = args.next().context(
        "fixture | prepare | serve | measure | reference (see docs/emuella-viewer-service.md)",
    )?;
    let mut options = BTreeMap::new();
    let mut roots = Vec::new();
    while let Some(key) = args.next() {
        let value = args.next().context("every option requires a value")?;
        if key == "--representation" {
            roots.push(PathBuf::from(value));
        } else {
            ensure!(
                key.starts_with("--") && !options.contains_key(&key),
                "unknown/duplicate argument"
            );
            options.insert(key, value);
        }
    }
    let get =
        |key: &str, default: &str| options.get(key).cloned().unwrap_or_else(|| default.into());
    match command.as_str() {
        "fixture" | "fixture-raster" => {
            let profile = Profile {
                width: get("--width", "2048").parse()?,
                height: get("--height", "2048").parse()?,
                tile_edge: get("--tile", "512").parse()?,
                decomposition_levels: get("--levels", "5").parse()?,
                bits_per_sample: get("--bits", "16").parse()?,
                components: get("--components", "1").parse()?,
                bits_per_pixel: get("--bpp", "2").parse()?,
            };
            if command == "fixture-raster" {
                emuella_viewer_tools::gdal::write_fixture(
                    &PathBuf::from(
                        options
                            .get("--gdal-library")
                            .context("--gdal-library required")?,
                    ),
                    &PathBuf::from(options.get("--output").context("--output required")?),
                    &profile,
                )?;
                return Ok(());
            }
            let (manifest, metrics) = fixture(
                &PathBuf::from(options.get("--output").context("--output required")?),
                &get("--target", "scientific"),
                profile,
                &get("--codec-revision", "calibration-unpinned"),
            )?;
            println!("{}", serde_json::to_string_pretty(&(manifest, metrics))?);
        }
        "prepare" => {
            let process_before = emuella_viewer_tools::process_io();
            let input = PathBuf::from(options.get("--input").context("--input required")?);
            let library = PathBuf::from(
                options
                    .get("--gdal-library")
                    .context("--gdal-library required")?,
            );
            let bands: Vec<u16> = get("--bands", "1")
                .split(',')
                .map(str::parse)
                .collect::<std::result::Result<_, _>>()?;
            let bits = get("--bits", "16").parse()?;
            let mut raster = Raster::open(&library, &input, bands.clone(), bits, 64 << 20)?;
            let profile = raster.profile(
                get("--tile", "512").parse()?,
                get("--levels", "5").parse()?,
                get("--bpp", "2").parse()?,
            );
            let hash_start = std::time::Instant::now();
            let source_sha256 = hash_file(&input)?;
            let hash_ms = hash_start.elapsed().as_secs_f64() * 1000.;
            let source_bytes = std::fs::metadata(&input)?.len();
            let mut mask_raster = if let Some(input) = options.get("--mask-input") {
                let selected: Vec<u16> = options
                    .get("--mask-bands")
                    .context("--mask-bands required")?
                    .split(',')
                    .map(str::parse)
                    .collect::<std::result::Result<_, _>>()?;
                let original_bits = get("--mask-bits", &bits.to_string()).parse()?;
                let source = Raster::open(
                    &library,
                    &PathBuf::from(input),
                    selected.clone(),
                    original_bits,
                    64 << 20,
                )?;
                ensure!(
                    source.width == raster.width
                        && source.height == raster.height
                        && source.components == raster.components,
                    "mask/source grid mismatch"
                );
                Some((
                    source,
                    emuella_viewer_source::validity::ValidityIdentity {
                        source_sha256: hash_file(&PathBuf::from(input))?,
                        bands: selected,
                        policy: emuella_viewer_source::validity::POLICY.into(),
                        tile_sha256: Vec::new(),
                    },
                ))
            } else {
                ensure!(
                    !options.contains_key("--mask-bands") && !options.contains_key("--mask-bits"),
                    "--mask-input required"
                );
                None
            };
            let identity = Identity {
                validity: mask_raster.as_ref().map(|(_, v)| v.clone()),
                source_sha256,
                bands,
                profile,
                codec_revision: options
                    .get("--codec-revision")
                    .context("exact --codec-revision required")?
                    .clone(),
                encoding_contract: "indexed-htonly-no-mct-irreversible97-one-layer-rate-search-v1"
                    .into(),
                spatial_policy_sha256: sha256(b"uniform-v1"),
                payload_sha256: String::new(),
                descriptor_format: "EHTIDX01".into(),
            };
            let output = PathBuf::from(options.get("--output").context("--output required")?);
            let mut mask_reader = |rect| {
                mask_raster
                    .as_mut()
                    .context("mask source absent")?
                    .0
                    .read_masks(rect)
            };
            let masks_enabled = identity.validity.is_some();
            let (manifest, mut metrics) = prepare_with_validity(
                &output,
                &get("--target", "image"),
                identity,
                |rect, planes| raster.read_tile(rect, planes),
                get("--retain-incomplete", "false").parse()?,
                if masks_enabled {
                    Some(&mut mask_reader)
                } else {
                    None
                },
            )?;
            metrics.source_outer_driver = Some(raster.driver.clone());
            metrics.source_nitf_ic = raster.nitf_ic.clone();
            metrics.source_index_required = Some(raster.driver == "NITF");
            metrics.source_decoder_policy = Some(
                if raster.driver == "NITF" {
                    "JP2Emuella retained source index required; prior alternative JP2 drivers deregistered; PAM disabled"
                } else {
                    "native GTiff; PAM disabled"
                }
                .into(),
            );
            let process_after = emuella_viewer_tools::process_io();
            metrics.process_read_bytes = process_after
                .0
                .zip(process_before.0)
                .map(|(a, b)| a.saturating_sub(b));
            metrics.process_rchar = process_after
                .1
                .zip(process_before.1)
                .map(|(a, b)| a.saturating_sub(b));
            metrics.process_syscr = process_after
                .2
                .zip(process_before.2)
                .map(|(a, b)| a.saturating_sub(b));
            metrics.source_hash_read_bytes = source_bytes;
            metrics.source_hash_ms = hash_ms;
            metrics.elapsed_ms += hash_ms;
            std::fs::write(
                output.join("preparation.json"),
                serde_json::to_vec_pretty(&metrics)?,
            )?;
            println!("{}", serde_json::to_string_pretty(&(manifest, metrics))?);
        }
        "measure" => {
            let region = emuella_viewer_source::Region {
                x: get("--x", "0").parse()?,
                y: get("--y", "0").parse()?,
                width: get("--width", "512").parse()?,
                height: get("--height", "512").parse()?,
                discard: get("--discard", "0").parse()?,
                components: get("--components", "0")
                    .split(',')
                    .map(str::parse)
                    .collect::<std::result::Result<_, _>>()?,
            };
            let result = emuella_viewer_tools::measure::measure(
                &get("--address", "127.0.0.1:8123"),
                &get("--target", "scientific-u16"),
                region,
                get("--rounds", "2").parse()?,
                get("--len", "262144").parse()?,
            )?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        "reference-export" => {
            use std::io::Write;
            ensure!(roots.len() == 1, "exactly one --representation required");
            let reference = emuella_viewer_tools::reference::Reference::open(&roots[0])?;
            let region = emuella_viewer_source::Region {
                x: get("--x", "0").parse()?,
                y: get("--y", "0").parse()?,
                width: get("--width", "256").parse()?,
                height: get("--height", "256").parse()?,
                discard: get("--discard", "0").parse()?,
                components: (0..reference.manifest.identity.profile.components).collect(),
            };
            let mut metrics = emuella_viewer_tools::reference::Metrics::default();
            let frame = reference.decode(&region, &mut metrics)?;
            let output = PathBuf::from(options.get("--output").context("--output required")?);
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output)?;
            for sample in &frame.samples {
                file.write_all(&sample.to_le_bytes())?;
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "method": "same-codec complete selected tiles, direct immutable file, crop; not independent decoding",
                    "tid": reference.manifest.tid, "region": region,
                    "width": frame.width, "height": frame.height, "metrics": metrics,
                    "fnv1a64_u16le": emuella_viewer_tools::reference::checksum(&frame.samples),
                }))?
            );
        }
        "reference" => {
            ensure!(roots.len() == 1, "exactly one --representation required");
            let report = emuella_viewer_tools::reference::compare_file(
                &roots[0],
                &PathBuf::from(options.get("--requests").context("--requests required")?),
            )?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            ensure!(
                report.mismatched_records == 0,
                "reference comparison failed"
            );
        }
        "serve" => {
            ensure!(
                !roots.is_empty(),
                "one or more --representation directories required"
            );
            let listener = TcpListener::bind(get("--listen", "127.0.0.1:8123"))?;
            println!(
                "listening http://{} (OS source cache state uncontrolled; logical and process counters reported separately)",
                listener.local_addr()?
            );
            Service::open(
                &roots,
                options.get("--web").map(PathBuf::from),
                get("--verify-payload", "false") == "true",
            )?
            .serve(listener)?;
        }
        _ => anyhow::bail!("unknown command"),
    }
    Ok(())
}
