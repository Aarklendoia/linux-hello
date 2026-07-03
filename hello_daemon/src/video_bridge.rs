//! Bridge to expose the V4L2 stream to Qt6/QML via FFI
//!
//! Lets the QML GUI display a live video stream
//! by converting V4L2 frames into Qt6 QVideoFrame

use parking_lot::Mutex;
use std::sync::Arc;

/// Structure for storing callbacks into C++
pub struct VideoFrameBridge {
    /// Pointer to the C++ frame processing function
    frame_callback: Arc<Mutex<Option<extern "C" fn(*const u8, usize, u32, u32)>>>,
}

impl VideoFrameBridge {
    pub fn new() -> Self {
        Self {
            frame_callback: Arc::new(Mutex::new(None)),
        }
    }

    /// Register the C++ callback to receive frames
    pub fn set_frame_callback(&self, callback: extern "C" fn(*const u8, usize, u32, u32)) {
        *self.frame_callback.lock() = Some(callback);
    }

    /// Send a V4L2 YUYV frame to Qt6
    ///
    /// # Arguments
    /// * `data` - Raw YUYV data
    /// * `width` - Image width
    /// * `height` - Image height
    pub fn push_frame(&self, data: &[u8], width: u32, height: u32) {
        if let Some(callback) = *self.frame_callback.lock() {
            callback(data.as_ptr(), data.len(), width, height);
        }
    }
}

/// FFI call from Rust — send a frame to Qt
#[no_mangle]
pub extern "C" fn video_bridge_push_frame(data: *const u8, len: usize, width: u32, height: u32) {
    if let Ok(bridge) = crate::get_video_bridge() {
        let slice = unsafe { std::slice::from_raw_parts(data, len) };
        bridge.push_frame(slice, width, height);
    }
}

/// Register the C++ callback
#[no_mangle]
pub extern "C" fn video_bridge_set_callback(callback: extern "C" fn(*const u8, usize, u32, u32)) {
    if let Ok(bridge) = crate::get_video_bridge() {
        bridge.set_frame_callback(callback);
    }
}
