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
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::os::unix::net::UnixStream;

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
const PAM_PROMPT_ECHO_ON: c_int = 1;

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
                        // Matches "debug"'s own key=value handling: the value
                        // is irrelevant, presence is what counts (accepts the
                        // documented `confirm=true` syntax as well as bare
                        // `confirm`).
                        "confirm" => opts.confirm = true,
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
/// Recognized keys: "looking", "recognized", "not_recognized", "confirm_prompt",
/// "not_confirmed"
///
/// `confirm_prompt`'s "[y/N]" keystroke hint is intentionally left in Latin
/// script in every language (common CLI i18n convention): the confirmation
/// logic in `pam_sm_authenticate` only ever accepts a literal "y", regardless
/// of locale, so translating it to e.g. "o"/"j" per language would silently
/// break confirmation for non-English users.
fn pam_t(key: &str) -> &'static str {
    let lang = detect_lang();
    match (lang.as_str(), key) {
        // English (default)
        (_, "looking") if lang == "en" => "🔍 Look at the camera...",
        (_, "recognized") if lang == "en" => "✓ Face recognized",
        (_, "not_recognized") if lang == "en" => "✗ Face not recognized",
        (_, "confirm_prompt") if lang == "en" => "✓ Face recognized. Confirm? [y/N]: ",
        (_, "not_confirmed") if lang == "en" => "✗ Not confirmed",
        // French
        ("fr", "looking") => "🔍 Regardez vers la caméra...",
        ("fr", "recognized") => "✓ Visage reconnu",
        ("fr", "not_recognized") => "✗ Visage non reconnu",
        ("fr", "confirm_prompt") => "✓ Visage reconnu. Confirmer ? [y/N] : ",
        ("fr", "not_confirmed") => "✗ Non confirmé",
        // German
        ("de", "looking") => "🔍 Schauen Sie in die Kamera...",
        ("de", "recognized") => "✓ Gesicht erkannt",
        ("de", "not_recognized") => "✗ Gesicht nicht erkannt",
        ("de", "confirm_prompt") => "✓ Gesicht erkannt. Bestätigen? [y/N]: ",
        ("de", "not_confirmed") => "✗ Nicht bestätigt",
        // Spanish
        ("es", "looking") => "🔍 Mire hacia la cámara...",
        ("es", "recognized") => "✓ Rostro reconocido",
        ("es", "not_recognized") => "✗ Rostro no reconocido",
        ("es", "confirm_prompt") => "✓ Rostro reconocido. ¿Confirmar? [y/N]: ",
        ("es", "not_confirmed") => "✗ No confirmado",
        // Portuguese
        ("pt", "looking") => "🔍 Olhe para a câmera...",
        ("pt", "recognized") => "✓ Rosto reconhecido",
        ("pt", "not_recognized") => "✗ Rosto não reconhecido",
        ("pt", "confirm_prompt") => "✓ Rosto reconhecido. Confirmar? [y/N]: ",
        ("pt", "not_confirmed") => "✗ Não confirmado",
        // Russian
        ("ru", "looking") => "🔍 Посмотрите на камеру...",
        ("ru", "recognized") => "✓ Лицо распознано",
        ("ru", "not_recognized") => "✗ Лицо не распознано",
        ("ru", "confirm_prompt") => "✓ Лицо распознано. Подтвердить? [y/N]: ",
        ("ru", "not_confirmed") => "✗ Не подтверждено",
        // Japanese
        ("ja", "looking") => "🔍 カメラを見てください...",
        ("ja", "recognized") => "✓ 顔が認識されました",
        ("ja", "not_recognized") => "✗ 顔が認識されませんでした",
        ("ja", "confirm_prompt") => "✓ 顔が認識されました。確認しますか？ [y/N]: ",
        ("ja", "not_confirmed") => "✗ 確認されませんでした",
        // Chinese
        ("zh", "looking") => "🔍 请看向摄像头...",
        ("zh", "recognized") => "✓ 人脸已识别",
        ("zh", "not_recognized") => "✗ 人脸未识别",
        ("zh", "confirm_prompt") => "✓ 人脸已识别。确认吗？[y/N]: ",
        ("zh", "not_confirmed") => "✗ 未确认",
        // Arabic
        ("ar", "looking") => "🔍 انظر إلى الكاميرا...",
        ("ar", "recognized") => "✓ تم التعرف على الوجه",
        ("ar", "not_recognized") => "✗ لم يتم التعرف على الوجه",
        ("ar", "confirm_prompt") => "✓ تم التعرف على الوجه. تأكيد؟ [y/N]: ",
        ("ar", "not_confirmed") => "✗ لم يتم التأكيد",
        // Hindi
        ("hi", "looking") => "🔍 कैमरे की ओर देखें...",
        ("hi", "recognized") => "✓ चेहरा पहचाना गया",
        ("hi", "not_recognized") => "✗ चेहरा नहीं पहचाना गया",
        ("hi", "confirm_prompt") => "✓ चेहरा पहचाना गया। पुष्टि करें? [y/N]: ",
        ("hi", "not_confirmed") => "✗ पुष्टि नहीं हुई",
        // English default
        (_, "looking") => "🔍 Look at the camera...",
        (_, "recognized") => "✓ Face recognized",
        (_, "not_recognized") => "✗ Face not recognized",
        (_, "confirm_prompt") => "✓ Face recognized. Confirm? [y/N]: ",
        (_, "not_confirmed") => "✗ Not confirmed",
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

/// Send a prompt via the PAM conversation and return the user's typed
/// response, unlike `pam_conv_send` which only ever pushes informational
/// text and discards whatever the conv callback allocates.
///
/// Returns `None` if the conversation function is unavailable, the call
/// fails, or the response is null (e.g. a non-interactive caller) — callers
/// must treat that as "could not confirm" and fail safe rather than block
/// forever, since the PAM conversation API has no timeout of its own.
fn pam_conv_prompt(pamh: *mut PamHandle, _flags: c_int, msg: &str) -> Option<String> {
    log_pam(&format!("pam_conv_prompt: msg={}", msg));

    use std::ffi::CString;
    let msg_cstr = match CString::new(msg) {
        Ok(s) => s,
        Err(e) => {
            log_pam(&format!("pam_conv_prompt: CString::new failed: {}", e));
            return None;
        }
    };

    unsafe {
        let mut item_ptr: *const std::os::raw::c_void = std::ptr::null();
        let ret = pam_get_item(pamh, PAM_CONV_ITEM, &mut item_ptr);
        if ret != PAM_SUCCESS || item_ptr.is_null() {
            log_pam("pam_conv_prompt: no PAM_CONV_ITEM available");
            return None;
        }

        let conv = &*(item_ptr as *const PamConv);
        let conv_fn = match conv.conv {
            Some(f) => f,
            None => {
                log_pam("pam_conv_prompt: conv.conv is None");
                return None;
            }
        };

        let pam_msg = PamMessage {
            msg_style: PAM_PROMPT_ECHO_ON,
            msg: msg_cstr.as_ptr(),
        };
        let msg_ptr: *const PamMessage = &pam_msg;
        let mut resp_ptr: *mut PamResponse = std::ptr::null_mut();

        let ret = (conv_fn)(1, &msg_ptr, &mut resp_ptr, conv.appdata_ptr);
        log_pam(&format!("pam_conv_prompt: conv_fn ret={}", ret));

        if ret != PAM_SUCCESS || resp_ptr.is_null() {
            return None;
        }

        let resp = &*resp_ptr;
        let answer = if resp.resp.is_null() {
            None
        } else {
            Some(CStr::from_ptr(resp.resp).to_string_lossy().into_owned())
        };

        if !resp.resp.is_null() {
            libc::free(resp.resp as *mut _);
        }
        libc::free(resp_ptr as *mut _);

        answer
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
        username, opts.context, opts.timeout_ms
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

                if opts.confirm {
                    // Documented behavior (docs/DESIGN.md): "Confirm sudo?
                    // [y/N]" — only an explicit "y"/"Y" grants access, same
                    // as a standard confirmation prompt defaulting to No.
                    match pam_conv_prompt(pamh, flags, pam_t("confirm_prompt")) {
                        Some(answer) if answer.trim().eq_ignore_ascii_case("y") => {
                            log_pam(&format!("Confirmation accepted for {}", username));
                            PAM_SUCCESS
                        }
                        Some(_) => {
                            log_pam(&format!("Confirmation declined for {}", username));
                            pam_conv_send(pamh, flags, PAM_ERROR_MSG, pam_t("not_confirmed"));
                            PAM_AUTH_ERR
                        }
                        None => {
                            // No interactive conversation available (e.g. a
                            // non-interactive caller) — can't ask, so don't
                            // grant on face match alone: let the next module
                            // (password) decide instead, same convention as
                            // "helper unavailable" below.
                            log_pam(&format!(
                                "No conversation available to confirm for {} — falling back to password",
                                username
                            ));
                            PAM_IGNORE
                        }
                    }
                } else {
                    pam_conv_send(pamh, flags, PAM_TEXT_INFO, pam_t("recognized"));
                    PAM_SUCCESS
                }
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

/// Returns the UID of the process on the other end of `stream`, via
/// `SO_PEERCRED` (`getsockopt`, Linux-specific — matches this project's
/// Linux-only scope). Implemented by hand rather than via
/// `std::os::unix::net::UnixStream::peer_cred` because that method is not
/// yet stable on this toolchain.
fn unix_socket_peer_uid(stream: &UnixStream) -> std::io::Result<u32> {
    use std::os::unix::io::AsRawFd;
    let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let ret = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut cred as *mut libc::ucred as *mut libc::c_void,
            &mut len,
        )
    };
    if ret != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(cred.uid)
}

/// Call the PAM helper via a Unix socket.
///
/// For most contexts this is the per-user socket created by that user's own
/// running `hello-daemon` session (`/run/hello-pam/<uid>.socket`, /run/hello-pam/
/// is created by systemd-tmpfiles, mode 1777, sticky — /run is not affected
/// by polkitd's PrivateTmp=yes unlike /tmp). Security relies on peer_cred
/// checks on *both* ends: the daemon side validates its caller (see
/// `hello_daemon::pam_helper::handle_pam_request`), and this side validates
/// the daemon itself (see `unix_socket_peer_uid`'s use below) — the shared
/// directory being mode 1777 means a different-uid local attacker could
/// otherwise squat this path before the real daemon starts and have this
/// module trust whatever response they send back.
///
/// For `context=sddm`, no per-user session/daemon exists yet at the login
/// screen — instead this connects to the fixed system-wide socket served by
/// `hello-daemon-system` (started at boot, root, verify-only; see
/// `hello_daemon::pam_helper::start_system_pam_helper` and
/// docs/PAM_MODULE.md). Same request/response wire format either way.
fn call_pam_helper_sync(req: &PamHelperRequest) -> Result<PamHelperResponse, String> {
    use std::io::{Read, Write};

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

    // Verify we're actually talking to the legitimate daemon, not a rogue
    // process that squatted this path before the real one started.
    // `/run/hello-pam/` is mode 1777 (world-writable, sticky — see
    // `hello_daemon::pam_helper::start_pam_helper`'s own comment): a
    // different-uid local attacker can't remove a socket the real daemon
    // already owns, but they *can* get there first if the real daemon
    // hasn't started yet (fresh boot, crashed and not yet restarted), and
    // the sticky bit would then block the real daemon from ever reclaiming
    // the path. Without this check, this module would unconditionally
    // trust whatever `PamHelperResponse` such a rogue listener sends back —
    // including a fabricated `Success` — for every context. The legitimate
    // peer is always a specific, known uid: root for the pre-login
    // `context=sddm` system socket (`hello_daemon_system` runs as root),
    // or the exact target user for the per-user socket (`hello-daemon` is
    // a systemd `--user` unit — it runs as the account it serves, never as
    // anyone else, so no other uid, root included, is ever legitimate here).
    let expected_peer_uid = if req.context == "sddm" {
        0
    } else {
        req.user_id
    };
    match unix_socket_peer_uid(&stream) {
        Ok(uid) if uid == expected_peer_uid => {}
        Ok(uid) => {
            return Err(format!(
                "Refusing untrusted socket peer at {}: expected uid {}, got {}",
                socket_path, expected_peer_uid, uid
            ));
        }
        Err(e) => {
            return Err(format!(
                "Could not verify socket peer credentials for {}: {}",
                socket_path, e
            ));
        }
    }

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

/// Logs via syslog (LOG_AUTHPRIV), not a raw file write. Two problems with
/// the previous approach (a fixed `/tmp/pam_linux_hello.log`, opened with
/// `create(true)` and no `O_NOFOLLOW`): this module runs inside privileged
/// callers (root `su`/`sudo`, `sddm-helper`) on every authentication
/// attempt, and `/tmp` is world-writable — any local user could
/// `ln -s /some/root/owned/target /tmp/pam_linux_hello.log` and have the
/// next privileged PAM call create/append to that target as root (a
/// textbook CWE-59 symlink attack). Worse, messages embed the raw PAM
/// username *before* it's validated as a real account (see the
/// `pam_sm_authenticate` call site) — a crafted username containing a
/// newline could inject an attacker-chosen line into whatever file the
/// symlink pointed at (e.g. a cron table), turning the symlink write into
/// arbitrary root command execution. `syslog(3)` has no analogous
/// predictable-path/symlink surface: there's no file path for a local
/// attacker to redirect, so this closes the underlying issue rather than
/// just relocating it. `LOG_AUTHPRIV` is the standard facility for
/// authentication events — routed by the system's syslog config to a
/// root/adm-only destination (e.g. `/var/log/auth.log`), not a
/// world-readable one.
fn log_pam(message: &str) {
    // A fixed "%s" format string with `message` passed as the vararg (not
    // interpolated into the format string itself) avoids a printf-style
    // format-string bug if `message` ever contains a literal `%`.
    // CString::new fails (and we just skip that line) if `message`
    // contains an embedded NUL — a crafted username could in principle
    // include one, so failing closed here rather than truncating/panicking
    // is the safe choice.
    let Ok(msg_c) = CString::new(message.replace(['\n', '\r'], " ")) else {
        return;
    };
    unsafe {
        libc::syslog(
            libc::LOG_AUTHPRIV | libc::LOG_INFO,
            c"pam_linux_hello: %s".as_ptr(),
            msg_c.as_ptr(),
        );
    }
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
    use std::ffi::CString;

    #[test]
    fn unix_socket_peer_uid_reports_our_own_uid_for_a_local_connection() {
        // Regression test for the socket-squatting fix: both ends of this
        // pair are this test process itself, so the kernel-reported peer
        // uid must equal our own real uid — confirming the SO_PEERCRED
        // plumbing (getsockopt call, struct layout, field access) actually
        // works, not just that it compiles.
        let (a, b) = UnixStream::pair().expect("socketpair");
        let peer_uid_of_a = unix_socket_peer_uid(&a).expect("peer_cred on a");
        let peer_uid_of_b = unix_socket_peer_uid(&b).expect("peer_cred on b");
        let our_uid = unsafe { libc::getuid() };
        assert_eq!(peer_uid_of_a, our_uid);
        assert_eq!(peer_uid_of_b, our_uid);
    }

    #[test]
    fn test_parse_options() {
        // Create test arguments
        let opts = PamOptions::default();
        assert_eq!(opts.context, "default");
        assert_eq!(opts.timeout_ms, 30000);
    }

    fn parse_argv(args: &[&str]) -> PamOptions {
        let cstrings: Vec<CString> = args.iter().map(|s| CString::new(*s).unwrap()).collect();
        let ptrs: Vec<*const c_char> = cstrings.iter().map(|s| s.as_ptr()).collect();
        parse_options(ptrs.len() as c_int, ptrs.as_ptr())
    }

    #[test]
    fn test_parse_options_confirm_defaults_to_false() {
        assert!(!PamOptions::default().confirm);
        assert!(!parse_argv(&["context=sudo"]).confirm);
    }

    #[test]
    fn test_parse_options_confirm_bare_flag() {
        let opts = parse_argv(&["context=sudo", "confirm"]);
        assert_eq!(opts.context, "sudo");
        assert!(opts.confirm);
    }

    #[test]
    fn test_parse_options_confirm_key_value_syntax() {
        // docs/DESIGN.md documents `confirm=true`; the value itself is
        // ignored (same convention as `debug`/`debug=...`) — presence is
        // what matters.
        assert!(parse_argv(&["confirm=true"]).confirm);
        assert!(parse_argv(&["confirm=false"]).confirm);
    }

    #[test]
    fn log_pam_does_not_write_to_the_old_predictable_tmp_path() {
        // Regression test for the /tmp symlink-attack fix: log_pam must go
        // through syslog only, never touch a file at this path. Doesn't
        // assert the path is absent — a real, root-owned instance of this
        // exact file can be left over on a dev/test machine from *before*
        // this fix landed (privileged PAM calls made under the old,
        // vulnerable code), and an unprivileged test process can't remove
        // a file it doesn't own. Instead: record the file's state (or
        // absence) beforehand, call log_pam, and confirm nothing changed —
        // this still fails if log_pam ever creates the file fresh (the
        // common case, and what CI will actually exercise) or appends to
        // an existing one.
        let legacy_path = "/tmp/pam_linux_hello.log";
        let before = std::fs::metadata(legacy_path).ok().map(|m| m.len());
        log_pam("test message, should only go to syslog, not this file");
        let after = std::fs::metadata(legacy_path).ok().map(|m| m.len());
        assert_eq!(
            before, after,
            "log_pam must not create or append to {}",
            legacy_path
        );
    }

    #[test]
    fn log_pam_handles_a_message_with_an_embedded_nul_without_panicking() {
        // CString::new rejects embedded NULs; log_pam must fail closed
        // (skip the line) rather than panic — a crafted PAM username could
        // contain one.
        log_pam("contains a nul: \0 right there");
    }

    #[test]
    fn log_pam_strips_embedded_newlines() {
        // Defense in depth: even though routing through syslog (rather than
        // a file this process controls the path of) already closes the
        // log-injection escalation described in the original finding,
        // embedded newlines are still stripped so a single log_pam call
        // can't be split into multiple syslog lines.
        // (No direct way to assert syslog's own output from a unit test;
        // this just confirms the call doesn't panic on newline-bearing
        // input, matching what a crafted PAM username could contain.)
        log_pam("line one\nline two\r\nline three");
    }
}
