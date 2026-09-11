//! Authored downstream use of the original public struct literals and methods.
use polyorama_core::{
    DemandPriority, ImageRegion, RegionConsumerId, RegionDemand, RegionKey, RegionalFrame,
    RegionalPixels, RepresentationId, SampleLayout, SourceStage,
};
use polyorama_runtime::{
    RegionalCompletion, RegionalRuntime, RegionalRuntimeLimits, RegionalUpload,
};
fn pixels() -> RegionalPixels {
    RegionalPixels {
        width: 2,
        height: 2,
        layout: SampleLayout::Scalar,
        precision: 11,
        samples: vec![1024; 4],
    }
}
fn runtime(bytes: usize) -> RegionalRuntime {
    let mut runtime = RegionalRuntime::new(RegionalRuntimeLimits {
        decoded_bytes: bytes,
        max_demands: 1,
        max_in_flight: 1,
    });
    runtime
        .reconcile(
            0,
            [RegionDemand {
                consumer: RegionConsumerId(0),
                key: RegionKey {
                    representation: RepresentationId([0; 32]),
                    region: ImageRegion {
                        x: 0,
                        y: 0,
                        width: 2,
                        height: 2,
                    },
                    reduction: 0,
                    components: vec![0],
                    stage: SourceStage(0),
                },
                priority: DemandPriority::Visible,
                max_decoded_bytes: bytes,
            }],
        )
        .unwrap();
    runtime
}
#[test]
fn old_pixel_and_upload_literals_and_runtime_methods_remain_source_compatible() {
    let mut runtime = runtime(8);
    let request = runtime.dispatch().pop().unwrap();
    let upload = RegionalUpload {
        key: request.key.clone(),
        token: request.token,
        pixels: pixels(),
    };
    assert_eq!(
        runtime.complete(&request, upload.pixels),
        RegionalCompletion::Accepted
    );
    let upload: RegionalUpload = runtime.take_decoded().unwrap();
    assert_eq!(upload.pixels.samples, vec![1024; 4]);
    assert_eq!(runtime.metrics().upload_bytes, 8);
}
#[test]
fn opt_in_validity_is_accounted_and_legacy_take_never_strips_it() {
    let mut runtime = runtime(12);
    let request = runtime.dispatch().pop().unwrap();
    let frame = RegionalFrame {
        pixels: pixels(),
        validity: Some(vec![1, 0, 1, 0]),
    };
    assert_eq!(
        runtime.complete_frame(&request, frame),
        RegionalCompletion::Accepted
    );
    assert_eq!(runtime.metrics().decoded_bytes, 12);
    assert!(runtime.take_decoded().is_none());
    let upload = runtime.take_decoded_frame().unwrap();
    assert_eq!(upload.pixels.validity, Some(vec![1, 0, 1, 0]));
    assert_eq!(runtime.metrics().upload_bytes, 12);
}
#[test]
fn validity_capacity_and_invalid_values_cannot_escape_existing_reservation() {
    for invalid in [vec![2, 0, 1, 0], {
        let mut v = Vec::with_capacity(16);
        v.extend([1, 0, 1, 0]);
        v
    }] {
        let mut runtime = runtime(12);
        let request = runtime.dispatch().pop().unwrap();
        assert_eq!(
            runtime.complete_frame(
                &request,
                RegionalFrame {
                    pixels: pixels(),
                    validity: Some(invalid)
                }
            ),
            RegionalCompletion::InvalidPayload
        );
        assert_eq!(runtime.metrics().accounted_decoded_bytes(), 0);
    }
}
