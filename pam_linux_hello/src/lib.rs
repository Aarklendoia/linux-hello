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
use tracing::{debug, info, warn};

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
            timeout_ms: 5000,
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

/// Envoyer un message via la PAM conversation (simplified)
fn _pam_conv_send(
    _handle: *mut PamHandle,
    _msg_style: c_int,
    _msg: &str,
) -> Result<Option<String>, String> {
    // TODO: Implémentation correcte de pam_conv
    // Pour maintenant, juste stub
    Ok(None)
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
    _flags: c_int,
    argc: c_int,
    argv: *const *const c_char,
) -> c_int {
    // Initialiser tracing pour ce thread
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(std::io::stderr)
        .try_init();

    debug!("pam_sm_authenticate appelé");
    log_pam("pam_sm_authenticate commencé");

    // Parser les options
    let opts = parse_options(argc, argv);

    if opts.debug {
        debug!(
            "Options: context={}, timeout_ms={}, confirm={}",
            opts.context, opts.timeout_ms, opts.confirm
        );
    }

    // Récupérer l'utilisateur PAM
    let username = unsafe {
        let mut user_ptr: *const c_char = std::ptr::null();
        let ret = pam_get_user(pamh, &mut user_ptr, std::ptr::null());

        if ret != PAM_SUCCESS {
            warn!("Impossible de récupérer utilisateur PAM");
            return PAM_AUTH_ERR;
        }

        if user_ptr.is_null() {
            warn!("Utilisateur PAM est null");
            return PAM_AUTH_ERR;
        }

        match CStr::from_ptr(user_ptr).to_str() {
            Ok(u) => u.to_string(),
            Err(_) => {
                warn!("Impossible de convertir utilisateur en UTF-8");
                return PAM_AUTH_ERR;
            }
        }
    };

    info!("Authentification faciale pour l'utilisateur: {}", username);
    log_pam(&format!(
        "pam_sm_authenticate utilisateur={} context={} timeout_ms={}",
        username, &opts.context, opts.timeout_ms
    ));

    // Récupérer le UID de l'utilisateur
    let user_id = match uid_from_name(&username) {
        Some(uid) => uid,
        None => {
            warn!(
                "Impossible de récupérer UID pour l'utilisateur: {}",
                username
            );
            return PAM_AUTH_ERR;
        }
    };

    debug!("UID de l'utilisateur {}: {}", username, user_id);

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
                info!(
                    "Authentification réussie pour {}: face_id={}, score={}",
                    username, face_id, similarity_score
                );
                log_pam(&format!(
                    "helper success user={} face_id={} score={}",
                    username, face_id, similarity_score
                ));
                PAM_SUCCESS
            }
            PamHelperResponse::Failure { reason } => {
                warn!("Authentification échouée pour {}: {}", username, reason);
                log_pam(&format!(
                    "helper failure user={} reason={}",
                    username, reason
                ));
                PAM_AUTH_ERR
            }
        },
        Err(e) => {
            // Erreur ou helper non disponible = ignorer et laisser pam_unix.so prendre le relais
            warn!(
                "PAM helper non disponible ou erreur: {}. Passant au fallback password.",
                e
            );
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
    debug!("pam_sm_setcred appelé");
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
    debug!("pam_sm_close_session");
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
    debug!("pam_sm_chauthtok");
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
    debug!("pam_sm_open_session");
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
    debug!("pam_sm_acct_mgmt");
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
#[serde(untagged)]
enum PamHelperResponse {
    Success {
        face_id: String,
        similarity_score: f32,
    },
    Failure {
        reason: String,
    },
}

/// Appeler le helper PAM via socket Unix OU directement via subprocess
fn call_pam_helper_sync(req: &PamHelperRequest) -> Result<PamHelperResponse, String> {
    let uid = unsafe { libc::getuid() };
    log_pam(&format!(
        "call_pam_helper_sync uid={} target_uid={}",
        uid, req.user_id
    ));

    // Essayer d'appeler la CLI quel que soit le UID
    // Si on est root, appel direct; sinon, on tente en tant qu'utilisateur
    call_via_cli(req)
}

/// Appeler le daemon via le CLI tool (simule ce que le helper ferait)
fn call_via_cli(req: &PamHelperRequest) -> Result<PamHelperResponse, String> {
    use std::process::Command;

    // Récupérer le home de l'utilisateur original en lisant /etc/passwd
    let home = get_home_for_uid(req.user_id)
        .ok_or("Impossible de récupérer le home de l'utilisateur".to_string())?;

    log_pam(&format!(
        "call_via_cli user_id={} home={}",
        req.user_id, &home
    ));

    let cli_path = "/home/edtech/Documents/linux-hello-rust/target/release/examples/test_cli";

    // Appeler le CLI de test pour vérifier
    let output = Command::new(cli_path)
        .arg("--storage")
        .arg(format!("{}/.local/share/linux-hello", home))
        .arg("verify")
        .arg(req.user_id.to_string())
        .arg(&req.context)
        .output()
        .map_err(|e| format!("Erreur lancement CLI: {}", e))?;

    log_pam(&format!(
        "call_via_cli status={} stdout={}",
        output.status,
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .unwrap_or("")
    ));

    if !output.status.success() {
        return Err("CLI verify failed".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parser la sortie pour extraire le score
    if stdout.contains("1.00") || stdout.contains("Succès") {
        Ok(PamHelperResponse::Success {
            face_id: "face_via_cli".to_string(),
            similarity_score: 1.0,
        })
    } else {
        Ok(PamHelperResponse::Failure {
            reason: "Face verification failed".to_string(),
        })
    }
}

/// Récupérer le répertoire home d'un utilisateur via son UID
fn get_home_for_uid(uid: u32) -> Option<String> {
    use std::fs;

    let passwd_content = fs::read_to_string("/etc/passwd").ok()?;

    for line in passwd_content.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 6 {
            if let Ok(line_uid) = parts[2].parse::<u32>() {
                if line_uid == uid {
                    return Some(parts[5].to_string());
                }
            }
        }
    }

    None
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
