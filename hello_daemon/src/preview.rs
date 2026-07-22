//! Video preview export functions - MJPEG HTTP server
//!
//! Encodes V4L2 frames to JPEG and streams them in real time
//! via a multipart HTTP server on 127.0.0.1:17823.

use image::{ImageBuffer, Rgb};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use tokio::sync::broadcast;

/// Fixed port of the MJPEG server (loopback only)
pub const MJPEG_PORT: u16 = 17823;

/// Broadcast channel: capacity 1 = always keep the most recent frame.
static MJPEG_TX: OnceLock<broadcast::Sender<Vec<u8>>> = OnceLock::new();

/// Latest encoded frame, for single-shot snapshot requests (GET /snapshot).
/// QtMultimedia's MediaPlayer cannot demux a raw multipart/x-mixed-replace
/// stream, so the GUI polls single JPEGs instead of playing the MJPEG feed.
static LATEST_JPEG: OnceLock<Mutex<Option<Vec<u8>>>> = OnceLock::new();

/// EMA-smoothed state of the detection rectangle: (x, y, w, h) as f32.
type SmoothBoxState = Mutex<Option<(f32, f32, f32, f32)>>;
static SMOOTH_BOX: OnceLock<SmoothBoxState> = OnceLock::new();

/// Generates a random 64-hex-char token, the same way
/// `linux_hello_config`'s control server does (reads 32 bytes directly from
/// /dev/urandom via `read_exact`, not `fs::read` — the latter would block
/// forever on a character device that never returns EOF).
fn generate_mjpeg_token() -> String {
    use std::io::Read;
    let mut buf = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut buf))
        .expect("Unable to read /dev/urandom for the MJPEG server token");
    buf.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Path of the file the GUI reads to learn the current MJPEG token — under
/// `$XDG_RUNTIME_DIR` (mode 0700, owned solely by this UID), not `/tmp`:
/// see `linux_hello_config::main::runtime_dir`'s doc comment for why a
/// shared, sticky, world-writable directory is the wrong choice for a file
/// like this (a different-uid attacker could plant it there first and the
/// legitimate writer could never reclaim the path).
fn mjpeg_token_file_path() -> Option<String> {
    std::env::var("XDG_RUNTIME_DIR")
        .ok()
        .map(|dir| format!("{}/hello-daemon-mjpeg.token", dir))
}

/// Compares two strings in time that doesn't depend on *where* they first
/// differ (see `linux_hello_config::main::constant_time_eq`, which this
/// mirrors — the token's length isn't secret, always 64 hex chars by
/// construction, so the length check may still return early).
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Extracts the `token` query-string parameter from an HTTP request line
/// (e.g. `GET /snapshot?token=abc&t=123 HTTP/1.1`). QML's `Image { source }`
/// can't set custom request headers, so unlike the GUI's own control-server
/// token (sent as a header), this one has to travel in the URL.
fn extract_token_param(request_line: &str) -> Option<&str> {
    let query = request_line.split('?').nth(1)?;
    let query = query.split_whitespace().next().unwrap_or(query);
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix("token="))
}

/// Writes `contents` to `path` at mode 0600, owned by the current process.
/// Removes whatever is at `path` first, then creates fresh with
/// `create_new` (`O_CREAT|O_EXCL`) — see
/// `linux_hello_config::main::write_owner_only_file`, which this mirrors,
/// for the full reasoning (guards a stale/planted file at `path`, and
/// applies the mode atomically at creation rather than via a separate
/// `set_permissions` call that would leave a moment where the file exists
/// with default/umask permissions).
fn write_owner_only_file(path: &str, contents: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let _ = std::fs::remove_file(path);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(contents.as_bytes())
}

/// Start the MJPEG HTTP server on 127.0.0.1:17823.
/// Must be called only once at daemon startup (with an active tokio runtime).
pub async fn start_mjpeg_server() -> anyhow::Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let (tx, _) = broadcast::channel::<Vec<u8>>(1);
    // Ignore the error if `start_mjpeg_server` is called twice
    let _ = MJPEG_TX.set(tx.clone());

    // Gates every request below on this shared secret — without it, any
    // local process regardless of user could otherwise watch live video of
    // whoever is enrolling or authenticating (loopback TCP has no per-user
    // ACL of its own). Written once at startup; the GUI reads it back the
    // same way it already reads the daemon's other runtime-dir files.
    let token = generate_mjpeg_token();
    match mjpeg_token_file_path() {
        Some(path) => {
            if let Err(e) = write_owner_only_file(&path, &token) {
                tracing::warn!(
                    "Could not write MJPEG token file {}: {} (camera preview in the GUI won't authenticate)",
                    path,
                    e
                );
            }
        }
        None => tracing::warn!(
            "XDG_RUNTIME_DIR not set — can't write the MJPEG token file (camera preview in the GUI won't authenticate)"
        ),
    }

    let listener = TcpListener::bind(format!("127.0.0.1:{}", MJPEG_PORT)).await?;
    tracing::info!("MJPEG server started: http://127.0.0.1:{}", MJPEG_PORT);

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let mut rx = tx.subscribe();
                    let token = token.clone();
                    tokio::spawn(async move {
                        let (mut reader, mut writer) = stream.into_split();
                        // Read the incoming HTTP request line to route /snapshot vs the MJPEG feed
                        let mut buf = [0u8; 1024];
                        let n = reader.read(&mut buf).await.unwrap_or(0);
                        let request_line = String::from_utf8_lossy(&buf[..n]);

                        let request_token = extract_token_param(&request_line);
                        if request_token.map(|t| constant_time_eq(t, &token)) != Some(true) {
                            let _ = writer
                                .write_all(b"HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n")
                                .await;
                            return;
                        }

                        let is_snapshot = request_line.starts_with("GET /snapshot");

                        if is_snapshot {
                            let jpeg = LATEST_JPEG
                                .get_or_init(|| Mutex::new(None))
                                .lock()
                                .unwrap()
                                .clone();
                            match jpeg {
                                Some(jpeg) => {
                                    let headers = format!(
                                        "HTTP/1.1 200 OK\r\nContent-Type: image/jpeg\r\nCache-Control: no-cache\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                        jpeg.len()
                                    );
                                    let _ = writer.write_all(headers.as_bytes()).await;
                                    let _ = writer.write_all(&jpeg).await;
                                }
                                None => {
                                    let _ = writer
                                        .write_all(b"HTTP/1.1 503 Service Unavailable\r\nConnection: close\r\n\r\n")
                                        .await;
                                }
                            }
                            return;
                        }

                        // MJPEG multipart HTTP headers
                        let headers = b"HTTP/1.1 200 OK\r\nContent-Type: multipart/x-mixed-replace; boundary=frame\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n";
                        if writer.write_all(headers).await.is_err() {
                            return;
                        }

                        loop {
                            match rx.recv().await {
                                Ok(jpeg) => {
                                    let part = format!(
                                        "--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                                        jpeg.len()
                                    );
                                    if writer.write_all(part.as_bytes()).await.is_err() {
                                        break;
                                    }
                                    if writer.write_all(&jpeg).await.is_err() {
                                        break;
                                    }
                                    if writer.write_all(b"\r\n").await.is_err() {
                                        break;
                                    }
                                    let _ = writer.flush().await;
                                }
                                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                                Err(_) => break,
                            }
                        }
                    });
                }
                Err(e) => tracing::error!("MJPEG accept error: {}", e),
            }
        }
    });

    Ok(())
}

/// Convert a YUYV frame to RGB888
fn yuyv_to_rgb(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut rgb = Vec::with_capacity((width * height * 3) as usize);

    for chunk in data.chunks(4) {
        if chunk.len() == 4 {
            let y1 = chunk[0] as i32;
            let u = chunk[1] as i32 - 128;
            let y2 = chunk[2] as i32;
            let v = chunk[3] as i32 - 128;

            // Convert Y,U,V to R,G,B
            for y in [y1, y2].iter() {
                let r = (y + (1402 * v) / 1000).clamp(0, 255) as u8;
                let g = (y - (344 * u) / 1000 - (714 * v) / 1000).clamp(0, 255) as u8;
                let b = (y + (1772 * u) / 1000).clamp(0, 255) as u8;

                rgb.push(r);
                rgb.push(g);
                rgb.push(b);
            }
        }
    }

    rgb
}

/// Write a YUYV frame as JPEG
pub fn write_frame_preview(
    data: &[u8],
    width: u32,
    height: u32,
    path: &std::path::Path,
) -> anyhow::Result<()> {
    // Convert YUYV to RGB
    let rgb_data = yuyv_to_rgb(data, width, height);

    // Create an RGB image
    let img = ImageBuffer::<Rgb<u8>, Vec<u8>>::from_raw(width, height, rgb_data)
        .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;

    // Save as JPEG (85% quality)
    img.save(path)?;

    Ok(())
}

/// Write the preview for GUI display (data already in RGB)
pub fn export_preview_frame(data: &[u8], width: u32, height: u32) -> anyhow::Result<()> {
    let path = Path::new("/tmp/linux-hello-preview.jpg");
    write_frame_preview(data, width, height, path)
}

/// Write the preview for GUI display (data already in RGB, no conversion).
/// Detects the face via skin color (YCbCr) and draws a green rectangle around it.
pub fn export_preview_frame_rgb(
    mut pixels: Vec<u8>,
    width: u32,
    height: u32,
) -> anyhow::Result<()> {
    // Skin-color detection + EMA smoothing of the rectangle
    if let Some((rx, ry, rw, rh)) = detect_face_region(&pixels, width, height) {
        let lock = SMOOTH_BOX.get_or_init(|| Mutex::new(None));
        let (sx, sy, sw, sh) = {
            let mut g = lock.lock().unwrap();
            let (rx, ry, rw, rh) = (rx as f32, ry as f32, rw as f32, rh as f32);
            let smoothed = match *g {
                None => (rx, ry, rw, rh),
                Some((px, py, pw, ph)) => {
                    // Higher than before (0.35): that smoothing lagged
                    // enough behind head movement to visibly trail the
                    // actual position (reported as the box sitting
                    // offset/stale right after moving). Still smooths out
                    // single-frame jitter, just converges faster.
                    const ALPHA: f32 = 0.5;
                    (
                        px * (1.0 - ALPHA) + rx * ALPHA,
                        py * (1.0 - ALPHA) + ry * ALPHA,
                        pw * (1.0 - ALPHA) + rw * ALPHA,
                        ph * (1.0 - ALPHA) + rh * ALPHA,
                    )
                }
            };
            *g = Some(smoothed);
            (
                smoothed.0 as u32,
                smoothed.1 as u32,
                smoothed.2 as u32,
                smoothed.3 as u32,
            )
        };
        draw_rect_rgb(&mut pixels, width, sx, sy, sw, sh, [0, 220, 0]);
    } else if let Some(lock) = SMOOTH_BOX.get() {
        *lock.lock().unwrap() = None;
    }

    let img = ImageBuffer::<Rgb<u8>, Vec<u8>>::from_raw(width, height, pixels)
        .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;

    // Downscale to 320x240 to reduce latency (less JPEG data).
    let small = image::DynamicImage::ImageRgb8(img).resize_exact(
        320,
        240,
        image::imageops::FilterType::Triangle,
    );

    // Encode as JPEG quality 65 (good size/quality tradeoff for a preview).
    let mut jpeg_bytes: Vec<u8> = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut jpeg_bytes);
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, 65);
    small.write_with_encoder(encoder)?;

    *LATEST_JPEG.get_or_init(|| Mutex::new(None)).lock().unwrap() = Some(jpeg_bytes.clone());
    if let Some(tx) = MJPEG_TX.get() {
        let _ = tx.send(jpeg_bytes);
    }
    Ok(())
}

/// Detects the face region using YCbCr criteria (skin color).
/// Looks for the centroid in the upper third of the image (forehead/cheeks)
/// to avoid hands near the chin biasing the result.
/// Returns (x, y, width, height) or None if no face is found.
fn detect_face_region(rgb: &[u8], width: u32, height: u32) -> Option<(u32, u32, u32, u32)> {
    // Search from 10% to 60% of the height: avoids the ceiling at the top
    // and hands/shoulders at the bottom. The centroid falls on the eyes/nose.
    let y_start = height / 10;
    let search_height = height * 3 / 5;
    let mut sum_x: u64 = 0;
    let mut sum_y: u64 = 0;
    let mut count: u64 = 0;

    for y in y_start..search_height {
        for x in 0..width {
            let idx = ((y * width + x) * 3) as usize;
            if idx + 2 >= rgb.len() {
                continue;
            }
            let r = rgb[idx] as f32;
            let g = rgb[idx + 1] as f32;
            let b = rgb[idx + 2] as f32;

            let luma = 0.299 * r + 0.587 * g + 0.114 * b;
            if luma < 40.0 {
                continue;
            }

            // RGB → YCbCr conversion
            let cb = 128.0 - 0.168736 * r - 0.331264 * g + 0.5 * b;
            let cr = 128.0 + 0.5 * r - 0.418688 * g - 0.081312 * b;

            // Chai & Ngan skin color range
            if (77.0..=127.0).contains(&cb) && (133.0..=173.0).contains(&cr) {
                sum_x += x as u64;
                sum_y += y as u64;
                count += 1;
            }
        }
    }

    // Not enough skin in the main area → extend up to 70%
    if count < 400 {
        let search_height2 = height * 7 / 10;
        for y in search_height..search_height2 {
            for x in 0..width {
                let idx = ((y * width + x) * 3) as usize;
                if idx + 2 >= rgb.len() {
                    continue;
                }
                let r = rgb[idx] as f32;
                let g = rgb[idx + 1] as f32;
                let b = rgb[idx + 2] as f32;

                let luma = 0.299 * r + 0.587 * g + 0.114 * b;
                if luma < 40.0 {
                    continue;
                }
                let cb = 128.0 - 0.168736 * r - 0.331264 * g + 0.5 * b;
                let cr = 128.0 + 0.5 * r - 0.418688 * g - 0.081312 * b;
                if (77.0..=127.0).contains(&cb) && (133.0..=173.0).contains(&cr) {
                    sum_x += x as u64;
                    sum_y += y as u64;
                    count += 1;
                }
            }
        }
    }

    // Global minimum threshold
    if count < 600 {
        return None;
    }

    // Centroid
    let cx = (sum_x / count) as u32;
    let cy = (sum_y / count) as u32;

    // The box is asymmetric: 20% above the centroid (forehead/eyebrows), 32%
    // below (chin/jaw). The centroid itself already sits low within the
    // face — cheeks/nose/mouth dominate the skin-pixel count while hair
    // typically excludes much of the forehead from it — so reaching the
    // chin needs noticeably more margin below than above. (Previously 22%,
    // which cropped the chin in practice.) Width: 18% on each side
    // (previously 16% — a little tight against the jaw on some faces).
    let above = height * 20 / 100;
    let below = height * 32 / 100;
    let half_w = width * 18 / 100;

    let x = cx.saturating_sub(half_w);
    let y = cy.saturating_sub(above);
    let x2 = (cx + half_w).min(width - 1);
    let y2 = (cy + below).min(height - 1);

    Some((x, y, x2 - x, y2 - y))
}

/// Draws a rectangle of color `color` (3px thick) in an RGB buffer.
fn draw_rect_rgb(rgb: &mut [u8], width: u32, x: u32, y: u32, w: u32, h: u32, color: [u8; 3]) {
    let thickness = 3u32;
    let stride = width as usize * 3;

    // Horizontal edges (top + bottom)
    for dx in x..x + w {
        for t in 0..thickness {
            let top =
                ((y + t) as usize * stride + dx as usize * 3).min(rgb.len().saturating_sub(3));
            let bot = ((y + h - 1 - t) as usize * stride + dx as usize * 3)
                .min(rgb.len().saturating_sub(3));
            if top + 2 < rgb.len() {
                rgb[top..top + 3].copy_from_slice(&color);
            }
            if bot + 2 < rgb.len() {
                rgb[bot..bot + 3].copy_from_slice(&color);
            }
        }
    }

    // Vertical edges (left + right)
    for dy in y..y + h {
        for t in 0..thickness {
            let left =
                (dy as usize * stride + (x + t) as usize * 3).min(rgb.len().saturating_sub(3));
            let right = (dy as usize * stride + (x + w - 1 - t) as usize * 3)
                .min(rgb.len().saturating_sub(3));
            if left + 2 < rgb.len() {
                rgb[left..left + 3].copy_from_slice(&color);
            }
            if right + 2 < rgb.len() {
                rgb[right..right + 3].copy_from_slice(&color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_token_param_finds_the_value() {
        assert_eq!(
            extract_token_param("GET /snapshot?token=abc123&t=456 HTTP/1.1\r\n"),
            Some("abc123")
        );
        assert_eq!(
            extract_token_param("GET /?token=abc123 HTTP/1.1\r\n"),
            Some("abc123")
        );
        // token as the second param
        assert_eq!(
            extract_token_param("GET /snapshot?t=456&token=abc123 HTTP/1.1\r\n"),
            Some("abc123")
        );
    }

    #[test]
    fn extract_token_param_none_when_missing() {
        assert_eq!(
            extract_token_param("GET /snapshot?t=456 HTTP/1.1\r\n"),
            None
        );
        assert_eq!(extract_token_param("GET /snapshot HTTP/1.1\r\n"), None);
        assert_eq!(extract_token_param(""), None);
    }

    #[test]
    fn constant_time_eq_matches_regular_equality() {
        assert!(constant_time_eq("abc123", "abc123"));
        assert!(!constant_time_eq("abc123", "abc124"));
        assert!(!constant_time_eq("abc123", "abc12"));
        assert!(!constant_time_eq("", "abc123"));
    }

    #[test]
    fn generate_mjpeg_token_is_64_lowercase_hex_chars_and_varies() {
        let a = generate_mjpeg_token();
        let b = generate_mjpeg_token();
        assert_eq!(a.len(), 64);
        assert!(a
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert_ne!(a, b);
    }

    #[test]
    fn write_owner_only_file_sets_mode_0600_and_replaces_existing_content() {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!(
            "hello-daemon-mjpeg-test-{}-{}.tmp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path_str = path.to_str().unwrap();
        std::fs::write(path_str, "stale content").unwrap();
        std::fs::set_permissions(path_str, std::fs::Permissions::from_mode(0o666)).unwrap();

        write_owner_only_file(path_str, "fresh token").unwrap();

        let mode = std::fs::metadata(path_str).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(std::fs::read_to_string(path_str).unwrap(), "fresh token");
        let _ = std::fs::remove_file(path_str);
    }

    #[test]
    fn yuyv_to_rgb_gray_input_is_gray_output() {
        // Same math as hello_camera's yuyv_to_rgb_strided, unstrided:
        // Y=128, U=128, V=128 -> u=v=0 -> R=G=B=Y for both pixels.
        let data = [128u8, 128, 128, 128];
        let rgb = yuyv_to_rgb(&data, 2, 1);
        assert_eq!(rgb, vec![128, 128, 128, 128, 128, 128]);
    }

    #[test]
    fn yuyv_to_rgb_applies_chrominance() {
        // Y=128, U=128 (u=0), V=200 (v=72): R=128+100=228, G=128-0-51=77, B=128.
        let data = [128u8, 128, 128, 200];
        let rgb = yuyv_to_rgb(&data, 2, 1);
        assert_eq!(rgb, vec![228, 77, 128, 228, 77, 128]);
    }

    #[test]
    fn write_frame_preview_produces_a_decodable_jpeg_of_the_right_size() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preview.jpg");
        // A tiny uniform-gray 4x2 YUYV frame.
        let data = vec![128u8; 4 * 2 * 2]; // width*height*2 bytes/pixel
        write_frame_preview(&data, 4, 2, &path).unwrap();

        let decoded = image::open(&path).expect("output must be a valid image");
        assert_eq!(decoded.width(), 4);
        assert_eq!(decoded.height(), 2);
    }

    #[test]
    fn detect_face_region_finds_a_synthetic_skin_colored_block() {
        let width = 100u32;
        let height = 100u32;
        let mut rgb = vec![0u8; (width * height * 3) as usize];

        // A block of a plausible skin tone (verified against the function's
        // own YCbCr thresholds) within the region it actually searches
        // (y in [height/10, height*7/10)).
        for y in 20..50u32 {
            for x in 30..70u32 {
                let idx = ((y * width + x) * 3) as usize;
                rgb[idx] = 200;
                rgb[idx + 1] = 150;
                rgb[idx + 2] = 120;
            }
        }

        let (bx, by, bw, bh) =
            detect_face_region(&rgb, width, height).expect("a skin-colored block should be found");

        // Robust property rather than exact pixel equality: the returned
        // box's center should fall within the synthetic block we drew.
        let cx = bx + bw / 2;
        let cy = by + bh / 2;
        assert!((30..70).contains(&cx), "center x {cx} outside [30,70)");
        assert!((20..50).contains(&cy), "center y {cy} outside [20,50)");
    }

    #[test]
    fn detect_face_region_returns_none_when_no_skin_color_is_present() {
        let width = 100u32;
        let height = 100u32;
        let rgb = vec![0u8; (width * height * 3) as usize]; // all black
        assert!(detect_face_region(&rgb, width, height).is_none());
    }

    #[test]
    fn mjpeg_token_file_path_is_derived_from_xdg_runtime_dir() {
        // Read-only check against whatever XDG_RUNTIME_DIR happens to be in
        // this process — never mutated here, so safe under concurrent test
        // execution.
        match std::env::var("XDG_RUNTIME_DIR") {
            Ok(dir) => assert_eq!(
                mjpeg_token_file_path(),
                Some(format!("{dir}/hello-daemon-mjpeg.token"))
            ),
            Err(_) => assert_eq!(mjpeg_token_file_path(), None),
        }
    }

    #[test]
    fn draw_rect_rgb_colors_the_border_and_leaves_the_interior_untouched() {
        let width = 20u32;
        let mut rgb = vec![0u8; (width * width * 3) as usize];
        let color = [10u8, 20, 30];

        draw_rect_rgb(&mut rgb, width, 5, 5, 10, 10, color);

        let pixel_at = |rgb: &[u8], x: u32, y: u32| -> [u8; 3] {
            let idx = ((y * width + x) * 3) as usize;
            [rgb[idx], rgb[idx + 1], rgb[idx + 2]]
        };

        // Top-left corner of the border.
        assert_eq!(pixel_at(&rgb, 5, 5), color);
        // Interior, well inside the 3px-thick border.
        assert_eq!(pixel_at(&rgb, 10, 10), [0, 0, 0]);
        // Outside the rect entirely.
        assert_eq!(pixel_at(&rgb, 0, 0), [0, 0, 0]);
    }
}
