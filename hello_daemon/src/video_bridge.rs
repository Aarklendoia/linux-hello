//! Bridge pour exposer le flux V4L2 à Qt6/QML via FFI
//!
//! Permet à la GUI QML d'afficher un flux vidéo en direct
//! en convertissant les frames V4L2 en QVideoFrame Qt6

use parking_lot::Mutex;
use std::sync::Arc;

/// Structure pour stocker les callbacks vers C++
pub struct VideoFrameBridge {
    /// Pointeur vers la fonction C++ de traitement des frames
    frame_callback: Arc<Mutex<Option<extern "C" fn(*const u8, usize, u32, u32)>>>,
}

impl VideoFrameBridge {
    pub fn new() -> Self {
        Self {
            frame_callback: Arc::new(Mutex::new(None)),
        }
    }

    /// Enregistrer le callback C++ pour recevoir les frames
    pub fn set_frame_callback(&self, callback: extern "C" fn(*const u8, usize, u32, u32)) {
        *self.frame_callback.lock() = Some(callback);
    }

    /// Envoyer une frame V4L2 YUYV vers Qt6
    ///
    /// # Arguments
    /// * `data` - Données brutes YUYV
    /// * `width` - Largeur de l'image
    /// * `height` - Hauteur de l'image
    pub fn push_frame(&self, data: &[u8], width: u32, height: u32) {
        if let Some(callback) = *self.frame_callback.lock() {
            callback(data.as_ptr(), data.len(), width, height);
        }
    }
}

/// Appel FFI depuis Rust — envoyer une frame à Qt
#[no_mangle]
pub extern "C" fn video_bridge_push_frame(data: *const u8, len: usize, width: u32, height: u32) {
    if let Ok(bridge) = crate::get_video_bridge() {
        let slice = unsafe { std::slice::from_raw_parts(data, len) };
        bridge.push_frame(slice, width, height);
    }
}

/// Enregistrer le callback C++
#[no_mangle]
pub extern "C" fn video_bridge_set_callback(callback: extern "C" fn(*const u8, usize, u32, u32)) {
    if let Ok(bridge) = crate::get_video_bridge() {
        bridge.set_frame_callback(callback);
    }
}
