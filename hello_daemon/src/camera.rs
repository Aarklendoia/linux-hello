//! Camera abstraction for the daemon
//!
//! Provides a simple interface to capture frames
//! and pass them to the recognition engine

use crate::capture_stream::CaptureFrameEvent;
use hello_camera::{Frame, FrameFormat};
use hello_face_core::{Embedding, EmbeddingExtractor, FaceDetector};
use std::os::unix::io::AsRawFd;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::{debug, info, warn};

/// Camera errors
#[derive(Debug, Error)]
pub enum CameraError {
    #[error("Camera not available")]
    NotAvailable,

    #[error("Capture timeout")]
    Timeout,

    #[error("Capture error: {0}")]
    CaptureError(String),

    #[error("Extraction error: {0}")]
    ExtractionError(String),

    /// Another process (the per-user daemon or the SDDM system listener)
    /// already holds the camera lock. Distinct from a genuine capture
    /// failure so callers/logs can tell "in use elsewhere" apart from
    /// "nobody in front of the camera" — the two used to be indistinguishable
    /// (both silently degraded to all-zero stub frames).
    #[error("Camera busy (in use by another process)")]
    Busy,
}

/// Path of the cross-process camera lock. Created (mode 0666) by
/// systemd-tmpfiles alongside `/run/hello-pam`, so both an unprivileged
/// per-user daemon and the root SDDM system listener can acquire it.
/// Overridable via `LINUX_HELLO_CAMERA_LOCK_PATH` for testing against a
/// scratch file instead of the real `/run/lock/` — unset in production.
fn camera_lock_path() -> String {
    std::env::var("LINUX_HELLO_CAMERA_LOCK_PATH")
        .unwrap_or_else(|_| "/run/lock/linux-hello-camera.lock".to_string())
}

/// Non-blocking, cross-process mutual exclusion around camera device access.
///
/// The V4L2 device itself has no locking of its own; without this, two
/// daemon instances (e.g. a user's own session daemon and the SDDM system
/// listener, on a multi-session machine) capturing at the same time would
/// only be arbitrated by the kernel/driver, previously degrading silently to
/// blank stub frames rather than a clear "busy" error. Released automatically
/// on drop — closing the fd releases the flock, so a panic or early return
/// during capture can't leave it held.
struct CameraLock {
    _file: std::fs::File,
}

impl CameraLock {
    fn try_acquire(path: &std::path::Path) -> Result<Self, CameraError> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|e| CameraError::CaptureError(format!("Camera lock file: {}", e)))?;

        // SAFETY: flock() on a valid, owned fd; no preconditions beyond that.
        let ret = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if ret != 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::WouldBlock {
                return Err(CameraError::Busy);
            }
            return Err(CameraError::CaptureError(format!(
                "Camera lock acquisition: {}",
                err
            )));
        }
        Ok(Self { _file: file })
    }
}

/// Result of a camera capture
pub struct CaptureResult {
    /// Captured RGB frames
    pub frames: Vec<Frame>,

    /// Captured IR frames (None if no IR camera)
    pub ir_frames: Option<Vec<Frame>>,

    /// Extracted embeddings
    pub embeddings: Vec<Embedding>,

    /// Average quality score
    pub quality_score: f32,

    /// IR liveness score (None if no IR camera)
    pub ir_liveness: Option<f32>,
}

/// Camera manager for the daemon
pub struct CameraManager {
    /// Default timeout for captures (ms)
    default_timeout_ms: u64,
    /// Path of the RGB device
    pub rgb_device: String,
    /// Path of the IR device (if detected)
    pub ir_device: Option<String>,
    /// Face detector (SCRFD or fallback)
    detector: Arc<Box<dyn FaceDetector>>,
    /// Embedding extractor (ArcFace or fallback)
    extractor: Arc<Box<dyn EmbeddingExtractor>>,
    /// Override for the cross-process camera lock file path. `None` in
    /// production (resolved via `camera_lock_path()` at acquire time, same
    /// as before this field existed) — set by `for_test()` so concurrent
    /// tests each get their own lock file instead of racing on the shared
    /// `LINUX_HELLO_CAMERA_LOCK_PATH` process-global env var.
    lock_path: Option<std::path::PathBuf>,
}

impl CameraManager {
    /// Create a camera manager by scanning available devices
    pub fn new(default_timeout_ms: u64) -> Self {
        let inventory = hello_camera::scan_cameras();
        info!(
            "Camera inventory: RGB={}, IR={}",
            inventory.rgb_device,
            inventory.ir_device.as_deref().unwrap_or("none")
        );
        let models_dir = hello_face_core::default_models_dir();
        let detector = Arc::new(hello_face_core::create_detector(&models_dir));
        let extractor = Arc::new(hello_face_core::create_extractor(&models_dir));
        Self {
            default_timeout_ms,
            rgb_device: inventory.rgb_device,
            ir_device: inventory.ir_device,
            detector,
            extractor,
            lock_path: None,
        }
    }

    /// Build a `CameraManager` around injected detector/extractor fakes and
    /// an isolated lock file, bypassing `create_detector`/`create_extractor`
    /// (and the real ONNX/model-file dependency that comes with them)
    /// entirely. `rgb_device` should point at a path that doesn't exist, so
    /// the real V4L2 capture functions fail fast and fall through to their
    /// existing (already-safe-without-hardware) stub-frame behavior.
    #[cfg(test)]
    pub(crate) fn for_test(
        rgb_device: impl Into<String>,
        lock_path: std::path::PathBuf,
        detector: Box<dyn FaceDetector>,
        extractor: Box<dyn EmbeddingExtractor>,
    ) -> Self {
        Self {
            default_timeout_ms: 1000,
            rgb_device: rgb_device.into(),
            ir_device: None,
            detector: Arc::new(detector),
            extractor: Arc::new(extractor),
            lock_path: Some(lock_path),
        }
    }

    /// Resolve the lock file path to use: the test override if set, else
    /// the same `camera_lock_path()` production default as before.
    fn resolved_lock_path(&self) -> std::path::PathBuf {
        self.lock_path
            .clone()
            .unwrap_or_else(|| std::path::PathBuf::from(camera_lock_path()))
    }

    /// Check whether an RGB camera is available
    pub fn is_available(&self) -> bool {
        std::path::Path::new(&self.rgb_device).exists()
    }

    /// Check whether an IR camera is available
    pub fn has_ir(&self) -> bool {
        self.ir_device
            .as_ref()
            .map(|p| std::path::Path::new(p).exists())
            .unwrap_or(false)
    }

    /// Capture N RGB frames (+ IR if available) and extract the embeddings
    pub async fn capture_frames(
        &self,
        num_frames: u32,
        timeout_ms: u64,
    ) -> Result<CaptureResult, CameraError> {
        let timeout = if timeout_ms == 0 {
            self.default_timeout_ms
        } else {
            timeout_ms
        };

        info!(
            "Capturing {} frames, timeout={}ms, rgb={}, ir={}",
            num_frames,
            timeout,
            self.rgb_device,
            self.ir_device.as_deref().unwrap_or("none")
        );

        let rgb_device = self.rgb_device.clone();
        let ir_device = self.ir_device.clone();
        let lock_path = self.resolved_lock_path();

        // RGB capture in a blocking thread (V4L2 is not async)
        let (rgb_frames, ir_frames) =
            tokio::task::spawn_blocking(move || -> Result<_, CameraError> {
                // Held for the whole capture; released when this closure returns.
                let _camera_lock = CameraLock::try_acquire(&lock_path)?;

                let mut rgb_frames: Vec<Frame> = Vec::new();
                let mut ir_frames: Vec<Frame> = Vec::new();

                // RGB capture
                let rgb_result = hello_camera::capture_rgb_stream_v4l2(
                    &rgb_device,
                    num_frames,
                    timeout,
                    |data, w, h| {
                        let ts = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;
                        rgb_frames.push(Frame {
                            data,
                            width: w,
                            height: h,
                            format: FrameFormat::Rgb8,
                            timestamp_ms: ts,
                        });
                    },
                );

                if let Err(e) = rgb_result {
                    warn!(
                        "RGB V4L2 capture failed ({}), falling back to simulation",
                        e
                    );
                    rgb_frames.clear();
                }

                // Pad with stub frames if the capture didn't provide enough frames
                let existing = rgb_frames.len() as u32;
                for i in existing..num_frames {
                    rgb_frames.push(Frame {
                        data: vec![0u8; 640 * 480 * 3],
                        width: 640,
                        height: 480,
                        format: FrameFormat::Rgb8,
                        timestamp_ms: i as u64 * 33,
                    });
                }

                // IR capture (parallel, optional)
                if let Some(ref ir_path) = ir_device {
                    let ir_result = hello_camera::capture_gray_stream_v4l2(
                        ir_path,
                        num_frames,
                        timeout,
                        |data, w, h| {
                            let ts = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64;
                            ir_frames.push(Frame {
                                data,
                                width: w,
                                height: h,
                                format: FrameFormat::Gray8,
                                timestamp_ms: ts,
                            });
                        },
                    );
                    if let Err(e) = ir_result {
                        warn!("IR capture failed ({}), disabled for this session", e);
                    }
                }

                let ir_opt = if ir_frames.is_empty() {
                    None
                } else {
                    Some(ir_frames)
                };

                Ok((rgb_frames, ir_opt))
            })
            .await
            .map_err(|e| CameraError::CaptureError(e.to_string()))??;

        // Extract embeddings from the RGB frames via detector + extractor
        let detector = Arc::clone(&self.detector);
        let extractor = Arc::clone(&self.extractor);

        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let embeddings: Vec<Embedding> = rgb_frames
            .iter()
            .enumerate()
            .map(|(i, frame)| {
                // 1. Detect faces in the frame
                debug!(
                    "Frame {}: {}×{} {} bytes",
                    i,
                    frame.width,
                    frame.height,
                    frame.data.len()
                );
                let faces = match detector.detect(&frame.data, frame.width, frame.height, 3) {
                    Ok(f) => f,
                    Err(e) => {
                        warn!("SCRFD detection error frame {}: {}", i, e);
                        vec![]
                    }
                };

                // 2. Take the face with the highest confidence
                let best_face = faces
                    .into_iter()
                    .max_by(|a, b| a.confidence.total_cmp(&b.confidence));

                // 3. Extract the embedding
                if let Some(face) = best_face {
                    match extractor.extract(&face, &frame.data, frame.width, frame.height, 3) {
                        Ok(emb) => return emb,
                        Err(e) => warn!("Embedding extraction frame {}: {}", i, e),
                    }
                } else {
                    warn!("No face detected in frame {}", i);
                }

                // No face detected or extraction failed: empty marker (quality 0)
                // Never use a fake embedding that would skew the comparison
                Embedding {
                    vector: vec![],
                    metadata: hello_face_core::EmbeddingMetadata {
                        model: "no-face".to_string(),
                        model_version: "0.0.0".to_string(),
                        extracted_at: now_secs + i as u64,
                        quality_score: 0.0,
                    },
                }
            })
            .collect();

        // Compute the IR liveness score from the first available IR frame
        let ir_liveness = ir_frames.as_ref().and_then(|frames| {
            let frame = frames.first()?;
            // Use the face box from the first RGB frame to align
            // In practice the RGB/IR cameras share the same field of view on the Brio
            let dummy_face = hello_face_core::FaceRegion {
                bounding_box: (
                    frame.width / 4,
                    frame.height / 5,
                    frame.width / 2,
                    frame.height * 3 / 5,
                ),
                confidence: 1.0,
                landmarks: vec![],
            };
            Some(hello_face_core::liveness::ir_liveness_score(
                &frame.data,
                frame.width,
                frame.height,
                &dummy_face,
            ))
        });

        let quality_score = embeddings
            .iter()
            .map(|e| e.metadata.quality_score)
            .sum::<f32>()
            / embeddings.len().max(1) as f32;

        debug!(
            "Capture complete: {} RGB frames, {} IR frames, quality={:.2}, liveness_ir={:?}",
            rgb_frames.len(),
            ir_frames.as_ref().map(|v| v.len()).unwrap_or(0),
            quality_score,
            ir_liveness,
        );

        Ok(CaptureResult {
            frames: rgb_frames,
            ir_frames,
            embeddings,
            quality_score,
            ir_liveness,
        })
    }

    /// Capture RGB frames continuously (device opened once, for the whole
    /// attempt) for up to `timeout_ms`, extracting an embedding from each
    /// frame with a detected face and handing it to `on_frame` together
    /// with a once-per-session IR liveness score (sampled from a single IR
    /// frame at the very start — same "first frame" semantics
    /// `capture_frames` already used; periodic re-sampling across the
    /// window would be a nice future improvement, not required now) and a
    /// per-frame RGB-only liveness score (always computed fresh — see
    /// `hello_face_core::liveness::rgb_liveness_score` — since it's the
    /// fallback used when there's no IR camera to sample from at all).
    /// Stops as soon as `on_frame` returns `true` ("I've decided, stop") or
    /// the deadline elapses.
    ///
    /// Used by `verify()`'s attempt loop so the camera stays visibly
    /// engaged for the whole window instead of a fixed quick burst —
    /// `capture_frames` is kept unchanged for enrollment's fixed-sample-
    /// count use case.
    pub async fn capture_until<F>(
        &self,
        timeout_ms: u64,
        mut on_frame: F,
    ) -> Result<(), CameraError>
    where
        F: FnMut(Embedding, Option<f32>, f32) -> bool + Send + 'static,
    {
        let timeout = if timeout_ms == 0 {
            self.default_timeout_ms
        } else {
            timeout_ms
        };

        info!(
            "Continuous capture for up to {}ms, rgb={}, ir={}",
            timeout,
            self.rgb_device,
            self.ir_device.as_deref().unwrap_or("none")
        );

        let rgb_device = self.rgb_device.clone();
        let ir_device = self.ir_device.clone();
        let detector = Arc::clone(&self.detector);
        let extractor = Arc::clone(&self.extractor);
        let lock_path = self.resolved_lock_path();

        tokio::task::spawn_blocking(move || -> Result<(), CameraError> {
            // Held for the whole attempt; released when this closure returns.
            let _camera_lock = CameraLock::try_acquire(&lock_path)?;

            // Sample IR liveness from several frames at the start of the
            // attempt and keep the best score, rather than a single frame
            // (same overall 2s budget as before). A lone frame can read low
            // from a transient IR glare/blur/illuminator flicker, which
            // used to condemn the entire attempt: the RGB match would keep
            // succeeding every frame while this one fixed liveness score
            // kept rejecting it for the whole timeout window, burning CPU
            // on face detection/embedding extraction with no chance of
            // ever succeeding. Taking the max over a handful of frames
            // means one bad IR sample no longer dooms the session.
            const IR_LIVENESS_SAMPLES: u32 = 5;
            let ir_liveness = ir_device.as_deref().and_then(|ir_path| {
                let mut best: Option<f32> = None;
                let _ = hello_camera::capture_gray_stream_v4l2(
                    ir_path,
                    IR_LIVENESS_SAMPLES,
                    2000,
                    |data, w, h| {
                        let dummy_face = hello_face_core::FaceRegion {
                            bounding_box: (w / 4, h / 5, w / 2, h * 3 / 5),
                            confidence: 1.0,
                            landmarks: vec![],
                        };
                        let score =
                            hello_face_core::liveness::ir_liveness_score(&data, w, h, &dummy_face);
                        best = Some(best.map_or(score, |b: f32| b.max(score)));
                    },
                );
                best
            });

            let mut frame_index: u32 = 0;
            let result =
                hello_camera::capture_rgb_stream_until(&rgb_device, timeout, |data, w, h| {
                    frame_index += 1;
                    match score_frame(&**detector, &**extractor, frame_index, &data, w, h) {
                        Some((embedding, rgb_liveness)) => {
                            on_frame(embedding, ir_liveness, rgb_liveness)
                        }
                        None => false, // no face / detection or extraction error: keep going
                    }
                });

            if let Err(e) = result {
                // Same fail-safe spirit as capture_frames: a capture hiccup
                // degrades to "no more frames for this attempt", it doesn't
                // hard-fail verify() (camera lock contention above is the
                // one exception that does propagate).
                warn!("Continuous RGB capture ended: {}", e);
            }
            Ok(())
        })
        .await
        .map_err(|e| CameraError::CaptureError(e.to_string()))?
    }

    /// Start a capture session with live streaming
    pub async fn start_capture_stream<F>(
        &self,
        num_frames: u32,
        timeout_ms: u64,
        mut on_frame: F,
    ) -> Result<(), CameraError>
    where
        F: FnMut(CaptureFrameEvent),
    {
        info!(
            "Starting streaming capture: {} frames, timeout={}ms",
            num_frames, timeout_ms
        );

        let rgb_device = self.rgb_device.clone();
        let mut frame_num: u32 = 0;

        let v4l2_result = tokio::task::block_in_place(|| {
            hello_camera::capture_rgb_stream_v4l2(
                &rgb_device,
                num_frames,
                timeout_ms,
                |rgb_data, width, height| {
                    let event = CaptureFrameEvent {
                        frame_number: frame_num,
                        total_frames: num_frames,
                        frame_data: rgb_data,
                        width,
                        height,
                        face_detected: false,
                        face_box: None,
                        quality_score: 0.85,
                        timestamp_ms: 0,
                    };
                    on_frame(event);
                    frame_num += 1;
                },
            )
        });

        match v4l2_result {
            Ok(()) => {
                info!("V4L2 streaming capture complete: {} frames", frame_num);
                return Ok(());
            }
            Err(e) => {
                warn!("V4L2 not available ({}), using simulation", e);
            }
        }

        // Fallback simulation: animated RGB gradient ~30fps
        let start_time = SystemTime::now();
        let timeout_dur = Duration::from_millis(timeout_ms);

        for frame_num_sim in 0..num_frames {
            if start_time.elapsed().unwrap_or_default() > timeout_dur {
                return Err(CameraError::Timeout);
            }

            let frame_data: Vec<u8> = (0u32..640 * 480)
                .flat_map(|i| {
                    let x = (i % 640) as u8;
                    let y = (i / 640) as u8;
                    let t = (frame_num_sim.wrapping_mul(40)) as u8;
                    [x.wrapping_add(t), y.wrapping_add(t), 128u8]
                })
                .collect();

            let ts = start_time
                .elapsed()
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            on_frame(CaptureFrameEvent {
                frame_number: frame_num_sim,
                total_frames: num_frames,
                frame_data,
                width: 640,
                height: 480,
                face_detected: false,
                face_box: None,
                quality_score: 0.85,
                timestamp_ms: ts,
            });
            tokio::time::sleep(Duration::from_millis(33)).await;
        }

        info!(
            "Streaming capture complete: {} frames (simulation)",
            num_frames
        );
        Ok(())
    }
}

/// Detect the best face in a frame, extract its embedding, and score its
/// RGB-only liveness — the per-frame body of `capture_until`'s callback,
/// pulled out as a pure function (no I/O, no `&self`) so it's directly
/// testable with fake detector/extractor implementations and an in-memory
/// buffer, without needing frames to flow through a real or simulated
/// camera at all.
///
/// Returns `None` if no face was detected, or detection/extraction failed —
/// callers treat that as "keep going, no verdict from this frame" rather
/// than a hard error, same as before this was extracted.
pub(crate) fn score_frame(
    detector: &dyn FaceDetector,
    extractor: &dyn EmbeddingExtractor,
    frame_index: u32,
    data: &[u8],
    w: u32,
    h: u32,
) -> Option<(Embedding, f32)> {
    let faces = match detector.detect(data, w, h, 3) {
        Ok(f) => f,
        Err(e) => {
            warn!("SCRFD detection error frame {}: {}", frame_index, e);
            return None;
        }
    };
    let best_face = faces
        .into_iter()
        .max_by(|a, b| a.confidence.total_cmp(&b.confidence))?;
    let embedding = match extractor.extract(&best_face, data, w, h, 3) {
        Ok(emb) => emb,
        Err(e) => {
            warn!("Embedding extraction frame {}: {}", frame_index, e);
            return None;
        }
    };
    // Weaker fallback signal for the (common) no-IR-camera case — see
    // hello_face_core::liveness::rgb_liveness_score. Always computed since
    // the RGB frame and detected face are already in hand at this point.
    let rgb_liveness = hello_face_core::liveness::rgb_liveness_score(data, w, h, &best_face);
    Some((embedding, rgb_liveness))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_camera_manager_creation() {
        // Force the stub detector/extractor by pointing at an empty models
        // dir. This test only exercises the camera scan, and loading the
        // real ONNX models requires a correctly configured ONNX Runtime
        // (ORT_DYLIB_PATH) that isn't guaranteed in a `cargo test` env —
        // without it, `ort` can hang indefinitely rather than failing fast.
        let empty_models_dir = tempfile::tempdir().unwrap();
        // SAFETY: no other test in this binary reads LINUX_HELLO_MODELS_DIR.
        unsafe {
            std::env::set_var("LINUX_HELLO_MODELS_DIR", empty_models_dir.path());
        }

        let camera = CameraManager::new(5000);
        // The scan must not panic even without /dev/video*
        assert!(!camera.rgb_device.is_empty());
    }

    #[test]
    fn test_capture_frames_fallback() {
        // Verify that frame padding works without hardware
        let num_frames: u32 = 3;
        let mut frames: Vec<Frame> = Vec::new();

        for i in 0..num_frames {
            frames.push(Frame {
                data: vec![0u8; 640 * 480 * 3],
                width: 640,
                height: 480,
                format: FrameFormat::Rgb8,
                timestamp_ms: i as u64 * 33,
            });
        }

        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].width, 640);
        assert_eq!(frames[0].height, 480);
    }

    #[test]
    fn test_camera_lock_busy_on_contention() {
        // A scratch file, private to this test — no process-global env var
        // involved, so this is safe to run concurrently with any other test.
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join("camera.lock");

        let first = CameraLock::try_acquire(&lock_path).expect("first acquire should succeed");
        let second = CameraLock::try_acquire(&lock_path);
        assert!(
            matches!(second, Err(CameraError::Busy)),
            "expected Busy while the first guard is still held, got {:?}",
            second.err()
        );

        drop(first);
        let third = CameraLock::try_acquire(&lock_path);
        assert!(
            third.is_ok(),
            "expected the lock to be acquirable again after the first guard was dropped"
        );
    }

    use crate::test_support::{blank_rgb_frame, default_face_region, FakeDetector, FakeExtractor};

    /// A `CameraManager` pointed at a device path that can't possibly exist,
    /// with its own private lock file — every test below gets independent
    /// state, no shared global (env var or otherwise).
    fn for_test(
        detector: FakeDetector,
        extractor: FakeExtractor,
    ) -> (tempfile::TempDir, CameraManager) {
        let dir = tempfile::tempdir().unwrap();
        let rgb_device = dir
            .path()
            .join("no-camera-here")
            .to_string_lossy()
            .into_owned();
        let lock_path = dir.path().join("camera.lock");
        let camera = CameraManager::for_test(
            rgb_device,
            lock_path,
            Box::new(detector),
            Box::new(extractor),
        );
        (dir, camera)
    }

    #[tokio::test]
    async fn test_capture_frames_with_injected_detector_produces_expected_embeddings() {
        let (_dir, camera) = for_test(
            FakeDetector::always_detects(default_face_region(640, 480)),
            FakeExtractor::with_vector(vec![1.0, 0.0, 0.0], 0.9),
        );

        let result = camera.capture_frames(3, 1000).await.unwrap();

        assert_eq!(result.frames.len(), 3);
        assert_eq!(result.embeddings.len(), 3);
        for emb in &result.embeddings {
            assert_eq!(emb.vector, vec![1.0, 0.0, 0.0]);
            assert!((emb.metadata.quality_score - 0.9).abs() < 1e-6);
        }
        assert!((result.quality_score - 0.9).abs() < 1e-6);
    }

    #[tokio::test]
    async fn test_capture_frames_with_never_detecting_detector_yields_empty_embeddings_and_zero_quality(
    ) {
        let (_dir, camera) = for_test(
            FakeDetector::never_detects(),
            FakeExtractor::with_vector(vec![1.0, 0.0, 0.0], 0.9),
        );

        let result = camera.capture_frames(2, 1000).await.unwrap();

        assert_eq!(result.embeddings.len(), 2);
        for emb in &result.embeddings {
            assert!(emb.vector.is_empty());
            assert_eq!(emb.metadata.quality_score, 0.0);
            assert_eq!(emb.metadata.model, "no-face");
        }
        assert_eq!(result.quality_score, 0.0);
    }

    #[tokio::test]
    async fn test_capture_frames_averages_mixed_per_frame_quality_scores() {
        // Alternating detector: hit, miss, hit, miss (calls counted from 0).
        let (_dir, camera) = for_test(
            FakeDetector::alternating(default_face_region(640, 480)),
            FakeExtractor::with_vector(vec![1.0, 0.0, 0.0], 0.8),
        );

        let result = camera.capture_frames(4, 1000).await.unwrap();

        assert_eq!(result.embeddings.len(), 4);
        let scores: Vec<f32> = result
            .embeddings
            .iter()
            .map(|e| e.metadata.quality_score)
            .collect();
        assert_eq!(scores, vec![0.8, 0.0, 0.8, 0.0]);
        assert!(
            (result.quality_score - 0.4).abs() < 1e-6,
            "expected the mean of [0.8, 0, 0.8, 0] = 0.4, got {}",
            result.quality_score
        );
    }

    #[tokio::test]
    async fn test_capture_frames_num_frames_controls_frame_and_embedding_count() {
        for num_frames in [1u32, 5u32] {
            let (_dir, camera) = for_test(
                FakeDetector::always_detects(default_face_region(640, 480)),
                FakeExtractor::with_vector(vec![1.0, 0.0, 0.0], 0.9),
            );
            let result = camera.capture_frames(num_frames, 1000).await.unwrap();
            assert_eq!(result.frames.len(), num_frames as usize);
            assert_eq!(result.embeddings.len(), num_frames as usize);
        }
    }

    #[tokio::test]
    async fn test_capture_until_with_unavailable_device_never_invokes_callback() {
        // Locks in a real asymmetry with capture_frames: unlike
        // capture_frames (which pads with stub frames on a capture
        // failure), capture_rgb_stream_until never invokes its callback at
        // all when the device can't be opened. verify()'s consecutive-
        // match orchestration is covered separately via score_frame +
        // matcher::match_with_liveness + record_frame_result directly,
        // precisely because of this.
        let (_dir, camera) = for_test(
            FakeDetector::always_detects(default_face_region(640, 480)),
            FakeExtractor::with_vector(vec![1.0, 0.0, 0.0], 0.9),
        );

        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let calls_clone = calls.clone();
        let result = camera
            .capture_until(200, move |_embedding, _ir, _rgb| {
                calls_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                false
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn test_score_frame_returns_none_when_no_face_detected() {
        let detector = FakeDetector::never_detects();
        let extractor = FakeExtractor::with_vector(vec![1.0, 0.0, 0.0], 0.9);
        let frame = blank_rgb_frame(64, 64);

        let result = score_frame(&detector, &extractor, 0, &frame, 64, 64);
        assert!(result.is_none());
    }

    #[test]
    fn test_score_frame_returns_embedding_and_rgb_liveness_when_face_detected() {
        let detector = FakeDetector::always_detects(default_face_region(64, 64));
        let extractor = FakeExtractor::with_vector(vec![0.6, 0.8, 0.0], 0.95);
        let frame = blank_rgb_frame(64, 64);

        let (embedding, rgb_liveness) =
            score_frame(&detector, &extractor, 0, &frame, 64, 64).expect("face should be found");
        assert_eq!(embedding.vector, vec![0.6, 0.8, 0.0]);
        assert!((0.0..=1.0).contains(&rgb_liveness));
    }

    // start_capture_stream uses tokio::task::block_in_place internally,
    // which requires a multi-threaded runtime (the default #[tokio::test]
    // is single-threaded).
    #[tokio::test(flavor = "multi_thread")]
    async fn test_start_capture_stream_simulation_fallback_emits_expected_frame_count_and_shape() {
        let (_dir, camera) = for_test(
            FakeDetector::never_detects(),
            FakeExtractor::with_vector(vec![], 0.0),
        );

        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let events_clone = events.clone();
        let result = camera
            .start_capture_stream(2, 5000, move |event| {
                events_clone.lock().unwrap().push(event);
            })
            .await;

        assert!(result.is_ok());
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 2);
        for event in events.iter() {
            assert_eq!(event.width, 640);
            assert_eq!(event.height, 480);
            assert_eq!(event.frame_data.len(), 640 * 480 * 3);
            assert!(!event.face_detected);
        }
    }
}
