//! Runtime GDAL C API. The library, driver directory and driver policy are
//! explicit invocation inputs, so public sources contain no host build paths.
use anyhow::{Result, ensure};
use emuella_viewer_source::{Profile, codec};
use libloading::Library;
use std::{
    ffi::{CStr, CString, c_char, c_int, c_void},
    path::Path,
};
type Handle = *mut c_void;
pub struct Raster {
    library: Library,
    dataset: Handle,
    pub width: u32,
    pub height: u32,
    pub components: u16,
    pub driver: String,
    pub nitf_ic: Option<String>,
    bands: Vec<u16>,
    pub bits: u8,
}
impl Raster {
    /// GDAL is used synchronously on the creating preparation thread.
    pub fn open(
        library_path: &Path,
        path: &Path,
        bands: Vec<u16>,
        bits: u8,
        cache_bytes: i64,
    ) -> Result<Self> {
        ensure!(
            (8..=16).contains(&bits)
                && matches!(bands.len(), 1 | 3)
                && bands.iter().all(|&b| b > 0),
            "supported unsigned bands/precision"
        );
        // SAFETY: symbols use GDAL's stable C ABI; the Library outlives dataset.
        unsafe {
            let library = Library::new(library_path)?;
            library.get::<unsafe extern "C" fn(*const c_char, *const c_char)>(
                b"CPLSetConfigOption\0",
            )?(c"GDAL_PAM_ENABLED".as_ptr(), c"NO".as_ptr());
            library.get::<unsafe extern "C" fn(*const c_char, *const c_char)>(
                b"CPLSetConfigOption\0",
            )?(c"JP2EMUELLA_REQUIRE_SOURCE_INDEX".as_ptr(), c"YES".as_ptr());
            library.get::<unsafe extern "C" fn()>(b"GDALAllRegister\0")?();
            let get_driver = library
                .get::<unsafe extern "C" fn(*const c_char) -> Handle>(b"GDALGetDriverByName\0")?;
            let deregister =
                library.get::<unsafe extern "C" fn(Handle)>(b"GDALDeregisterDriver\0")?;
            // Match the maintained fork's project-authored NITF integration tests:
            // remove drivers tried before JP2Emuella, without invoking their codecs.
            for name in [c"JP2KAK", c"JP2ECW", c"JP2MRSID", c"JP2OPENJPEG"] {
                let driver = get_driver(name.as_ptr());
                if !driver.is_null() {
                    deregister(driver);
                }
            }
            let emuella_driver = get_driver(c"JP2Emuella".as_ptr());
            let emuella_registered = !emuella_driver.is_null();
            let indexed_source_supported = if emuella_registered {
                let capability =
                    library.get::<unsafe extern "C" fn(
                        Handle,
                        *const c_char,
                        *const c_char,
                    ) -> *const c_char>(b"GDALGetMetadataItem\0")?(
                        emuella_driver,
                        c"JP2EMUELLA_SOURCE_INDEX".as_ptr(),
                        std::ptr::null(),
                    );
                !capability.is_null()
                    && CStr::from_ptr(capability).to_bytes() == b"REQUIRED_SUPPORTED"
            } else {
                false
            };
            library.get::<unsafe extern "C" fn(i64)>(b"GDALSetCacheMax64\0")?(cache_bytes);
            let path = CString::new(path.as_os_str().as_encoded_bytes())?;
            let dataset = library
                .get::<unsafe extern "C" fn(*const c_char, c_int) -> Handle>(b"GDALOpen\0")?(
                path.as_ptr(),
                0,
            );
            ensure!(!dataset.is_null(), "GDAL failed to open source read-only");
            let result = (|| {
                let width = library
                    .get::<unsafe extern "C" fn(Handle) -> c_int>(b"GDALGetRasterXSize\0")?(
                    dataset,
                );
                let height = library
                    .get::<unsafe extern "C" fn(Handle) -> c_int>(b"GDALGetRasterYSize\0")?(
                    dataset,
                );
                let count = library
                    .get::<unsafe extern "C" fn(Handle) -> c_int>(b"GDALGetRasterCount\0")?(
                    dataset,
                );
                ensure!(
                    width > 0 && height > 0 && bands.iter().all(|&b| i32::from(b) <= count),
                    "invalid raster geometry/bands"
                );
                for &band in &bands {
                    let handle = library.get::<unsafe extern "C" fn(Handle, c_int) -> Handle>(
                        b"GDALGetRasterBand\0",
                    )?(dataset, i32::from(band));
                    let datatype = library
                        .get::<unsafe extern "C" fn(Handle) -> c_int>(b"GDALGetRasterDataType\0")?(
                        handle,
                    );
                    ensure!(
                        datatype == if bits == 8 { 1 } else { 2 },
                        "native source type must be Byte or UInt16 without conversion"
                    );
                }
                let driver = library
                    .get::<unsafe extern "C" fn(Handle) -> Handle>(b"GDALGetDatasetDriver\0")?(
                    dataset,
                );
                let name = library.get::<unsafe extern "C" fn(Handle) -> *const c_char>(
                    b"GDALGetDriverShortName\0",
                )?(driver);
                let driver = CStr::from_ptr(name).to_str()?.to_owned();
                ensure!(
                    matches!(driver.as_str(), "GTiff" | "NITF"),
                    "only GTiff/NITF sources admitted"
                );
                let nitf_ic = if driver == "NITF" {
                    ensure!(
                        emuella_registered,
                        "NITF requires registered JP2Emuella driver"
                    );
                    ensure!(
                        indexed_source_supported,
                        "NITF preparation requires JP2Emuella retained source-index support"
                    );
                    let value =
                        library.get::<unsafe extern "C" fn(
                            Handle,
                            *const c_char,
                            *const c_char,
                        ) -> *const c_char>(
                            b"GDALGetMetadataItem\0"
                        )?(dataset, c"NITF_IC".as_ptr(), std::ptr::null());
                    ensure!(!value.is_null(), "NITF IC metadata missing");
                    let value = CStr::from_ptr(value).to_str()?.to_owned();
                    ensure!(value == "C8", "only NITF IC=C8 admitted");
                    Some(value)
                } else {
                    None
                };
                Ok((width as u32, height as u32, driver, nitf_ic))
            })();
            match result {
                Ok((width, height, driver, nitf_ic)) => Ok(Self {
                    library,
                    dataset,
                    width,
                    height,
                    components: bands.len() as u16,
                    driver,
                    nitf_ic,
                    bands,
                    bits,
                }),
                Err(e) => {
                    library.get::<unsafe extern "C" fn(Handle)>(b"GDALClose\0")?(dataset);
                    Err(e)
                }
            }
        }
    }
    pub fn profile(
        &self,
        tile_edge: u32,
        decomposition_levels: u8,
        bits_per_pixel: f32,
    ) -> Profile {
        Profile {
            width: self.width,
            height: self.height,
            tile_edge,
            decomposition_levels,
            bits_per_sample: self.bits,
            components: self.components,
            bits_per_pixel,
        }
    }
    pub fn read_tile(&mut self, rect: codec::TileRect, planes: &mut [Vec<u8>]) -> Result<()> {
        ensure!(planes.len() == self.bands.len(), "plane count");
        // SAFETY: GDAL writes the checked tile-sized, tightly packed native buffer.
        unsafe {
            let band_fn = self
                .library
                .get::<unsafe extern "C" fn(Handle, c_int) -> Handle>(b"GDALGetRasterBand\0")?;
            let read = self.library.get::<unsafe extern "C" fn(
                Handle,
                c_int,
                c_int,
                c_int,
                c_int,
                c_int,
                *mut c_void,
                c_int,
                c_int,
                c_int,
                c_int,
                c_int,
            ) -> c_int>(b"GDALRasterIO\0")?;
            for (&band, plane) in self.bands.iter().zip(planes) {
                ensure!(
                    plane.len()
                        == rect.width as usize
                            * rect.height as usize
                            * usize::from(self.bits.div_ceil(8)),
                    "tile buffer length"
                );
                let result = read(
                    band_fn(self.dataset, i32::from(band)),
                    0,
                    rect.x as i32,
                    rect.y as i32,
                    rect.width as i32,
                    rect.height as i32,
                    plane.as_mut_ptr().cast(),
                    rect.width as i32,
                    rect.height as i32,
                    if self.bits == 8 { 1 } else { 2 },
                    0,
                    0,
                );
                ensure!(result == 0, "GDAL tile read failed");
                // The codec requires little-endian unsigned words.
                if cfg!(target_endian = "big") && self.bits > 8 {
                    for word in plane.chunks_exact_mut(2) {
                        word.swap(0, 1);
                    }
                }
            }
        }
        Ok(())
    }
}
impl Drop for Raster {
    fn drop(&mut self) {
        // SAFETY: unique dataset handle closes before its library.
        unsafe {
            if let Ok(close) = self
                .library
                .get::<unsafe extern "C" fn(Handle)>(b"GDALClose\0")
            {
                close(self.dataset);
            }
        }
    }
}

/// Create a public native-precision GTiff fixture using the same deterministic
/// window generator as direct representation fixtures. At most one tile is held.
pub fn write_fixture(library_path: &Path, output: &Path, profile: &Profile) -> Result<()> {
    emuella_viewer_source::checked(codec::ht_indexed::IndexedLossyHt::sparse(
        profile.codec(),
        3,
        0..1,
    ))?;
    ensure!(!output.exists(), "fixture output exists");
    let temporary = output.with_extension(format!("{}.incomplete.tif", std::process::id()));
    ensure!(!temporary.exists(), "temporary fixture exists");
    // SAFETY: stable GDAL C signatures; dataset and buffers remain live through calls.
    let result = unsafe {
        let library = Library::new(library_path)?;
        library
            .get::<unsafe extern "C" fn(*const c_char, *const c_char)>(b"CPLSetConfigOption\0")?(
            c"GDAL_PAM_ENABLED".as_ptr(),
            c"NO".as_ptr(),
        );
        library.get::<unsafe extern "C" fn()>(b"GDALAllRegister\0")?();
        let driver = library
            .get::<unsafe extern "C" fn(*const c_char) -> Handle>(b"GDALGetDriverByName\0")?(
            c"GTiff".as_ptr(),
        );
        ensure!(!driver.is_null(), "GTiff unavailable");
        let name = CString::new(temporary.as_os_str().as_encoded_bytes())?;
        let options = [
            c"TILED=YES".as_ptr(),
            c"BLOCKXSIZE=256".as_ptr(),
            c"BLOCKYSIZE=256".as_ptr(),
            c"BIGTIFF=IF_NEEDED".as_ptr(),
            std::ptr::null(),
        ];
        let dataset = library.get::<unsafe extern "C" fn(
            Handle,
            *const c_char,
            c_int,
            c_int,
            c_int,
            c_int,
            *const *const c_char,
        ) -> Handle>(b"GDALCreate\0")?(
            driver,
            name.as_ptr(),
            profile.width as i32,
            profile.height as i32,
            i32::from(profile.components),
            if profile.bits_per_sample == 8 { 1 } else { 2 },
            options.as_ptr(),
        );
        ensure!(!dataset.is_null(), "GTiff creation failed");
        let result = (|| {
            let band_fn = library
                .get::<unsafe extern "C" fn(Handle, c_int) -> Handle>(b"GDALGetRasterBand\0")?;
            let write = library.get::<unsafe extern "C" fn(
                Handle,
                c_int,
                c_int,
                c_int,
                c_int,
                c_int,
                *mut c_void,
                c_int,
                c_int,
                c_int,
                c_int,
                c_int,
            ) -> c_int>(b"GDALRasterIO\0")?;
            for y in (0..profile.height).step_by(256) {
                for x in (0..profile.width).step_by(256) {
                    let rect = codec::TileRect {
                        tile_index: 0,
                        tile_x: x / 256,
                        tile_y: y / 256,
                        x,
                        y,
                        width: 256.min(profile.width - x),
                        height: 256.min(profile.height - y),
                    };
                    let mut planes = vec![
                        vec![
                            0;
                            rect.width as usize
                                * rect.height as usize
                                * usize::from(profile.bits_per_sample.div_ceil(8))
                        ];
                        usize::from(profile.components)
                    ];
                    crate::synthetic_tile(profile, rect, &mut planes)?;
                    for (band, plane) in planes.iter_mut().enumerate() {
                        if cfg!(target_endian = "big") && profile.bits_per_sample > 8 {
                            for word in plane.chunks_exact_mut(2) {
                                word.swap(0, 1);
                            }
                        }
                        ensure!(
                            write(
                                band_fn(dataset, band as i32 + 1),
                                1,
                                x as i32,
                                y as i32,
                                rect.width as i32,
                                rect.height as i32,
                                plane.as_mut_ptr().cast(),
                                rect.width as i32,
                                rect.height as i32,
                                if profile.bits_per_sample == 8 { 1 } else { 2 },
                                0,
                                0
                            ) == 0,
                            "GTiff tile write"
                        );
                    }
                }
            }
            Ok(())
        })();
        library.get::<unsafe extern "C" fn(Handle)>(b"GDALClose\0")?(dataset);
        result
    };
    if result.is_err() {
        if temporary.exists() {
            std::fs::remove_file(&temporary)?;
        }
        return result;
    }
    std::fs::rename(temporary, output)?;
    Ok(())
}
