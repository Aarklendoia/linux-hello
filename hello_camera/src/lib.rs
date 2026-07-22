//! Multi-backend camera access abstraction (V4L2, PipeWire)

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
/// Result of classifying a single device from its reported name and
/// supported pixel formats — the decision `scan_cameras` makes once per
/// `/dev/videoN`, pulled out as a pure function (no device/ioctl needed) so
/// it's directly testable with plain strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeviceKind {
    Ir,
    Rgb,
    Neither,
}

/// Classify a device from its `card` name and supported format list.
///
/// IR criteria (either one): the name contains "ir"/"infrared"
/// (case-insensitive *substring* match, not word-boundary — e.g. a
/// hypothetical camera named "Circle Cam" would also match on "ir"; kept
/// as-is, unchanged from before this was extracted), or it only supports
/// GREY/Y800 (no YUYV/MJPG/RGB). Otherwise RGB if a color format is
/// present, else neither.
fn classify_device(card_name: &str, formats: &[String]) -> DeviceKind {
    let is_ir_by_name = {
        let card = card_name.to_lowercase();
        card.contains("ir") || card.contains("infrared")
    };
    let has_grey = formats
        .iter()
        .any(|f| f.contains("GREY") || f.contains("Y800"));
    let has_color = formats
        .iter()
        .any(|f| f.contains("YUYV") || f.contains("MJPG") || f.contains("RGB"));

    if is_ir_by_name || (has_grey && !has_color) {
        DeviceKind::Ir
    } else if has_color {
        DeviceKind::Rgb
    } else {
        DeviceKind::Neither
    }
}

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

        let card_name = v4l::Device::query_caps(&dev)
            .map(|caps| caps.card)
            .unwrap_or_default();

        let formats: Vec<String> = dev
            .enum_formats()
            .unwrap_or_default()
            .iter()
            .map(|f| f.fourcc.str().unwrap_or_default().to_string())
            .collect();

        match classify_device(&card_name, &formats) {
            DeviceKind::Ir if ir.is_none() => {
                tracing::info!(
                    "IR camera detected: {} (formats: {})",
                    path,
                    formats.join(", ")
                );
                ir = Some(path);
            }
            DeviceKind::Rgb if rgb.is_none() => {
                tracing::info!(
                    "RGB camera detected: {} (formats: {})",
                    path,
                    formats.join(", ")
                );
                rgb = Some(path);
            }
            _ => {}
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

/// Opens `device_path` and negotiates 640x480 capture in the given FourCC
/// pixel format — shared by capture_gray_stream_v4l2/capture_rgb_stream_v4l2/
/// capture_rgb_stream_until, which each repeated this exact open/negotiate
/// sequence, differing only in the FourCC.
#[cfg(feature = "v4l2")]
fn open_and_configure(
    device_path: &str,
    fourcc: &[u8; 4],
) -> Result<(v4l::Device, v4l::Format), CameraError> {
    use v4l::video::Capture;

    let dev = v4l::Device::with_path(device_path)
        .map_err(|e| CameraError::NotAvailable(format!("{}: {}", device_path, e)))?;

    let mut fmt = dev
        .format()
        .map_err(|e| CameraError::OpenFailed(e.to_string()))?;
    fmt.width = 640;
    fmt.height = 480;
    fmt.fourcc = v4l::format::FourCC::new(fourcc);

    let applied = dev.set_format(&fmt).map_err(|e| {
        CameraError::OpenFailed(format!(
            "set_format {}: {}",
            String::from_utf8_lossy(fourcc),
            e
        ))
    })?;

    Ok((dev, applied))
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

    let (dev, applied) = open_and_configure(device_path, b"GREY")?;
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

/// Capture `num_frames` RGB frames from a V4L2 device using a single
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

    let (dev, applied) = open_and_configure(device_path, b"YUYV")?;
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

/// Capture RGB frames continuously from a **single, persistently open**
/// V4L2 stream, invoking `on_frame` for each one and stopping as soon as it
/// returns `true` ("I've decided, stop") or the wall-clock `timeout_ms`
/// deadline is reached — whichever comes first.
///
/// Unlike `capture_rgb_stream_v4l2` (kept as-is for enrollment's fixed
/// sample-count use case), this is for the verify path: it keeps the device
/// open for the whole attempt window so the camera's activity LED stays
/// steadily on instead of blinking on/off on every repeated call.
///
/// Explicitly sets the underlying `v4l::Stream`'s per-dequeue poll timeout
/// (left unset — i.e. block forever — by the other capture functions here):
/// without it, a single stalled `stream.next()` isn't actually bounded by
/// `timeout_ms` at all, since the deadline is only checked *before* starting
/// a new dequeue, not during one already in flight.
#[cfg(feature = "v4l2")]
pub fn capture_rgb_stream_until<F>(
    device_path: &str,
    timeout_ms: u64,
    mut on_frame: F,
) -> Result<(), CameraError>
where
    F: FnMut(Vec<u8>, u32, u32) -> bool,
{
    use v4l::buffer::Type;
    use v4l::io::traits::CaptureStream;

    let (dev, applied) = open_and_configure(device_path, b"YUYV")?;
    let width = applied.width;
    let height = applied.height;

    let mut stream = v4l::io::mmap::Stream::with_buffers(&dev, Type::VideoCapture, 4)
        .map_err(|e| CameraError::CaptureFailed(format!("Stream creation error: {}", e)))?;
    // Per-dequeue bound, well above the observed ~220ms/frame rate, so a
    // stalled frame can't silently defeat the overall deadline below.
    stream.set_timeout(std::time::Duration::from_millis(2000));

    let start = std::time::Instant::now();
    let deadline = std::time::Duration::from_millis(timeout_ms);

    while start.elapsed() < deadline {
        match stream.next() {
            Ok((buf, _meta)) => {
                let rgb = yuyv_to_rgb_strided(buf, width, height, applied.stride);
                if on_frame(rgb, width, height) {
                    return Ok(());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(e) => return Err(CameraError::CaptureFailed(format!("Capture error: {}", e))),
        }
    }

    Ok(())
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
    fn test_yuyv_to_rgb_strided_gray_input_is_gray_output() {
        // Y=128, U=128, V=128 -> u=v=0 -> R=G=B=Y for both pixels.
        let data = [128u8, 128, 128, 128];
        let rgb = yuyv_to_rgb_strided(&data, 2, 1, 4);
        assert_eq!(rgb, vec![128, 128, 128, 128, 128, 128]);
    }

    #[test]
    fn test_yuyv_to_rgb_strided_applies_chrominance() {
        // Y=128, U=128 (u=0), V=200 (v=72): R=128+100=228, G=128-0-51=77, B=128.
        let data = [128u8, 128, 128, 200];
        let rgb = yuyv_to_rgb_strided(&data, 2, 1, 4);
        assert_eq!(rgb, vec![228, 77, 128, 228, 77, 128]);
    }

    #[test]
    fn test_yuyv_to_rgb_strided_skips_row_padding() {
        // stride=6 > row_bytes=4: 2 padding bytes per row that must be
        // skipped, not fed into the conversion, and must not misalign the
        // next row's start offset.
        #[rustfmt::skip]
        let data = [
            128u8, 128, 128, 128, 9, 9, // row 0: gray, then 2 padding bytes
            200u8, 128, 200, 128, 9, 9, // row 1: brighter gray, then padding
        ];
        let rgb = yuyv_to_rgb_strided(&data, 2, 2, 6);
        assert_eq!(
            rgb,
            vec![
                128, 128, 128, 128, 128, 128, // row 0
                200, 200, 200, 200, 200, 200, // row 1
            ]
        );
    }

    #[test]
    fn test_yuyv_to_rgb_strided_stops_early_on_a_truncated_buffer() {
        // Declares height=3 but only supplies enough data for 2 rows —
        // matching a truncated/mismatched V4L2 capture. Must return the
        // rows it actually has instead of panicking on an out-of-bounds
        // slice.
        let data = [128u8, 128, 128, 128, 128, 128, 128, 128]; // 2 rows worth
        let rgb = yuyv_to_rgb_strided(&data, 2, 3, 4);
        assert_eq!(rgb.len(), 2 * 2 * 3, "only 2 of 3 rows should be produced");
    }

    #[test]
    fn test_frame_validate_accepts_correctly_sized_rgb8_and_gray8() {
        let rgb = Frame {
            data: vec![0u8; 4 * 2 * 3],
            width: 4,
            height: 2,
            format: FrameFormat::Rgb8,
            timestamp_ms: 0,
        };
        assert!(rgb.validate().is_ok());

        let gray = Frame {
            data: vec![0u8; 4 * 2],
            width: 4,
            height: 2,
            format: FrameFormat::Gray8,
            timestamp_ms: 0,
        };
        assert!(gray.validate().is_ok());
    }

    #[test]
    fn test_frame_validate_rejects_a_mismatched_buffer_size() {
        let frame = Frame {
            data: vec![0u8; 10], // too short for 4x2 Rgb8 (needs 24)
            width: 4,
            height: 2,
            format: FrameFormat::Rgb8,
            timestamp_ms: 0,
        };
        assert!(frame.validate().is_err());
    }

    #[test]
    fn test_frame_validate_accepts_any_size_for_mjpeg() {
        let frame = Frame {
            data: vec![0u8; 7], // arbitrary — MJPEG is variable-size
            width: 4,
            height: 2,
            format: FrameFormat::MjPeg,
            timestamp_ms: 0,
        };
        assert!(frame.validate().is_ok());
    }

    #[test]
    fn test_classify_device_ir_by_name() {
        // Real card name from this session's hardware (HP IR Camera),
        // reporting the same GREY-only format set an IR sensor typically
        // does — name-based classification should still be what fires.
        assert_eq!(
            classify_device("HP IR Camera", &["GREY".to_string()]),
            DeviceKind::Ir
        );
        // Case-insensitive, and the "infrared" spelling.
        assert_eq!(
            classify_device("Some Infrared Sensor", &["YUYV".to_string()]),
            DeviceKind::Ir
        );
    }

    #[test]
    fn test_classify_device_ir_by_format_only_when_grey_and_no_color() {
        assert_eq!(
            classify_device("Generic Camera", &["GREY".to_string()]),
            DeviceKind::Ir
        );
        assert_eq!(
            classify_device("Generic Camera", &["Y800".to_string()]),
            DeviceKind::Ir
        );
    }

    #[test]
    fn test_classify_device_rgb_when_color_format_present_and_not_ir_named() {
        assert_eq!(
            classify_device("HP 5MP Camera", &["YUYV".to_string()]),
            DeviceKind::Rgb
        );
        assert_eq!(
            classify_device("Generic Webcam", &["MJPG".to_string()]),
            DeviceKind::Rgb
        );
    }

    #[test]
    fn test_classify_device_neither_without_grey_or_color() {
        assert_eq!(
            classify_device("Unknown Device", &["SOMETHING_ELSE".to_string()]),
            DeviceKind::Neither
        );
        assert_eq!(classify_device("Unknown Device", &[]), DeviceKind::Neither);
    }

    #[test]
    fn test_classify_device_ir_name_wins_even_with_a_color_format_present() {
        // A device claiming to be IR by name but also reporting a color
        // format (unusual, but the name check is checked first/is an "OR")
        // still classifies as IR — matches scan_cameras's original
        // if/else-if precedence before this was extracted.
        assert_eq!(
            classify_device("HP IR Camera", &["YUYV".to_string()]),
            DeviceKind::Ir
        );
    }

    #[test]
    fn test_classify_device_grey_and_color_together_is_rgb_not_ir() {
        // has_grey && !has_color is the IR-by-format condition — a device
        // reporting BOTH isn't classified as IR on format grounds alone
        // (only a name match could make it IR in that case).
        assert_eq!(
            classify_device("Generic Camera", &["GREY".to_string(), "YUYV".to_string()]),
            DeviceKind::Rgb
        );
    }
}
