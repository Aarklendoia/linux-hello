//! Fonctions d'exportation de preview vidéo - serveur MJPEG HTTP
//!
//! Encode les frames V4L2 en JPEG et les diffuse en temps réel
//! via un serveur HTTP multipart sur 127.0.0.1:17823.

use image::{ImageBuffer, Rgb};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use tokio::sync::broadcast;

/// Port fixe du serveur MJPEG (loopback uniquement)
pub const MJPEG_PORT: u16 = 17823;

/// Canal broadcast : capacité 1 = on garde toujours la frame la plus récente.
static MJPEG_TX: OnceLock<broadcast::Sender<Vec<u8>>> = OnceLock::new();

/// État lissé EMA du rectangle de détection : (x, y, w, h) en f32.
type SmoothBoxState = Mutex<Option<(f32, f32, f32, f32)>>;
static SMOOTH_BOX: OnceLock<SmoothBoxState> = OnceLock::new();

/// Démarrer le serveur MJPEG HTTP sur 127.0.0.1:17823.
/// Doit être appelé une seule fois au démarrage du daemon (runtime tokio actif).
pub async fn start_mjpeg_server() -> anyhow::Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let (tx, _) = broadcast::channel::<Vec<u8>>(1);
    // Ignorer l'erreur si `start_mjpeg_server` est appelée deux fois
    let _ = MJPEG_TX.set(tx.clone());

    let listener = TcpListener::bind(format!("127.0.0.1:{}", MJPEG_PORT)).await?;
    tracing::info!("Serveur MJPEG démarré : http://127.0.0.1:{}", MJPEG_PORT);

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let mut rx = tx.subscribe();
                    tokio::spawn(async move {
                        let (mut reader, mut writer) = stream.into_split();
                        // Lire et ignorer la requête HTTP entrante
                        let mut buf = [0u8; 1024];
                        let _ = reader.read(&mut buf).await;

                        // En-têtes HTTP multipart MJPEG
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
                Err(e) => tracing::error!("Erreur accept MJPEG: {}", e),
            }
        }
    });

    Ok(())
}

/// Convertir une frame YUYV en RGB888
fn yuyv_to_rgb(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut rgb = Vec::with_capacity((width * height * 3) as usize);

    for chunk in data.chunks(4) {
        if chunk.len() == 4 {
            let y1 = chunk[0] as i32;
            let u = chunk[1] as i32 - 128;
            let y2 = chunk[2] as i32;
            let v = chunk[3] as i32 - 128;

            // Convertir Y,U,V en R,G,B
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

/// Écrire une frame YUYV en JPEG
pub fn write_frame_preview(
    data: &[u8],
    width: u32,
    height: u32,
    path: &std::path::Path,
) -> anyhow::Result<()> {
    // Convertir YUYV en RGB
    let rgb_data = yuyv_to_rgb(data, width, height);

    // Créer une image RGB
    let img = ImageBuffer::<Rgb<u8>, Vec<u8>>::from_raw(width, height, rgb_data)
        .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;

    // Sauvegarder en JPEG (85% qualité)
    img.save(path)?;

    Ok(())
}

/// Écrire la preview pour l'affichage GUI (données déjà en RGB)
pub fn export_preview_frame(data: &[u8], width: u32, height: u32) -> anyhow::Result<()> {
    let path = Path::new("/tmp/linux-hello-preview.jpg");
    write_frame_preview(data, width, height, path)
}

/// Écrire la preview pour l'affichage GUI (données déjà en RGB, pas de conversion).
/// Détecte le visage par skin-color (YCbCr) et dessine un rectangle vert autour.
pub fn export_preview_frame_rgb(data: &[u8], width: u32, height: u32) -> anyhow::Result<()> {
    let mut pixels = data.to_vec();

    // Détection skin-color + lissage EMA du rectangle
    if let Some((rx, ry, rw, rh)) = detect_face_region(&pixels, width, height) {
        let lock = SMOOTH_BOX.get_or_init(|| Mutex::new(None));
        let (sx, sy, sw, sh) = {
            let mut g = lock.lock().unwrap();
            let (rx, ry, rw, rh) = (rx as f32, ry as f32, rw as f32, rh as f32);
            let smoothed = match *g {
                None => (rx, ry, rw, rh),
                Some((px, py, pw, ph)) => {
                    const ALPHA: f32 = 0.35;
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

    // Réduire à 320×240 pour diminuer la latence (moins de données JPEG).
    let small = image::DynamicImage::ImageRgb8(img).resize_exact(
        320,
        240,
        image::imageops::FilterType::Triangle,
    );

    // Encoder en JPEG qualité 65 (bon compromis taille/qualité pour une preview).
    let mut jpeg_bytes: Vec<u8> = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut jpeg_bytes);
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, 65);
    small.write_with_encoder(encoder)?;

    if let Some(tx) = MJPEG_TX.get() {
        let _ = tx.send(jpeg_bytes);
    }
    Ok(())
}

/// Détecte la région du visage par critères YCbCr (couleur peau).
/// Cherche le centroïde dans le tiers supérieur de l'image (front/joues)
/// pour éviter que les mains au niveau du menton ne biaisent le résultat.
/// Retourne (x, y, largeur, hauteur) ou None si aucun visage trouvé.
fn detect_face_region(rgb: &[u8], width: u32, height: u32) -> Option<(u32, u32, u32, u32)> {
    // Chercher de 10% à 60% de la hauteur : évite le plafond en haut
    // et les mains/épaules en bas. Le centroïde tombe sur les yeux/nez.
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

            // Conversion RGB → YCbCr
            let cb = 128.0 - 0.168736 * r - 0.331264 * g + 0.5 * b;
            let cr = 128.0 + 0.5 * r - 0.418688 * g - 0.081312 * b;

            // Plage couleur peau Chai & Ngan
            if (77.0..=127.0).contains(&cb) && (133.0..=173.0).contains(&cr) {
                sum_x += x as u64;
                sum_y += y as u64;
                count += 1;
            }
        }
    }

    // Pas assez de peau dans la zone principale → étendre jusqu'à 70%
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

    // Seuil global minimum
    if count < 600 {
        return None;
    }

    // Centroïde
    let cx = (sum_x / count) as u32;
    let cy = (sum_y / count) as u32;

    // La boîte est asymétrique : 20% au-dessus du centroïde (front/sourcils), 22% en dessous (menton).
    // Largeur : 16% de chaque côté.
    let above = height * 20 / 100;
    let below = height * 22 / 100;
    let half_w = width * 16 / 100;

    let x = cx.saturating_sub(half_w);
    let y = cy.saturating_sub(above);
    let x2 = (cx + half_w).min(width - 1);
    let y2 = (cy + below).min(height - 1);

    Some((x, y, x2 - x, y2 - y))
}

/// Dessine un rectangle de couleur `color` (épaisseur 3px) dans un buffer RGB.
fn draw_rect_rgb(rgb: &mut [u8], width: u32, x: u32, y: u32, w: u32, h: u32, color: [u8; 3]) {
    let thickness = 3u32;
    let stride = width as usize * 3;

    // Bords horizontaux (haut + bas)
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

    // Bords verticaux (gauche + droite)
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
