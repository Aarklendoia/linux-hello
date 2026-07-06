//! Rust PAM module for Linux Hello
//!
//! Compilation: cargo build --release
//! Installation: cp target/release/libpam_linux_hello.so /lib/security/pam_linux_hello.so
//!
//! Usage in /etc/pam.d/service:
//! ```
//! auth sufficient pam_linux_hello.so context=login timeout_ms=5000
//! auth include system-login
//! ```

// Use the system allocator (libc malloc) instead of the Rust allocator.
// Avoids the conflict between two Rust runtimes when the .so is loaded by
// a Rust binary (rust-sudo-rs): both share the same system malloc.
#[global_allocator]
static ALLOCATOR: std::alloc::System = std::alloc::System;

use serde::{Deserialize, Serialize};
use std::ffi::CStr;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::raw::{c_char, c_int};
use std::os::unix::fs::OpenOptionsExt;
use std::time::{SystemTime, UNIX_EPOCH};

// Basic C bindings
#[repr(C)]
pub struct PamHandle {
    _private: [u8; 0],
}

extern "C" {
    fn pam_get_user(pamh: *mut PamHandle, user: *mut *const c_char, prompt: *const c_char)
        -> c_int;
}

// PAM constants
const PAM_SUCCESS: c_int = 0;

// Retcodes
const PAM_AUTH_ERR: c_int = 7;
const PAM_IGNORE: c_int = 25;

// PAM conversation message styles
const PAM_TEXT_INFO: c_int = 4;
const PAM_ERROR_MSG: c_int = 3;

// Item type to retrieve the conversation function
const PAM_CONV_ITEM: c_int = 5;

/// PAM message structure (see <security/pam_appl.h>)
#[repr(C)]
struct PamMessage {
    msg_style: c_int,
    msg: *const c_char,
}

/// PAM response structure
#[repr(C)]
struct PamResponse {
    resp: *mut c_char,
    resp_retcode: c_int,
}

/// Structure of the PAM conversation function
#[repr(C)]
struct PamConv {
    conv: Option<
        unsafe extern "C" fn(
            num_msg: c_int,
            msg: *const *const PamMessage,
            resp: *mut *mut PamResponse,
            appdata_ptr: *mut std::os::raw::c_void,
        ) -> c_int,
    >,
    appdata_ptr: *mut std::os::raw::c_void,
}

extern "C" {
    fn pam_get_item(
        pamh: *const PamHandle,
        item_type: c_int,
        item: *mut *const std::os::raw::c_void,
    ) -> c_int;
}

/// PAM module options
#[derive(Debug, Clone)]
struct PamOptions {
    /// Authentication context (login, sudo, screenlock, sddm, etc.)
    context: String,

    /// Timeout in ms for capture
    timeout_ms: u64,

    /// Similarity threshold (0.0-1.0)
    similarity_threshold: f32,

    /// If true, ask for confirmation before success
    confirm: bool,

    /// Debug mode
    debug: bool,
}

impl Default for PamOptions {
    fn default() -> Self {
        Self {
            context: "default".to_string(),
            timeout_ms: 30000,
            similarity_threshold: 0.6,
            confirm: false,
            debug: false,
        }
    }
}

/// Parse PAM options
fn parse_options(argc: c_int, argv: *const *const c_char) -> PamOptions {
    let mut opts = PamOptions::default();

    if argc <= 0 || argv.is_null() {
        return opts;
    }

    unsafe {
        for i in 0..argc as usize {
            let arg_ptr = *argv.add(i);
            if arg_ptr.is_null() {
                continue;
            }

            if let Ok(arg_cstr) = CStr::from_ptr(arg_ptr).to_str() {
                // Parse key=value
                if let Some((key, value)) = arg_cstr.split_once('=') {
                    match key {
                        "context" => opts.context = value.to_string(),
                        "timeout_ms" => {
                            if let Ok(ms) = value.parse::<u64>() {
                                opts.timeout_ms = ms;
                            }
                        }
                        "similarity_threshold" => {
                            if let Ok(threshold) = value.parse::<f32>() {
                                opts.similarity_threshold = threshold;
                            }
                        }
                        "debug" => opts.debug = true,
                        _ => {}
                    }
                } else if arg_cstr == "debug" {
                    opts.debug = true;
                } else if arg_cstr == "confirm" {
                    opts.confirm = true;
                }
            }
        }
    }

    opts
}

/// Detect the current language from the PAM environment variables.
/// Returns the 2-letter language code (e.g. "fr", "de"), or "en" by default.
fn detect_lang() -> String {
    for var in &["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
        if let Ok(val) = std::env::var(var) {
            let lang = val.split(['.', '_', '@']).next().unwrap_or("en");
            if lang.len() >= 2 {
                return lang[..2].to_lowercase();
            }
        }
    }
    "en".to_string()
}

/// Translate a PAM message according to the detected language.
/// Recognized keys: "looking", "recognized", "not_recognized"
fn pam_t(key: &str) -> &'static str {
    let lang = detect_lang();
    match (lang.as_str(), key) {
        // English (default)
        (_, "looking") if lang == "en" => "🔍 Look at the camera...",
        (_, "recognized") if lang == "en" => "✓ Face recognized",
        (_, "not_recognized") if lang == "en" => "✗ Face not recognized",
        // French
        ("fr", "looking") => "🔍 Regardez vers la caméra...",
        ("fr", "recognized") => "✓ Visage reconnu",
        ("fr", "not_recognized") => "✗ Visage non reconnu",
        // German
        ("de", "looking") => "🔍 Schauen Sie in die Kamera...",
        ("de", "recognized") => "✓ Gesicht erkannt",
        ("de", "not_recognized") => "✗ Gesicht nicht erkannt",
        // Spanish
        ("es", "looking") => "🔍 Mire hacia la cámara...",
        ("es", "recognized") => "✓ Rostro reconocido",
        ("es", "not_recognized") => "✗ Rostro no reconocido",
        // Portuguese
        ("pt", "looking") => "🔍 Olhe para a câmera...",
        ("pt", "recognized") => "✓ Rosto reconhecido",
        ("pt", "not_recognized") => "✗ Rosto não reconhecido",
        // Russian
        ("ru", "looking") => "🔍 Посмотрите на камеру...",
        ("ru", "recognized") => "✓ Лицо распознано",
        ("ru", "not_recognized") => "✗ Лицо не распознано",
        // Japanese
        ("ja", "looking") => "🔍 カメラを見てください...",
        ("ja", "recognized") => "✓ 顔が認識されました",
        ("ja", "not_recognized") => "✗ 顔が認識されませんでした",
        // Chinese
        ("zh", "looking") => "🔍 请看向摄像头...",
        ("zh", "recognized") => "✓ 人脸已识别",
        ("zh", "not_recognized") => "✗ 人脸未识别",
        // Arabic
        ("ar", "looking") => "🔍 انظر إلى الكاميرا...",
        ("ar", "recognized") => "✓ تم التعرف على الوجه",
        ("ar", "not_recognized") => "✗ لم يتم التعرف على الوجه",
        // Hindi
        ("hi", "looking") => "🔍 कैमरे की ओर देखें...",
        ("hi", "recognized") => "✓ चेहरा पहचाना गया",
        ("hi", "not_recognized") => "✗ चेहरा नहीं पहचाना गया",
        // English default
        (_, "looking") => "🔍 Look at the camera...",
        (_, "recognized") => "✓ Face recognized",
        (_, "not_recognized") => "✗ Face not recognized",
        _ => "",
    }
}

/// Send a message via the PAM conversation (pam_conv).
/// We send even if PAM_SILENT is active: the user must know
/// that the camera is in use (essential biometric feedback).
fn pam_conv_send(pamh: *mut PamHandle, _flags: c_int, msg_style: c_int, msg: &str) {
    log_pam(&format!(
        "pam_conv_send: msg_style={} msg={}",
        msg_style, msg
    ));

    use std::ffi::CString;
    let msg_cstr = match CString::new(msg) {
        Ok(s) => s,
        Err(e) => {
            log_pam(&format!("pam_conv_send: CString::new failed: {}", e));
            return;
        }
    };

    unsafe {
        let mut item_ptr: *const std::os::raw::c_void = std::ptr::null();
        let ret = pam_get_item(pamh, PAM_CONV_ITEM, &mut item_ptr);
        log_pam(&format!(
            "pam_conv_send: pam_get_item ret={} ptr_null={}",
            ret,
            item_ptr.is_null()
        ));
        if ret != PAM_SUCCESS || item_ptr.is_null() {
            return;
        }

        let conv = &*(item_ptr as *const PamConv);
        let conv_fn = match conv.conv {
            Some(f) => f,
            None => {
                log_pam("pam_conv_send: conv.conv is None");
                return;
            }
        };

        let pam_msg = PamMessage {
            msg_style,
            msg: msg_cstr.as_ptr(),
        };
        let msg_ptr: *const PamMessage = &pam_msg;
        let mut resp_ptr: *mut PamResponse = std::ptr::null_mut();

        let ret = (conv_fn)(1, &msg_ptr, &mut resp_ptr, conv.appdata_ptr);
        log_pam(&format!("pam_conv_send: conv_fn ret={}", ret));

        // Free the response allocated by the application
        if !resp_ptr.is_null() {
            let resp = &*resp_ptr;
            if !resp.resp.is_null() {
                libc::free(resp.resp as *mut _);
            }
            libc::free(resp_ptr as *mut _);
        }
    }
}

/// Main PAM function: authentication
///
/// # Arguments
/// * `pamh` - PAM handle
/// * `flags` - PAM flags (PAM_SILENT, etc.)
/// * `argc` - Number of arguments
/// * `argv` - Arguments (argv\[0\] is the module name, argv\[1..\] are the options)
///
/// # Returns
/// PAM_SUCCESS if authentication succeeded
/// PAM_AUTH_ERR if authentication failed
/// PAM_IGNORE if the module cannot authenticate (let the next one continue)
///
/// # Safety
/// This function dereferences raw pointers passed from C code (pamh and argv).
/// The caller must ensure these pointers are valid and properly aligned.
#[allow(non_snake_case)]
#[allow(unsafe_op_in_unsafe_fn)]
#[no_mangle]
pub unsafe extern "C" fn pam_sm_authenticate(
    pamh: *mut PamHandle,
    flags: c_int,
    argc: c_int,
    argv: *const *const c_char,
) -> c_int {
    log_pam("pam_sm_authenticate called");
    log_pam("pam_sm_authenticate started");

    // Parse the options
    let opts = parse_options(argc, argv);

    if opts.debug {
        log_pam(&format!(
            "Options: context={}, timeout_ms={}, confirm={}",
            opts.context, opts.timeout_ms, opts.confirm
        ));
    }

    // Retrieve the PAM user
    let username = unsafe {
        let mut user_ptr: *const c_char = std::ptr::null();
        let ret = pam_get_user(pamh, &mut user_ptr, std::ptr::null());

        if ret != PAM_SUCCESS {
            log_pam("Failed to retrieve PAM user");
            return PAM_AUTH_ERR;
        }

        if user_ptr.is_null() {
            log_pam("PAM user is null");
            return PAM_AUTH_ERR;
        }

        match CStr::from_ptr(user_ptr).to_str() {
            Ok(u) => u.to_string(),
            Err(_) => {
                log_pam("Failed to convert user to UTF-8");
                return PAM_AUTH_ERR;
            }
        }
    };

    log_pam(&format!("Face authentication for user: {}", username));
    log_pam(&format!(
        "pam_sm_authenticate user={} context={} timeout_ms={}",
        username, &opts.context, opts.timeout_ms
    ));

    // Retrieve the user's UID
    let user_id = match uid_from_name(&username) {
        Some(uid) => uid,
        None => {
            log_pam(&format!("Failed to retrieve UID for user: {}", username));
            return PAM_AUTH_ERR;
        }
    };

    log_pam(&format!("UID for user {}: {}", username, user_id));

    // Inform the user that recognition is in progress
    pam_conv_send(pamh, flags, PAM_TEXT_INFO, pam_t("looking"));

    // Build the request for the PAM helper
    let helper_req = PamHelperRequest {
        user_id,
        context: opts.context.clone(),
        timeout_ms: opts.timeout_ms,
    };

    // Call the helper via socket instead of D-Bus
    match call_pam_helper_sync(&helper_req) {
        Ok(response) => match response {
            PamHelperResponse::Success {
                face_id,
                similarity_score,
            } => {
                log_pam(&format!(
                    "Authentication succeeded for {}: face_id={}, score={}",
                    username, face_id, similarity_score
                ));
                log_pam(&format!(
                    "helper success user={} face_id={} score={}",
                    username, face_id, similarity_score
                ));
                pam_conv_send(pamh, flags, PAM_TEXT_INFO, pam_t("recognized"));
                PAM_SUCCESS
            }
            PamHelperResponse::Failure { reason } => {
                log_pam(&format!(
                    "Authentication failed for {}: {}",
                    username, reason
                ));
                log_pam(&format!(
                    "helper failure user={} reason={}",
                    username, reason
                ));
                pam_conv_send(pamh, flags, PAM_ERROR_MSG, pam_t("not_recognized"));
                PAM_AUTH_ERR
            }
        },
        Err(e) => {
            // Error or helper unavailable = ignore and let pam_unix.so take over
            log_pam(&format!(
                "PAM helper unavailable or error: {}. Falling back to password.",
                e
            ));
            log_pam(&format!("helper error user={} err={}", username, e));
            PAM_IGNORE // <- IMPORTANT: PAM_IGNORE to move to the next module, not PAM_SYSTEM_ERR
        }
    }
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn pam_sm_setcred(
    _pamh: *mut PamHandle,
    _flags: c_int,
    _argc: c_int,
    _argv: *const *const c_char,
) -> c_int {
    log_pam("pam_sm_setcred called");
    PAM_SUCCESS
}

/// PAM function for session closing
#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn pam_sm_close_session(
    _pamh: *mut PamHandle,
    _flags: c_int,
    _argc: c_int,
    _argv: *const *const c_char,
) -> c_int {
    log_pam("pam_sm_close_session");
    PAM_SUCCESS
}

/// PAM function for password change (no action necessary)
#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn pam_sm_chauthtok(
    _pamh: *mut PamHandle,
    _flags: c_int,
    _argc: c_int,
    _argv: *const *const c_char,
) -> c_int {
    log_pam("pam_sm_chauthtok");
    PAM_IGNORE
}

/// PAM function for session management (no action necessary)
#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn pam_sm_open_session(
    _pamh: *mut PamHandle,
    _flags: c_int,
    _argc: c_int,
    _argv: *const *const c_char,
) -> c_int {
    log_pam("pam_sm_open_session");
    PAM_SUCCESS
}

/// PAM function for access management (not necessary for authentication)
#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn pam_sm_acct_mgmt(
    _pamh: *mut PamHandle,
    _flags: c_int,
    _argc: c_int,
    _argv: *const *const c_char,
) -> c_int {
    log_pam("pam_sm_acct_mgmt");
    PAM_SUCCESS
}

// ============================================================================
// Helpers
// ============================================================================

/// Translate a username into a UID
fn uid_from_name(username: &str) -> Option<u32> {
    use std::ffi::CString;

    unsafe {
        let username_cstr = match CString::new(username) {
            Ok(cstr) => cstr,
            Err(_) => return None,
        };

        // getpwnam is a C function from libc
        extern "C" {
            fn getpwnam(name: *const c_char) -> *mut libc::passwd;
        }

        let pwd = getpwnam(username_cstr.as_ptr());
        if pwd.is_null() {
            return None;
        }

        Some((*pwd).pw_uid)
    }
}

/// Request structure for the PAM helper via socket
#[derive(Serialize, Deserialize, Debug)]
struct PamHelperRequest {
    user_id: u32,
    context: String,
    timeout_ms: u64,
}

/// Response structure from the PAM helper
#[derive(Serialize, Deserialize, Debug)]
enum PamHelperResponse {
    Success {
        face_id: String,
        similarity_score: f32,
    },
    Failure {
        reason: String,
    },
}

/// Call the PAM helper via a Unix socket.
///
/// For most contexts this is the per-user socket created by that user's own
/// running `hello-daemon` session (`/run/hello-pam/<uid>.socket`, /run/hello-pam/
/// is created by systemd-tmpfiles, mode 1777, sticky — /run is not affected
/// by polkitd's PrivateTmp=yes unlike /tmp). Security relies on peer_cred on
/// the daemon side (not on path permissions).
///
/// For `context=sddm`, no per-user session/daemon exists yet at the login
/// screen — instead this connects to the fixed system-wide socket served by
/// `hello-daemon-system` (started at boot, root, verify-only; see
/// `hello_daemon::pam_helper::start_system_pam_helper` and
/// docs/PAM_MODULE.md). Same request/response wire format either way.
fn call_pam_helper_sync(req: &PamHelperRequest) -> Result<PamHelperResponse, String> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let socket_path = if req.context == "sddm" {
        "/run/hello-pam/system.socket".to_string()
    } else {
        format!("/run/hello-pam/{}.socket", req.user_id)
    };
    log_pam(&format!("Connecting to socket: {}", socket_path));

    // Short connection timeout: if the daemon is down, fail fast
    // and let the password take over without waiting 30s.
    // connect() on a Unix socket is immediate: ECONNREFUSED if the daemon is down,
    // instant success otherwise. No need for a connection timeout.
    let mut stream = UnixStream::connect(&socket_path)
        .map_err(|e| format!("Socket {} unreachable: {}", socket_path, e))?;

    // Read timeout = recognition duration + 2s margin.
    // If the daemon crashes during verify(), the stream closes and read_to_end
    // returns immediately with empty data -> Err -> PAM_IGNORE -> password.
    stream
        .set_read_timeout(Some(std::time::Duration::from_millis(
            req.timeout_ms + 2000,
        )))
        .ok();
    stream
        .set_write_timeout(Some(std::time::Duration::from_millis(5000)))
        .ok();

    let request_json = serde_json::to_string(req).map_err(|e| format!("Serialize: {}", e))?;
    stream
        .write_all(request_json.as_bytes())
        .map_err(|e| format!("Write: {}", e))?;
    // Signal the end of writing so the daemon knows to parse the request
    stream.shutdown(std::net::Shutdown::Write).ok();

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|e| format!("Read: {}", e))?;

    serde_json::from_slice(&response).map_err(|e| {
        format!(
            "Deserialize: {} (received: {})",
            e,
            String::from_utf8_lossy(&response)
        )
    })
}

fn log_pam(message: &str) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let line = format!(
        "{}.{:03} pam_linux_hello: {}",
        timestamp.as_secs(),
        timestamp.subsec_millis(),
        message
    );
    let mut file = match OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o644)
        .open("/tmp/pam_linux_hello.log")
    {
        Ok(f) => f,
        Err(e) => {
            eprintln!("pam log failed: {}", e);
            return;
        }
    };
    let _ = writeln!(file, "{}", line);
}

// The old functions remain for compatibility (unused)
#[allow(dead_code)]
/// D-Bus request structure for Verify
#[derive(Serialize, Deserialize, Debug)]
struct VerifyRequest {
    user_id: u32,
    context: String,
    timeout_ms: u64,
}

#[allow(dead_code)]
/// D-Bus response structure for Verify
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
enum VerifyResponse {
    Success {
        face_id: String,
        similarity_score: f32,
    },
    Failure {
        reason: String,
    },
}

// Test
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_options() {
        // Create test arguments
        let opts = PamOptions::default();
        assert_eq!(opts.context, "default");
        assert_eq!(opts.timeout_ms, 30000);
    }
}
