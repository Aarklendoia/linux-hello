//! Fonctions d'exportation de preview vidéo
//!
//! Écrit les frames YUYV en JPEG dans /tmp pour affichage GUI

use image::{ImageBuffer, Rgb};
use std::path::Path;

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
    path: &Path,
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

/// Écrire la preview pour l'affichage GUI
pub fn export_preview_frame(data: &[u8], width: u32, height: u32) -> anyhow::Result<()> {
    let path = Path::new("/tmp/linux-hello-preview.jpg");
    write_frame_preview(data, width, height, path)
}
