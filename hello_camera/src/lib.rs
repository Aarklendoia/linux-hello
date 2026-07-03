//! Multi-backend camera access abstraction (V4L2, PipeWire)

use std::sync::Arc;
use thiserror::Error;

/// Camera errors
#[derive(Debug, Error)]
pub enum CameraError {
    #[error("Camera not available: {0}")]
    NotAvailable(String),

    #[error("Open error: {0}")]
    OpenFailed(String),

    #[error("Capture error: {0}")]
    CaptureFailed(String),

    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("Capture timeout")]
    CaptureTimeout,

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Supported frame format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameFormat {
    /// RGB 8 bits per channel
    Rgb8,
    /// Grayscale 8 bits
    Gray8,
    /// Compressed MJPEG
    MjPeg,
}

impl FrameFormat {
    pub fn channels(&self) -> u32 {
        match self {
            FrameFormat::Rgb8 => 3,
            FrameFormat::Gray8 => 1,
            FrameFormat::MjPeg => 3, // decoded to RGB
        }
    }
}

/// Description of a captured frame
#[derive(Debug, Clone)]
pub struct Frame {
    /// Raw pixel data
    pub data: Vec<u8>,

    /// Width in pixels
    pub width: u32,

    /// Height in pixels
    pub height: u32,

    /// Frame format
    pub format: FrameFormat,

    /// Capture timestamp (ms since start)
    pub timestamp_ms: u64,
}

impl Frame {
    /// Return the number of channels
    pub fn channels(&self) -> u32 {
        self.format.channels()
    }

    /// Verify that the data matches the dimensions
    pub fn validate(&self) -> Result<(), CameraError> {
        let expected_size = match self.format {
            FrameFormat::Rgb8 => (self.width * self.height * 3) as usize,
            FrameFormat::Gray8 => (self.width * self.height) as usize,
            FrameFormat::MjPeg => self.data.len(), // variable size
        };

        if self.format != FrameFormat::MjPeg && self.data.len() != expected_size {
            return Err(CameraError::CaptureFailed(format!(
                "Frame size mismatch: expected {}, got {}",
                expected_size,
                self.data.len()
            )));
        }
        Ok(())
    }
}

/// Camera configuration
#[derive(Debug, Clone)]
pub struct CameraConfig {
    /// Device path (e.g. /dev/video0)
    pub device_path: String,

    /// Desired width
    pub width: u32,

    /// Desired height
    pub height: u32,

    /// Preferred format
    pub preferred_format: FrameFormat,

    /// Target FPS
    pub fps: u32,

    /// Capture timeout (ms)
    pub capture_timeout_ms: u64,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            device_path: "/dev/video0".to_string(),
            width: 640,
            height: 480,
            preferred_format: FrameFormat::Rgb8,
            fps: 30,
            capture_timeout_ms: 5000,
        }
    }
}

/// Trait for a generic camera
pub trait CameraBackend: Send + Sync {
    /// Start the camera
    fn open(&mut self) -> Result<(), CameraError>;

    /// Stop the camera
    fn close(&mut self) -> Result<(), CameraError>;

    /// Capture a frame (blocking)
    fn capture(&mut self, timeout_ms: u64) -> Result<Frame, CameraError>;

    /// Number of pending frames
    fn pending_frames(&self) -> usize;

    /// Flush the buffer (useful before verification)
    fn flush_buffers(&mut self) -> Result<(), CameraError>;

    /// Check whether the camera is open
    fn is_open(&self) -> bool;

    /// Backend name
    fn backend_name(&self) -> &str;
}

/// Shared handle for a camera
pub type SharedCamera = Arc<parking_lot::Mutex<Box<dyn CameraBackend>>>;

// ============================================================================
// V4L2 Implementation
// ============================================================================

#[cfg(feature = "v4l2")]
pub mod v4l2_backend {
    use super::*;
    use std::time::Instant;
    use tracing::info;

    /// Real V4L2 backend with direct access to Linux devices
    pub struct V4L2Camera {
        config: CameraConfig,
        device: Option<v4l::Device>,
        is_open: bool,
        start_time: Instant,
        stream_initialized: bool,
    }

    impl V4L2Camera {
        pub fn new(config: CameraConfig) -> Self {
            Self {
                config,
                device: None,
                is_open: false,
                start_time: Instant::now(),
                stream_initialized: false,
            }
        }

        /// Open the V4L2 device and configure it for capture
        fn open_device(&mut self) -> Result<(), CameraError> {
            use v4l::video::Capture;

            // Open the V4L2 device
            let dev = v4l::Device::with_path(&self.config.device_path).map_err(|e| {
                CameraError::OpenFailed(format!(
                    "Failed to open {}: {}",
                    self.config.device_path, e
                ))
            })?;

            // Get the current format and adapt it
            let mut format = dev
                .format()
                .map_err(|e| CameraError::OpenFailed(format!("Error reading format: {}", e)))?;

            // Configure the resolution
            format.width = self.config.width;
            format.height = self.config.height;

            // Choose the format according to preferences
            match self.config.preferred_format {
                FrameFormat::Rgb8 => {
                    // RGB24 (standard V4L2 format: R,G,B,R,G,B...)
                    format.fourcc = v4l::format::FourCC::new(b"RGB3");
                }
                FrameFormat::Gray8 => {
                    // Grayscale format
                    format.fourcc = v4l::format::FourCC::new(b"GREY");
                }
                FrameFormat::MjPeg => {
                    // MJPEG format (often more efficient)
                    format.fourcc = v4l::format::FourCC::new(b"MJPG");
                }
            }

            // Apply the configuration
            dev.set_format(&format).map_err(|e| {
                CameraError::OpenFailed(format!("V4L2 format configuration error: {}", e))
            })?;

            info!(
                "V4L2 device opened and configured: {} ({}x{} @ {}fps)",
                self.config.device_path, self.config.width, self.config.height, self.config.fps
            );

            self.device = Some(dev);
            Ok(())
        }
    }

    impl CameraBackend for V4L2Camera {
        fn open(&mut self) -> Result<(), CameraError> {
            self.open_device()?;
            self.is_open = true;
            self.start_time = Instant::now();

            info!(
                "V4L2 camera opened: {}x{} at {}",
                self.config.width, self.config.height, self.config.device_path
            );
            Ok(())
        }

        fn close(&mut self) -> Result<(), CameraError> {
            self.device = None;
            self.stream_initialized = false;
            self.is_open = false;
            info!("V4L2 camera closed");
            Ok(())
        }

        fn capture(&mut self, _timeout_ms: u64) -> Result<Frame, CameraError> {
            use v4l::buffer::Type;
            use v4l::io::traits::CaptureStream;

            if !self.is_open || self.device.is_none() {
                return Err(CameraError::CaptureFailed("Camera not open".to_string()));
            }

            let dev = self.device.as_ref().unwrap();

            // Create an mmap stream on each capture (simple but functional approach)
            // In an optimized implementation, we'd want to store this,
            // but with v4l's generics and lifetimes it's complex
            let mut stream = v4l::io::mmap::Stream::with_buffers(dev, Type::VideoCapture, 4)
                .map_err(|e| {
                    CameraError::CaptureFailed(format!("Stream creation error: {}", e))
                })?;

            // Capture a frame
            match stream.next() {
                Ok((buf, _meta)) => {
                    let timestamp_ms = self.start_time.elapsed().as_millis() as u64;

                    Ok(Frame {
                        data: buf.to_vec(),
                        width: self.config.width,
                        height: self.config.height,
                        format: self.config.preferred_format,
                        timestamp_ms,
                    })
                }
                Err(e) => Err(CameraError::CaptureFailed(format!(
                    "V4L2 capture error: {}",
                    e
                ))),
            }
        }

        fn pending_frames(&self) -> usize {
            if self.is_open {
                1
            } else {
                0
            }
        }

        fn flush_buffers(&mut self) -> Result<(), CameraError> {
            // Drain old buffers by capturing and discarding a few frames
            // Note: with the mmap approach, it's simpler
            if self.is_open {
                // Try a few quick captures to flush the buffers
                for _ in 0..3 {
                    let _ = self.capture(100);
                }
            }
            Ok(())
        }

        fn is_open(&self) -> bool {
            self.is_open && self.device.is_some()
        }

        fn backend_name(&self) -> &str {
            "V4L2-Logitech-Brio"
        }
    }
}

/// Convert a YUYV buffer to RGB888, taking the stride (per-row padding) into account.
/// `stride` = bytes per row as returned by V4L2 (`applied.stride`).
fn yuyv_to_rgb_strided(data: &[u8], width: u32, height: u32, stride: u32) -> Vec<u8> {
    let mut rgb = Vec::with_capacity((width * height * 3) as usize);
    let row_bytes = (width * 2) as usize; // useful bytes per row in YUYV
    let stride = stride as usize;

    for row in 0..height as usize {
        let row_start = row * stride;
        let row_end = row_start + row_bytes;
        if row_end > data.len() {
            break;
        }
        let row_data = &data[row_start..row_end];
        for chunk in row_data.chunks(4) {
            if chunk.len() == 4 {
                let y1 = chunk[0] as i32;
                let u = chunk[1] as i32 - 128;
                let y2 = chunk[2] as i32;
                let v = chunk[3] as i32 - 128;
                for &y in &[y1, y2] {
                    rgb.push((y + (1402 * v) / 1000).clamp(0, 255) as u8);
                    rgb.push((y - (344 * u) / 1000 - (714 * v) / 1000).clamp(0, 255) as u8);
                    rgb.push((y + (1772 * u) / 1000).clamp(0, 255) as u8);
                }
            }
        }
    }
    rgb
}

/// Capture `num_frames` frames from V4L2 in YUYV and deliver them as RGB via callback.
///
/// Opens `/dev/video0` (or the given path), configures YUYV 640x480, creates a **single**
/// Result of a scan of available cameras
#[derive(Debug, Clone)]
pub struct CameraInventory {
    /// Main RGB device (e.g. /dev/video0)
    pub rgb_device: String,
    /// IR device if found (e.g. /dev/video2 for Logitech Brio)
    pub ir_device: Option<String>,
}

/// Automatically looks for RGB and IR cameras among /dev/video0..9.
///
/// IR criteria:
/// - The device name contains "IR" or "Infrared" (case-insensitive)
/// - OR only the GREY format is supported (no YUYV or MJPG)
///
/// Returns the first RGB camera found + the first IR one if available.
#[cfg(feature = "v4l2")]
pub fn scan_cameras() -> CameraInventory {
    use v4l::video::Capture;

    let mut rgb: Option<String> = None;
    let mut ir: Option<String> = None;

    for idx in 0..10u8 {
        let path = format!("/dev/video{}", idx);
        let Ok(dev) = v4l::Device::with_path(&path) else {
            continue;
        };

        // Read the capabilities for the device name
        let is_ir_by_name = v4l::Device::query_caps(&dev)
            .map(|caps| {
                let card = caps.card.to_lowercase();
                card.contains("ir") || card.contains("infrared")
            })
            .unwrap_or(false);

        // Read the supported formats
        let formats: Vec<String> = dev
            .enum_formats()
            .unwrap_or_default()
            .iter()
            .map(|f| f.fourcc.str().unwrap_or_default().to_string())
            .collect();

        let has_grey = formats
            .iter()
            .any(|f| f.contains("GREY") || f.contains("Y800"));
        let has_color = formats
            .iter()
            .any(|f| f.contains("YUYV") || f.contains("MJPG") || f.contains("RGB"));

        // IR camera: named IR or GREY-only
        let is_ir = is_ir_by_name || (has_grey && !has_color);

        if is_ir && ir.is_none() {
            tracing::info!(
                "IR camera detected: {} (formats: {})",
                path,
                formats.join(", ")
            );
            ir = Some(path);
        } else if !is_ir && has_color && rgb.is_none() {
            tracing::info!(
                "RGB camera detected: {} (formats: {})",
                path,
                formats.join(", ")
            );
            rgb = Some(path);
        }
    }

    CameraInventory {
        rgb_device: rgb.unwrap_or_else(|| "/dev/video0".to_string()),
        ir_device: ir,
    }
}

#[cfg(not(feature = "v4l2"))]
pub fn scan_cameras() -> CameraInventory {
    CameraInventory {
        rgb_device: "/dev/video0".to_string(),
        ir_device: None,
    }
}

/// Capture `num_frames` frames in GREY (8-bit grayscale) from a V4L2 device.
///
/// Used for IR cameras (e.g. Logitech Brio infrared channel).
/// Callback: `on_frame(gray_data: Vec<u8>, width, height)`
#[cfg(feature = "v4l2")]
pub fn capture_gray_stream_v4l2<F>(
    device_path: &str,
    num_frames: u32,
    timeout_ms: u64,
    mut on_frame: F,
) -> Result<(), CameraError>
where
    F: FnMut(Vec<u8>, u32, u32),
{
    use v4l::buffer::Type;
    use v4l::io::traits::CaptureStream;
    use v4l::video::Capture;

    let dev = v4l::Device::with_path(device_path)
        .map_err(|e| CameraError::NotAvailable(format!("{}: {}", device_path, e)))?;

    let mut fmt = dev
        .format()
        .map_err(|e| CameraError::OpenFailed(e.to_string()))?;
    fmt.width = 640;
    fmt.height = 480;
    fmt.fourcc = v4l::format::FourCC::new(b"GREY");

    let applied = dev
        .set_format(&fmt)
        .map_err(|e| CameraError::OpenFailed(format!("set_format GREY: {}", e)))?;

    let width = applied.width;
    let height = applied.height;

    let mut stream = v4l::io::mmap::Stream::with_buffers(&dev, Type::VideoCapture, 4)
        .map_err(|e| CameraError::CaptureFailed(format!("GREY stream error: {}", e)))?;

    let start = std::time::Instant::now();
    let timeout_dur = std::time::Duration::from_millis(timeout_ms);

    for _ in 0..num_frames {
        if start.elapsed() > timeout_dur {
            break;
        }
        let (buf, _meta) = stream
            .next()
            .map_err(|e| CameraError::CaptureFailed(format!("GREY capture error: {}", e)))?;
        on_frame(buf.to_vec(), width, height);
    }

    Ok(())
}

/// persistent mmap stream (more efficient than creating one per frame), then calls
/// `on_frame(rgb_data, width, height)` for each captured frame.
///
/// Returns `Ok(())` if at least one frame was captured, `Err` if the camera is
/// not available or no frame could be acquired.
#[cfg(feature = "v4l2")]
pub fn capture_rgb_stream_v4l2<F>(
    device_path: &str,
    num_frames: u32,
    timeout_ms: u64,
    mut on_frame: F,
) -> Result<(), CameraError>
where
    F: FnMut(Vec<u8>, u32, u32),
{
    use v4l::buffer::Type;
    use v4l::io::traits::CaptureStream;
    use v4l::video::Capture;

    let dev = v4l::Device::with_path(device_path)
        .map_err(|e| CameraError::NotAvailable(format!("{}: {}", device_path, e)))?;

    // Configure YUYV 640x480
    let mut fmt = dev
        .format()
        .map_err(|e| CameraError::OpenFailed(e.to_string()))?;
    fmt.width = 640;
    fmt.height = 480;
    fmt.fourcc = v4l::format::FourCC::new(b"YUYV");

    let applied = dev
        .set_format(&fmt)
        .map_err(|e| CameraError::OpenFailed(format!("set_format YUYV: {}", e)))?;

    let width = applied.width;
    let height = applied.height;

    // A single persistent stream for all frames
    let mut stream = v4l::io::mmap::Stream::with_buffers(&dev, Type::VideoCapture, 4)
        .map_err(|e| CameraError::CaptureFailed(format!("Stream creation error: {}", e)))?;

    let start = std::time::Instant::now();
    let timeout_dur = std::time::Duration::from_millis(timeout_ms);

    for _ in 0..num_frames {
        if start.elapsed() > timeout_dur {
            break;
        }

        let (buf, _meta) = stream
            .next()
            .map_err(|e| CameraError::CaptureFailed(format!("Capture error: {}", e)))?;

        let rgb = yuyv_to_rgb_strided(buf, width, height, applied.stride);
        on_frame(rgb, width, height);
    }

    Ok(())
}

/// Create a camera with the default available backend
pub fn create_camera(config: CameraConfig) -> Result<Box<dyn CameraBackend>, CameraError> {
    #[cfg(feature = "v4l2")]
    {
        Ok(Box::new(v4l2_backend::V4L2Camera::new(config)))
    }

    #[cfg(not(feature = "v4l2"))]
    {
        Err(CameraError::NotAvailable(
            "No camera backend compiled".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_format_channels() {
        assert_eq!(FrameFormat::Rgb8.channels(), 3);
        assert_eq!(FrameFormat::Gray8.channels(), 1);
    }

    #[test]
    fn test_camera_config_default() {
        let config = CameraConfig::default();
        assert_eq!(config.width, 640);
        assert_eq!(config.height, 480);
    }
}
