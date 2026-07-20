#![no_main]

use arbitrary::Arbitrary;
use hello_face_core::liveness::ir_liveness_score;
use hello_face_core::FaceRegion;
use libfuzzer_sys::fuzz_target;

/// Same structured-input rationale as the rgb_liveness_score target: caps
/// width/height/bbox to u16 so the fuzzer lands near the real boundary
/// (buffer length vs. w*h, bbox overlapping/exceeding the frame) instead of
/// mostly rejecting on the first size check.
#[derive(Debug, Arbitrary)]
struct Input {
    width: u16,
    height: u16,
    bbox_x: u16,
    bbox_y: u16,
    bbox_w: u16,
    bbox_h: u16,
    data: Vec<u8>,
}

fuzz_target!(|input: Input| {
    let face = FaceRegion {
        bounding_box: (
            input.bbox_x as u32,
            input.bbox_y as u32,
            input.bbox_w as u32,
            input.bbox_h as u32,
        ),
        confidence: 1.0,
        landmarks: vec![],
    };

    // Must never panic — this is the exact function whose truncated-buffer
    // OOB read was fixed before (see the "no decision" sentinel it returns
    // for a too-short buffer); this target guards against a regression and
    // covers bbox/dimension combinations the handwritten unit tests don't.
    let score = ir_liveness_score(&input.data, input.width as u32, input.height as u32, &face);
    assert!((0.0..=1.0).contains(&score), "score out of [0,1]: {score}");
});
