#![no_main]

use arbitrary::Arbitrary;
use hello_face_core::liveness::rgb_liveness_score;
use hello_face_core::FaceRegion;
use libfuzzer_sys::fuzz_target;

/// Structured input instead of a raw byte blob: width/height/bounding-box
/// are capped to u16 so the fuzzer spends its time near the interesting
/// boundary (buffer length vs. w*h*3, bbox overlapping/exceeding the frame)
/// rather than mostly generating buffers too short to ever pass the initial
/// size check.
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

    // Must never panic (index out of bounds, overflow, etc.) regardless of
    // how `width`/`height`/`bbox` relate to `data`'s actual length — this
    // is exactly the class of bug already found and fixed once in this
    // function's IR counterpart (a truncated/mismatched capture buffer).
    let score = rgb_liveness_score(&input.data, input.width as u32, input.height as u32, &face);
    assert!((0.0..=1.0).contains(&score), "score out of [0,1]: {score}");
});
