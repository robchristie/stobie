//! Byte-preserving conversion of authenticated legacy validity representations.
use anyhow::{Context, Result, ensure};
use emuella_viewer_source::{Manifest, sha256, validity};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

/// Write a fresh derivative. Incomplete output is retained on failure; inputs are never changed.
pub fn convert(input: &Path, output: &Path) -> Result<serde_json::Value> {
    let old: Manifest = serde_json::from_slice(&fs::read(input.join("manifest.json"))?)?;
    old.validate()?;
    let identity = old
        .identity
        .validity
        .as_ref()
        .context("required validity absent")?;
    ensure!(
        identity.policy == validity::POLICY,
        "legacy validity required"
    );
    fs::create_dir(output)?;
    fs::create_dir(output.join("descriptors"))?;
    fs::create_dir(output.join("masks"))?;
    let mut new = old.clone();
    let mut hashes = Vec::new();
    let mut rows = Vec::new();
    let mut old_mask_bytes = 0u64;
    let mut mask_bytes = 0u64;
    let mut states = [0usize; 3];
    let p = &old.identity.profile;
    for d in 0..=p.decomposition_levels {
        fs::create_dir(output.join(format!("masks/{d}")))?;
        let mut level = Vec::new();
        for t in 0..p.tiles() as u16 {
            let path = format!("masks/{d}/{t}.bin");
            ensure!(
                fs::metadata(input.join(&path))?.len() == identity.byte_len(p, t, d)? as u64,
                "mask length mismatch"
            );
            let bytes = fs::read(input.join(&path))?;
            identity.check(p, t, d, &bytes)?;
            let (w, h) = validity::tile_size(p, t, d)?;
            let plane = (w * h).div_ceil(8) as usize;
            let all_invalid = bytes.iter().all(|&b| b == 0);
            let all_valid = bytes
                .chunks_exact(plane)
                .all(|b| (0..(w * h) as usize).all(|i| validity::bit(b, i)));
            let compact = if all_invalid {
                vec![0]
            } else if all_valid {
                vec![1]
            } else {
                let mut v = Vec::with_capacity(bytes.len() + 1);
                v.push(2);
                v.extend_from_slice(&bytes);
                v
            };
            let entry = validity::catalogue_entry(validity::COMPACT_POLICY, &compact)?;
            states[compact[0] as usize] += 1;
            old_mask_bytes += bytes.len() as u64;
            mask_bytes += compact.len() as u64;
            fs::write(output.join(&path), &compact)?;
            rows.push(serde_json::json!({"discard":d,"tile":t,"state":compact[0],"legacy_bytes":bytes.len(),"compact_bytes":compact.len(),"legacy_sha256":sha256(&bytes),"compact_sha256":sha256(&compact)}));
            level.push(entry);
        }
        hashes.push(level);
    }
    let v = new
        .identity
        .validity
        .as_mut()
        .context("required validity absent")?;
    v.policy = validity::COMPACT_POLICY.into();
    v.tile_sha256 = hashes;
    let mut descriptor_bytes = 0u64;
    for (t, expected) in old.descriptor_sha256.iter().enumerate() {
        let path = format!("descriptors/{t}.bin");
        ensure!(
            fs::metadata(input.join(&path))?.len() <= 1 << 20,
            "descriptor bound"
        );
        let bytes = fs::read(input.join(&path))?;
        ensure!(sha256(&bytes) == *expected, "descriptor digest mismatch");
        descriptor_bytes += bytes.len() as u64;
        fs::write(output.join(path), &bytes)?;
    }
    // Bounded streaming copy and both hashes bind unchanged payload bytes.
    let mut source = fs::File::open(input.join("payload.j2c"))?;
    let mut target = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("payload.j2c"))?;
    let mut buffer = vec![0; 1 << 20];
    let mut payload_bytes = 0u64;
    loop {
        let n = source.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        target.write_all(&buffer[..n])?;
        payload_bytes += n as u64;
    }
    target.sync_all()?;
    ensure!(
        payload_bytes == old.encoded_bytes
            && crate::hash_file(&input.join("payload.j2c"))? == old.identity.payload_sha256
            && crate::hash_file(&output.join("payload.j2c"))? == old.identity.payload_sha256,
        "payload identity mismatch"
    );
    new.seal()?;
    new.validate()?;
    for row in &rows {
        let t = row["tile"].as_u64().unwrap() as u16;
        let d = row["discard"].as_u64().unwrap() as u8;
        let bytes = fs::read(output.join(format!("masks/{d}/{t}.bin")))?;
        new.identity
            .validity
            .as_ref()
            .unwrap()
            .check(p, t, d, &bytes)?;
    }
    let manifest = serde_json::to_vec_pretty(&new)?;
    ensure!(manifest.len() <= 1 << 20, "manifest bound");
    fs::write(output.join("manifest.json"), &manifest)?;
    Ok(
        serde_json::json!({"schema":"compact-validity-conversion/1","legacy_root":input,"root":output,"legacy_tid":old.tid,"tid":new.tid,
        "payload_bytes":payload_bytes,"descriptor_bytes":descriptor_bytes,"legacy_mask_bytes":old_mask_bytes,"mask_bytes":mask_bytes,
        "manifest_bytes":manifest.len(),"legacy_manifest_bytes":fs::metadata(input.join("manifest.json"))?.len(),
        "total_representation_bytes":payload_bytes+descriptor_bytes+mask_bytes+manifest.len() as u64,
        "new_state_tag_serialised_bytes":rows.len()*2,"new_state_tag_retained_heap_bytes":new.identity.validity.as_ref().unwrap().compact_metadata_bytes(),
        "all_invalid_files":states[0],"all_valid_files":states[1],"bitmap_files":states[2],
        "payload_and_descriptors_unchanged":true,"catalogue":rows}),
    )
}
