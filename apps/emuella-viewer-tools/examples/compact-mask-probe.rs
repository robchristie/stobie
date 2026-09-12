//! Targeted native compact-mask qualification against retained exact legacy arrays.
use anyhow::{Context, Result, ensure};
use emuella_viewer_source::{ClientLimits, Manifest, Region, SharedClient, checked, sha256};
use emuella_viewer_tools::Service;
use serde::Deserialize;
use std::{fs, path::PathBuf};
#[derive(Deserialize)]
struct Scene {
    root: PathBuf,
    legacy_root: PathBuf,
    indices: Vec<usize>,
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 3, "input JSON and fresh output required");
    let scenes: Vec<Scene> = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut rows = Vec::new();
    for scene in scenes {
        let manifest: Manifest =
            serde_json::from_slice(&fs::read(scene.root.join("manifest.json"))?)?;
        manifest.validate()?;
        let original = scene
            .legacy_root
            .parent()
            .context("legacy evidence parent")?;
        let requests: Vec<Region> =
            serde_json::from_slice(&fs::read(original.join("requests.json"))?)?;
        let mut service = Service::open(std::slice::from_ref(&scene.root), None, true)?;
        let mut client = SharedClient::new(ClientLimits::default());
        client.register(manifest.clone())?;
        for index in scene.indices {
            let region = requests.get(index).context("request index")?;
            for tile in client.missing_tiles(&manifest.tid, region)? {
                let response = service.route(&format!(
                    "/descriptor/{}/{tile}?tid={}",
                    manifest.target, manifest.tid
                ))?;
                client.install_descriptor(&manifest.tid, tile, &response.body)?;
            }
            let scope = client.begin_request(&manifest.tid, region)?;
            let reservation = client.request_reservation();
            let result = (|| -> Result<serde_json::Value> {
                for tile in client.missing_masks(&manifest.tid, region)? {
                    let response = service.route(&format!(
                        "/mask/{}/{}/{tile}?tid={}",
                        manifest.target, region.discard, manifest.tid
                    ))?;
                    client.install_mask(&manifest.tid, tile, region.discard, &response.body)?;
                }
                for _ in 0..64 {
                    if client.ready(&manifest.tid, region)? {
                        break;
                    }
                    let request = client.request(&manifest.tid, region, 256 << 10)?;
                    let response =
                        service.route(&format!("/jpip?{}", checked(request.query())?))?;
                    let mut reader = client.begin_response_headers(
                        &manifest.tid,
                        response
                            .headers
                            .iter()
                            .map(|(k, v)| (k.as_str(), v.as_str())),
                    )?;
                    client.receive(&mut reader, &response.body)?;
                    client.finish(reader)?;
                }
                ensure!(
                    client.ready(&manifest.tid, region)?,
                    "bounded transport incomplete"
                );
                let decoded = client.decode(&manifest.tid, region)?;
                let expected_validity =
                    fs::read(original.join(format!("region-{index}.validity")))?;
                let expected_samples = fs::read(original.join(format!("region-{index}.u16le")))?;
                ensure!(
                    decoded.validity.as_ref() == Some(&expected_validity),
                    "legacy validity mismatch"
                );
                let mut samples = Vec::with_capacity(expected_samples.len());
                for i in 0..(decoded.width * decoded.height) as usize {
                    for plane in &decoded.planes {
                        if decoded.bits_per_sample == 8 {
                            samples.extend_from_slice(&[plane[i], 0]);
                        } else {
                            samples.extend_from_slice(&plane[i * 2..i * 2 + 2]);
                        }
                    }
                }
                ensure!(samples == expected_samples, "legacy samples mismatch");
                Ok(
                    serde_json::json!({"target":manifest.target,"tid":manifest.tid,"index":index,"region":region,
                    "pixels":expected_validity.len(),"samples":samples.len()/2,"validity_sha256":sha256(&expected_validity),"samples_sha256":sha256(&samples),
                    "reservation_bytes":reservation.0,"pin_metadata_bytes":reservation.1,"resident_bytes":client.resident_bytes(),
                    "compact_catalogue_metadata_bytes":client.compact_catalogue_metadata_bytes(),
                    "mask_cache_metadata_bytes":client.mask_cache_metadata_bytes(),
                    "peak_mask_cache_metadata_bytes":client.peak_mask_cache_metadata_bytes(),
                    "mask_cache_entries":client.mask_cache_entries(),"mask_cache_slots":client.mask_cache_slots(),
                    "mask_cache_slot_bytes":client.mask_cache_slot_bytes(),"mask_cache_container_bytes":client.mask_cache_container_bytes(),"metrics":client.metrics}),
                )
            })();
            client.end_request(scope);
            ensure!(
                client.request_reservation() == (0, 0),
                "reservation release"
            );
            let mut row = result?;
            row["terminal_reservation_bytes"] = serde_json::json!([0, 0]);
            rows.push(row);
        }
    }
    use std::io::Write;
    let result = serde_json::json!({"schema":"compact-validity-native-probe/1","passed":true,"records":rows});
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[2])?
        .write_all(&serde_json::to_vec_pretty(&result)?)?;
    Ok(())
}
