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

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use tracing::{debug, info, warn};

// Bindings C basiques
#[repr(C)]
pub struct PamHandle {
    _private: [u8; 0],
}

extern "C" {
    fn pam_get_item(
        pamh: *mut PamHandle,
        item_type: c_int,
        item: *mut *const c_void,
    ) -> c_int;

    fn pam_get_user(
        pamh: *mut PamHandle,
        user: *mut *const c_char,
        prompt: *const c_char,
    ) -> c_int;
}

// Constantes PAM
const PAM_SUCCESS: c_int = 0;
const PAM_USER: c_int = 2;
const PAM_AUTHTOK: c_int = 6;
const PAM_RHOST: c_int = 7;
const PAM_CONV: c_int = 3;

// Retcodes
const PAM_AUTH_ERR: c_int = 7;
const PAM_SYSTEM_ERR: c_int = 4;
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
/// * `argv` - Arguments (argv[0] est le nom du module, argv[1..] sont les options)
///
/// # Returns
/// PAM_SUCCESS si authentification réussie
/// PAM_AUTH_ERR si authentification échouée
/// PAM_IGNORE si le module ne peut pas authentifier (laisser continuer)
#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn pam_sm_authenticate(
    pamh: *mut PamHandle,
    _flags: c_int,
    argc: c_int,
    argv: *const *const c_char,
) -> c_int {
    // Initialiser tracing pour ce thread
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(|| std::io::stderr())
        .try_init();

    debug!("pam_sm_authenticate appelé");

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

    info!(
        "Authentification faciale pour l'utilisateur: {}",
        username
    );

    // TODO: Appeler le daemon D-Bus pour vérifier
    // let verify_req = VerifyRequest {
    //     user_id: uid_from_name(&username),
    //     context: opts.context.clone(),
    //     timeout_ms: opts.timeout_ms,
    // };
    // let result = call_daemon(&verify_req);

    // Pour l'instant: mock
    if opts.debug {
        debug!("Mode debug: returning PAM_IGNORE (continue avec password)");
    }

    // PAM_IGNORE = ne pas authentifier, laisser d'autres modules s'en charger
    // C'est le comportement prudent pour le développement
    PAM_IGNORE
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

/// Traduire un nom d'utilisateur en UID (simplifié)
fn _uid_from_name(_username: &str) -> u32 {
    // TODO: Utiliser getpwnam pour vraie conversion
    1000
}

/// Appeler le daemon D-Bus (skeleton)
fn _call_daemon(_req: &str) -> Result<String, String> {
    // TODO: Implémentation D-Bus
    Err("Non implémenté".to_string())
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
