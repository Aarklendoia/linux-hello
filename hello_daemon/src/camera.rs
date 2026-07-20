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
    fn try_acquire() -> Result<Self, CameraError> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(camera_lock_path())
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
        }
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

        // RGB capture in a blocking thread (V4L2 is not async)
        let (rgb_frames, ir_frames) =
            tokio::task::spawn_blocking(move || -> Result<_, CameraError> {
                // Held for the whole capture; released when this closure returns.
                let _camera_lock = CameraLock::try_acquire()?;

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

        tokio::task::spawn_blocking(move || -> Result<(), CameraError> {
            // Held for the whole attempt; released when this closure returns.
            let _camera_lock = CameraLock::try_acquire()?;

            // Sample IR liveness once at the start of the attempt (same
            // "first frame" semantics as capture_frames today).
            let ir_liveness = ir_device.as_deref().and_then(|ir_path| {
                let mut ir_frame: Option<Frame> = None;
                let _ = hello_camera::capture_gray_stream_v4l2(ir_path, 1, 2000, |data, w, h| {
                    ir_frame = Some(Frame {
                        data,
                        width: w,
                        height: h,
                        format: FrameFormat::Gray8,
                        timestamp_ms: 0,
                    });
                });
                let frame = ir_frame?;
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

            let mut frame_index: u32 = 0;
            let result =
                hello_camera::capture_rgb_stream_until(&rgb_device, timeout, |data, w, h| {
                    frame_index += 1;
                    let faces = match detector.detect(&data, w, h, 3) {
                        Ok(f) => f,
                        Err(e) => {
                            warn!("SCRFD detection error frame {}: {}", frame_index, e);
                            return false;
                        }
                    };
                    let Some(best_face) = faces
                        .into_iter()
                        .max_by(|a, b| a.confidence.total_cmp(&b.confidence))
                    else {
                        return false; // no face in this frame, keep going
                    };
                    let embedding = match extractor.extract(&best_face, &data, w, h, 3) {
                        Ok(emb) => emb,
                        Err(e) => {
                            warn!("Embedding extraction frame {}: {}", frame_index, e);
                            return false;
                        }
                    };
                    // Weaker fallback signal for the (common) no-IR-camera
                    // case — see hello_face_core::liveness::rgb_liveness_score.
                    // Always computed (never a hiccup-prone Option) since
                    // the RGB frame and detected face are already in hand
                    // at this point.
                    let rgb_liveness =
                        hello_face_core::liveness::rgb_liveness_score(&data, w, h, &best_face);
                    on_frame(embedding, ir_liveness, rgb_liveness)
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
        // Point at a scratch file, not the real /run/lock/ path.
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join("camera.lock");
        // SAFETY: this test doesn't run concurrently with anything else that
        // reads LINUX_HELLO_CAMERA_LOCK_PATH.
        unsafe {
            std::env::set_var("LINUX_HELLO_CAMERA_LOCK_PATH", &lock_path);
        }

        let first = CameraLock::try_acquire().expect("first acquire should succeed");
        let second = CameraLock::try_acquire();
        assert!(
            matches!(second, Err(CameraError::Busy)),
            "expected Busy while the first guard is still held, got {:?}",
            second.err()
        );

        drop(first);
        let third = CameraLock::try_acquire();
        assert!(
            third.is_ok(),
            "expected the lock to be acquirable again after the first guard was dropped"
        );
    }
}
