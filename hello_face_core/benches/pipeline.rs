//! Per-stage latency of the face pipeline on this machine (#239).
//!
//! `cargo bench -p hello_face_core` loads the real models (from
//! `default_models_dir()`, overridable with `LINUX_HELLO_MODELS_DIR`) and
//! times each stage a verification runs per camera frame: SCRFD detection,
//! ArcFace alignment + embedding, and the IR/RGB liveness scores. Prints
//! min/median/p95/max per stage, then the process's peak RSS, which is
//! roughly what the models cost the daemon in memory.
//!
//! No camera needed. By default the frame is synthetic, which times the
//! networks just as well (their cost doesn't depend on what's in the frame).
//! To also see a real detection, pass a photo:
//! `env LINUX_HELLO_BENCH_FRAME=face.jpg cargo bench -p hello_face_core`.
//!
//! Skips (exit 0) when the models aren't installed: they're not shipped
//! (#211), see `sudo linux-hello models install`.
//!
//! A plain `harness = false` binary rather than criterion: per-frame stages
//! cost milliseconds, so wall-clock percentiles are enough, and it keeps
//! `cargo test` from building a benchmark framework on every CI run.

use hello_face_core::{liveness, FaceRegion};
use std::time::{Duration, Instant};

const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;
const WARMUP: usize = 5;
const ITERATIONS: usize = 50;

fn main() {
    let models_dir = hello_face_core::default_models_dir();
    let load_start = Instant::now();
    let Some((detector, extractor)) = hello_face_core::load_models(&models_dir) else {
        eprintln!(
            "skipping: face models not installed in {} (sudo linux-hello models install)",
            models_dir.display()
        );
        return;
    };
    println!(
        "models: {} ({}), {} ({}) from {}, loaded in {:.0} ms",
        detector.name(),
        detector.model_version(),
        extractor.model_name(),
        extractor.model_version(),
        models_dir.display(),
        load_start.elapsed().as_secs_f64() * 1000.0
    );

    let (rgb, width, height, source) = frame();
    println!("frame: {source}, {width}x{height}\n");
    let gray: Vec<u8> = rgb.chunks_exact(3).map(|p| p[1]).collect();

    let detected = detector
        .detect(&rgb, width, height, 3)
        .expect("detection failed");
    // A synthetic frame has no face: time extraction on a centered region
    // of a typical size instead.
    let face = detected
        .into_iter()
        .max_by(|a, b| a.confidence.total_cmp(&b.confidence))
        .unwrap_or_else(|| FaceRegion {
            bounding_box: (width / 4, height / 5, width / 2, height * 3 / 5),
            confidence: 1.0,
            landmarks: vec![],
        });

    println!(
        "{:<28} {:>9} {:>9} {:>9} {:>9}",
        "stage", "min", "median", "p95", "max"
    );
    report(
        "detection (SCRFD-500M)",
        time(|| {
            detector.detect(&rgb, width, height, 3).ok();
        }),
    );
    report(
        "embedding (ArcFace MBF)",
        time(|| {
            extractor.extract(&face, &rgb, width, height, 3).ok();
        }),
    );
    report(
        "IR liveness score",
        time(|| {
            liveness::ir_liveness_score(&gray, width, height, &face);
        }),
    );
    report(
        "RGB liveness score",
        time(|| {
            liveness::rgb_liveness_score(&rgb, width, height, &face);
        }),
    );

    if let Some(kb) = peak_rss_kb() {
        println!("\npeak RSS: {:.0} MiB", kb as f64 / 1024.0);
    }
}

/// The RGB frame to run on: `LINUX_HELLO_BENCH_FRAME` resized to 640x480
/// (what the daemon negotiates with the camera), or a synthetic gradient.
fn frame() -> (Vec<u8>, u32, u32, String) {
    if let Some(path) = std::env::var_os("LINUX_HELLO_BENCH_FRAME") {
        let img = image::open(&path)
            .unwrap_or_else(|e| panic!("LINUX_HELLO_BENCH_FRAME {:?}: {}", path, e))
            .resize_exact(WIDTH, HEIGHT, image::imageops::FilterType::Triangle)
            .to_rgb8();
        return (
            img.into_raw(),
            WIDTH,
            HEIGHT,
            format!("{}", std::path::Path::new(&path).display()),
        );
    }
    let rgb = (0..WIDTH * HEIGHT)
        .flat_map(|i| {
            let (x, y) = (i % WIDTH, i / WIDTH);
            [(x * 255 / WIDTH) as u8, (y * 255 / HEIGHT) as u8, 128]
        })
        .collect();
    (rgb, WIDTH, HEIGHT, "synthetic".to_string())
}

fn time(mut f: impl FnMut()) -> Vec<Duration> {
    for _ in 0..WARMUP {
        f();
    }
    let mut samples: Vec<Duration> = (0..ITERATIONS)
        .map(|_| {
            let start = Instant::now();
            f();
            start.elapsed()
        })
        .collect();
    samples.sort();
    samples
}

fn report(stage: &str, sorted: Vec<Duration>) {
    let ms = |d: Duration| format!("{:.2} ms", d.as_secs_f64() * 1000.0);
    let p95 = sorted[(sorted.len() * 95 / 100).min(sorted.len() - 1)];
    println!(
        "{:<28} {:>9} {:>9} {:>9} {:>9}",
        stage,
        ms(sorted[0]),
        ms(sorted[sorted.len() / 2]),
        ms(p95),
        ms(sorted[sorted.len() - 1])
    );
}

/// `VmHWM` from /proc/self/status, in KiB.
fn peak_rss_kb() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("VmHWM:"))?
        .trim()
        .trim_end_matches("kB")
        .trim()
        .parse()
        .ok()
}
