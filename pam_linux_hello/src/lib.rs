//! Module PAM Rust pour Linux Hello
//!
//! Compilation: cargo build --release
//! Installation: cp target/release/libpam_linux_hello.so /lib/security/pam_linux_hello.so
//!
//! Utilisation dans /etc/pam.d/service:
//! ```
//! auth sufficient pam_linux_hello.so context=login timeout_ms=5000
//! auth include system-login
//! ```

use serde::{Deserialize, Serialize};
use std::ffi::CStr;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::raw::{c_char, c_int};
use std::os::unix::fs::OpenOptionsExt;
use std::time::{SystemTime, UNIX_EPOCH};

// Bindings C basiques
#[repr(C)]
pub struct PamHandle {
    _private: [u8; 0],
}

extern "C" {
    fn pam_get_user(pamh: *mut PamHandle, user: *mut *const c_char, prompt: *const c_char)
        -> c_int;
}

// Constantes PAM
const PAM_SUCCESS: c_int = 0;

// Retcodes
const PAM_AUTH_ERR: c_int = 7;
const PAM_IGNORE: c_int = 25;

// Styles de message PAM conversation
const PAM_TEXT_INFO: c_int = 4;
const PAM_ERROR_MSG: c_int = 3;

// Item type pour récupérer la fonction de conversation
const PAM_CONV_ITEM: c_int = 5;

/// Structure message PAM (voir <security/pam_appl.h>)
#[repr(C)]
struct PamMessage {
    msg_style: c_int,
    msg: *const c_char,
}

/// Structure réponse PAM
#[repr(C)]
struct PamResponse {
    resp: *mut c_char,
    resp_retcode: c_int,
}

/// Structure de la fonction de conversation PAM
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

/// Options du module PAM
#[derive(Debug, Clone)]
struct PamOptions {
    /// Contexte d'authentification (login, sudo, screenlock, sddm, etc.)
    context: String,

    /// Timeout en ms pour la capture
    timeout_ms: u64,

    /// Seuil de similarité (0.0-1.0)
    similarity_threshold: f32,

    /// Si true, demander confirmation avant succès
    confirm: bool,

    /// Mode debug
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

/// Parser les options PAM
fn parse_options(argc: c_int, argv: *const *const c_char) -> PamOptions {
    let mut opts = PamOptions::default();

    if argc <= 1 || argv.is_null() {
        return opts;
    }

    unsafe {
        for i in 1..argc as usize {
            let arg_ptr = *argv.add(i);
            if arg_ptr.is_null() {
                continue;
            }

            if let Ok(arg_cstr) = CStr::from_ptr(arg_ptr).to_str() {
                // Parser key=value
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

/// Détecter la langue courante depuis les variables d'environnement PAM.
/// Retourne le code de langue à 2 lettres (ex: "fr", "de"), ou "en" par défaut.
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

/// Traduire un message PAM selon la langue détectée.
/// Clés reconnues : "looking", "recognized", "not_recognized"
fn pam_t(key: &str) -> &'static str {
    let lang = detect_lang();
    match (lang.as_str(), key) {
        // Anglais (défaut)
        (_, "looking") if lang == "en" => "🔍 Look at the camera...",
        (_, "recognized") if lang == "en" => "✓ Face recognized",
        (_, "not_recognized") if lang == "en" => "✗ Face not recognized",
        // Français
        ("fr", "looking") => "🔍 Regardez vers la caméra...",
        ("fr", "recognized") => "✓ Visage reconnu",
        ("fr", "not_recognized") => "✗ Visage non reconnu",
        // Allemand
        ("de", "looking") => "🔍 Schauen Sie in die Kamera...",
        ("de", "recognized") => "✓ Gesicht erkannt",
        ("de", "not_recognized") => "✗ Gesicht nicht erkannt",
        // Espagnol
        ("es", "looking") => "🔍 Mire hacia la cámara...",
        ("es", "recognized") => "✓ Rostro reconocido",
        ("es", "not_recognized") => "✗ Rostro no reconocido",
        // Portugais
        ("pt", "looking") => "🔍 Olhe para a câmera...",
        ("pt", "recognized") => "✓ Rosto reconhecido",
        ("pt", "not_recognized") => "✗ Rosto não reconhecido",
        // Russe
        ("ru", "looking") => "🔍 Посмотрите на камеру...",
        ("ru", "recognized") => "✓ Лицо распознано",
        ("ru", "not_recognized") => "✗ Лицо не распознано",
        // Japonais
        ("ja", "looking") => "🔍 カメラを見てください...",
        ("ja", "recognized") => "✓ 顔が認識されました",
        ("ja", "not_recognized") => "✗ 顔が認識されませんでした",
        // Chinois
        ("zh", "looking") => "🔍 请看向摄像头...",
        ("zh", "recognized") => "✓ 人脸已识别",
        ("zh", "not_recognized") => "✗ 人脸未识别",
        // Arabe
        ("ar", "looking") => "🔍 انظر إلى الكاميرا...",
        ("ar", "recognized") => "✓ تم التعرف على الوجه",
        ("ar", "not_recognized") => "✗ لم يتم التعرف على الوجه",
        // Hindi
        ("hi", "looking") => "🔍 कैमरे की ओर देखें...",
        ("hi", "recognized") => "✓ चेहरा पहचाना गया",
        ("hi", "not_recognized") => "✗ चेहरा नहीं पहचाना गया",
        // Défaut anglais
        (_, "looking") => "🔍 Look at the camera...",
        (_, "recognized") => "✓ Face recognized",
        (_, "not_recognized") => "✗ Face not recognized",
        _ => "",
    }
}

/// Envoyer un message via la PAM conversation (pam_conv).
/// On envoie même si PAM_SILENT est actif : l'utilisateur doit savoir
/// que la caméra est en cours d'utilisation (retour biométrique essentiel).
fn pam_conv_send(pamh: *mut PamHandle, _flags: c_int, msg_style: c_int, msg: &str) {
    log_pam(&format!(
        "pam_conv_send: msg_style={} msg={}",
        msg_style, msg
    ));

    use std::ffi::CString;
    let msg_cstr = match CString::new(msg) {
        Ok(s) => s,
        Err(e) => {
            log_pam(&format!("pam_conv_send: CString::new échoué: {}", e));
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
                log_pam("pam_conv_send: conv.conv est None");
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

        // Libérer la réponse allouée par l'application
        if !resp_ptr.is_null() {
            let resp = &*resp_ptr;
            if !resp.resp.is_null() {
                libc::free(resp.resp as *mut _);
            }
            libc::free(resp_ptr as *mut _);
        }
    }
}

/// Fonction principale PAM: authentication
///
/// # Arguments
/// * `pamh` - PAM handle
/// * `flags` - Flags PAM (PAM_SILENT, etc.)
/// * `argc` - Nombre d'arguments
/// * `argv` - Arguments (argv\[0\] est le nom du module, argv\[1..\] sont les options)
///
/// # Returns
/// PAM_SUCCESS si authentification réussie
/// PAM_AUTH_ERR si authentification échouée
/// PAM_IGNORE si le module ne peut pas authentifier (laisser continuer)
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
    log_pam("pam_sm_authenticate appelé");
    log_pam("pam_sm_authenticate commencé");

    // Parser les options
    let opts = parse_options(argc, argv);

    if opts.debug {
        log_pam(&format!(
            "Options: context={}, timeout_ms={}, confirm={}",
            opts.context, opts.timeout_ms, opts.confirm
        ));
    }

    // Récupérer l'utilisateur PAM
    let username = unsafe {
        let mut user_ptr: *const c_char = std::ptr::null();
        let ret = pam_get_user(pamh, &mut user_ptr, std::ptr::null());

        if ret != PAM_SUCCESS {
            log_pam("Impossible de récupérer utilisateur PAM");
            return PAM_AUTH_ERR;
        }

        if user_ptr.is_null() {
            log_pam("Utilisateur PAM est null");
            return PAM_AUTH_ERR;
        }

        match CStr::from_ptr(user_ptr).to_str() {
            Ok(u) => u.to_string(),
            Err(_) => {
                log_pam("Impossible de convertir utilisateur en UTF-8");
                return PAM_AUTH_ERR;
            }
        }
    };

    log_pam(&format!(
        "Authentification faciale pour l'utilisateur: {}",
        username
    ));
    log_pam(&format!(
        "pam_sm_authenticate utilisateur={} context={} timeout_ms={}",
        username, &opts.context, opts.timeout_ms
    ));

    // Récupérer le UID de l'utilisateur
    let user_id = match uid_from_name(&username) {
        Some(uid) => uid,
        None => {
            log_pam(&format!(
                "Impossible de récupérer UID pour l'utilisateur: {}",
                username
            ));
            return PAM_AUTH_ERR;
        }
    };

    log_pam(&format!("UID de l'utilisateur {}: {}", username, user_id));

    // Informer l'utilisateur que la reconnaissance est en cours
    pam_conv_send(pamh, flags, PAM_TEXT_INFO, pam_t("looking"));

    // Créer la requête pour le helper PAM
    let helper_req = PamHelperRequest {
        user_id,
        context: opts.context.clone(),
        timeout_ms: opts.timeout_ms,
    };

    // Appeler le helper via socket au lieu de D-Bus
    match call_pam_helper_sync(&helper_req) {
        Ok(response) => match response {
            PamHelperResponse::Success {
                face_id,
                similarity_score,
            } => {
                log_pam(&format!(
                    "Authentification réussie pour {}: face_id={}, score={}",
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
                    "Authentification échouée pour {}: {}",
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
            // Erreur ou helper non disponible = ignorer et laisser pam_unix.so prendre le relais
            log_pam(&format!(
                "PAM helper non disponible ou erreur: {}. Passant au fallback password.",
                e
            ));
            log_pam(&format!("helper error user={} err={}", username, e));
            PAM_IGNORE // ← IMPORTANT: PAM_IGNORE pour passer au suivant, pas PAM_SYSTEM_ERR
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
    log_pam("pam_sm_setcred appelé");
    PAM_SUCCESS
}

/// Fonction PAM pour fermeture de session
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

/// Fonction PAM pour changement de mot de passe (pas d'action nécessaire)
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

/// Fonction PAM pour gestion de session (pas d'action nécessaire)
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

/// Fonction PAM pour gestion d'accès (pas nécessaire pour authentication)
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

/// Traduire un nom d'utilisateur en UID
fn uid_from_name(username: &str) -> Option<u32> {
    use std::ffi::CString;

    unsafe {
        let username_cstr = match CString::new(username) {
            Ok(cstr) => cstr,
            Err(_) => return None,
        };

        // getpwnam est une fonction C de libc
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

/// Structure de requête pour le helper PAM via socket
#[derive(Serialize, Deserialize, Debug)]
struct PamHelperRequest {
    user_id: u32,
    context: String,
    timeout_ms: u64,
}

/// Structure de réponse du helper PAM
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

/// Appeler le helper PAM via socket Unix créée par le daemon utilisateur.
/// Le daemon écoute sur /tmp/hello-pam-<uid>.socket avec 0o666.
fn call_pam_helper_sync(req: &PamHelperRequest) -> Result<PamHelperResponse, String> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let socket_path = format!("/tmp/hello-pam-{}.socket", req.user_id);
    log_pam(&format!("Connecting to socket: {}", socket_path));

    let mut stream = UnixStream::connect(&socket_path)
        .map_err(|e| format!("Socket {} inaccessible: {}", socket_path, e))?;

    // Timeout = timeout de reconnaissance + 2s de marge réseau
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
    // Signaler la fin de l'écriture pour que le daemon sache parser la requête
    stream.shutdown(std::net::Shutdown::Write).ok();

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|e| format!("Read: {}", e))?;

    serde_json::from_slice(&response).map_err(|e| {
        format!(
            "Deserialize: {} (reçu: {})",
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

// Les anciennes fonctions restent pour compatibilité (pas utilisées)
#[allow(dead_code)]
/// Structure de requête D-Bus pour Verify
#[derive(Serialize, Deserialize, Debug)]
struct VerifyRequest {
    user_id: u32,
    context: String,
    timeout_ms: u64,
}

#[allow(dead_code)]
/// Structure de réponse D-Bus pour Verify
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
        // Créer des arguments de test
        let opts = PamOptions::default();
        assert_eq!(opts.context, "default");
        assert_eq!(opts.timeout_ms, 5000);
    }
}
