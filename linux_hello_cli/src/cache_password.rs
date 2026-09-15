//! `linux-hello cache-password` — captures the user's real login password,
//! verifies it via a real PAM `auth` call, and hands it to
//! `hello-daemon-system`'s cache socket to be sealed inside the TPM — see
//! `hello_daemon::secret_cache`'s module docs and
//! `docs/PAM_MODULE.md`'s "Password caching / KWallet auto-unlock" section
//! for the full design (also https://github.com/Aarklendoia/linux-hello/issues/149).
//!
//! Nothing in this codebase previously called into libpam as a *client*
//! (only `pam_linux_hello.so` implements the service-module side) — this
//! hand-rolls the minimal `pam_start`/`pam_authenticate`/`pam_end` FFI
//! needed, on the same low-level-bindings style already used in
//! `pam_linux_hello/src/lib.rs`, rather than pulling in an unfamiliar
//! `pam-client`-style crate.

use std::ffi::{CStr, CString};
use std::io::Write;
use std::os::raw::{c_char, c_int, c_void};
use zeroize::Zeroizing;

const CONSENT_WARNING: &str = "\
This stores an encrypted, TPM-sealed copy of your account password so
KWallet/the keyring can unlock automatically after a face-only login at
the SDDM login screen.

A future root-level compromise of this machine could recover this EXACT
password — and if you use the same password anywhere else (email, banking,
other machines), that compromise follows it there too, even after this
machine's own compromise is fixed.

If you reuse this login password elsewhere, either change this account's
password to one used only for logging into this machine before continuing,
or don't enable this.";

/// Entry point for `Commands::CachePassword`.
pub fn run() -> anyhow::Result<()> {
    let uid = unsafe { libc::getuid() };

    println!("{}", CONSENT_WARNING);
    print!("\nContinue? [y/N] ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    if !answer.trim().eq_ignore_ascii_case("y") {
        println!("Cancelled — nothing was stored.");
        return Ok(());
    }

    print!("Password: ");
    std::io::stdout().flush()?;
    let password = Zeroizing::new(read_password_no_echo()?);
    if password.is_empty() {
        anyhow::bail!("Empty password — nothing was cached.");
    }

    if !verify_password(&password)? {
        anyhow::bail!("Incorrect password — nothing was cached.");
    }
    println!("✓ Password verified.");

    send_to_cache_socket(uid, &password)?;
    println!("✓ Cached — KWallet should now auto-unlock after a face-only SDDM login.");
    Ok(())
}

/// Reads a line from stdin with terminal echo disabled (the newline itself
/// still echoes, `ECHONL`, so the terminal doesn't look frozen after Enter)
/// — hand-rolled via `termios` rather than a new dependency, matching this
/// codebase's existing preference for direct libc calls over small wrapper
/// crates (see e.g. `security_util::generate_token`'s direct
/// `/dev/urandom` read).
fn read_password_no_echo() -> std::io::Result<String> {
    let fd = 0; // stdin
    let mut term: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut term) } != 0 {
        // Not a terminal (e.g. piped input in a test/script) — just read
        // plainly rather than failing outright.
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        return Ok(trim_newline(input));
    }
    let original = term;
    term.c_lflag &= !libc::ECHO;
    term.c_lflag |= libc::ECHONL;
    unsafe { libc::tcsetattr(fd, libc::TCSANOW, &term) };

    let mut input = String::new();
    let result = std::io::stdin().read_line(&mut input);

    unsafe { libc::tcsetattr(fd, libc::TCSANOW, &original) };
    result?;
    Ok(trim_newline(input))
}

fn trim_newline(mut s: String) -> String {
    while s.ends_with('\n') || s.ends_with('\r') {
        s.pop();
    }
    s
}

// ============================================================================
// Minimal PAM client bindings (pam_start / pam_authenticate / pam_end)
// ============================================================================
//
// Real Linux-PAM message-style constants (`<security/_pam_types.h>`) — not
// to be confused with `pam_linux_hello`'s own local constants of similar
// names, which that module only ever uses as opaque values it forwards to
// an existing conversation callback, never interprets itself.
const PAM_SUCCESS: c_int = 0;
const PAM_PROMPT_ECHO_OFF: c_int = 1;
const PAM_PROMPT_ECHO_ON: c_int = 2;
const PAM_CONV_ERR: c_int = 6;
const PAM_BUF_ERR: c_int = 5;

#[repr(C)]
struct PamHandleOpaque {
    _private: [u8; 0],
}

#[repr(C)]
struct CPamMessage {
    msg_style: c_int,
    msg: *const c_char,
}

#[repr(C)]
struct CPamResponse {
    resp: *mut c_char,
    resp_retcode: c_int,
}

#[repr(C)]
struct CPamConv {
    conv: Option<
        unsafe extern "C" fn(
            num_msg: c_int,
            msg: *mut *const CPamMessage,
            resp: *mut *mut CPamResponse,
            appdata_ptr: *mut c_void,
        ) -> c_int,
    >,
    appdata_ptr: *mut c_void,
}

// Unlike `pam_linux_hello` (a cdylib dlopen'd by an already-running process
// that has libpam.so loaded, so its own `extern "C"` PAM calls resolve
// against the host process's global symbol table with no explicit link),
// this is a standalone binary — it must link against libpam itself for
// `pam_start`/`pam_authenticate`/`pam_end` to resolve at link time.
#[link(name = "pam")]
extern "C" {
    fn pam_start(
        service_name: *const c_char,
        user: *const c_char,
        pam_conversation: *const CPamConv,
        pamh: *mut *mut PamHandleOpaque,
    ) -> c_int;
    fn pam_authenticate(pamh: *mut PamHandleOpaque, flags: c_int) -> c_int;
    fn pam_end(pamh: *mut PamHandleOpaque, pam_status: c_int) -> c_int;
}

/// PAM conversation callback: answers every `PAM_PROMPT_ECHO_OFF`/
/// `PAM_PROMPT_ECHO_ON` message (a password/username-style prompt) with the
/// password already captured via [`read_password_no_echo`] — this client is
/// non-interactive, it never re-prompts the user itself. Other message
/// styles (info/error text from `pam_unix.so`) get an empty response, same
/// as any real PAM application would provide for a message that doesn't
/// expect one.
///
/// # Safety
/// Called by libpam's C code during `pam_authenticate`, per the
/// `struct pam_conv` contract: `msg`/`resp` are valid for `num_msg` entries,
/// `appdata_ptr` is whatever was set in [`verify_password`] (a
/// `*const CString`, kept alive for the whole `pam_authenticate` call).
/// Every `resp[i].resp` this function allocates uses `libc::malloc`
/// (matching `pam_linux_hello`'s own established convention for memory
/// crossing this boundary) since libpam frees it with `free()` afterward —
/// mixing that with Rust's global allocator would be undefined behavior.
unsafe extern "C" fn pam_conv_callback(
    num_msg: c_int,
    msg: *mut *const CPamMessage,
    resp: *mut *mut CPamResponse,
    appdata_ptr: *mut c_void,
) -> c_int {
    if num_msg <= 0 || msg.is_null() || appdata_ptr.is_null() {
        return PAM_CONV_ERR;
    }
    let password_cstr = &*(appdata_ptr as *const CString);
    let n = num_msg as usize;

    let resp_array = libc::calloc(n, std::mem::size_of::<CPamResponse>()) as *mut CPamResponse;
    if resp_array.is_null() {
        return PAM_BUF_ERR;
    }

    for i in 0..n {
        let message_ptr = *msg.add(i);
        if message_ptr.is_null() {
            continue;
        }
        let message = &*message_ptr;
        let entry = &mut *resp_array.add(i);
        entry.resp_retcode = 0;
        entry.resp = if matches!(message.msg_style, PAM_PROMPT_ECHO_OFF | PAM_PROMPT_ECHO_ON) {
            let bytes = password_cstr.as_bytes_with_nul();
            let buf = libc::malloc(bytes.len()) as *mut c_char;
            if !buf.is_null() {
                std::ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_char, buf, bytes.len());
            }
            buf
        } else {
            std::ptr::null_mut()
        };
    }

    *resp = resp_array;
    PAM_SUCCESS
}

/// Verifies `password` is actually this account's current password, via a
/// real PAM `auth` call against `/etc/pam.d/linux-hello-cache-password`
/// (installed by `install-pam.sh`: `auth required pam_unix.so`).
/// `pam_unix.so` verifying a non-root caller's *own* current password
/// already works system-wide today via `unix_chkpwd` (the setuid-root
/// helper it delegates to) — no elevated privilege is needed here.
fn verify_password(password: &str) -> anyhow::Result<bool> {
    let username = current_username()?;
    let service = CString::new("linux-hello-cache-password")?;
    let user = CString::new(username)?;
    // Kept alive for the whole pam_start/pam_authenticate/pam_end sequence
    // below — the conversation callback borrows it via `appdata_ptr`.
    let password_cstr = CString::new(password)?;

    let conv = CPamConv {
        conv: Some(pam_conv_callback),
        appdata_ptr: &password_cstr as *const CString as *mut c_void,
    };

    let mut pamh: *mut PamHandleOpaque = std::ptr::null_mut();
    // SAFETY: service/user/conv all outlive the pam_start/pam_authenticate/
    // pam_end sequence below; pamh is only ever accessed through the
    // pointer libpam itself gave us.
    let start_ret = unsafe { pam_start(service.as_ptr(), user.as_ptr(), &conv, &mut pamh) };
    if start_ret != PAM_SUCCESS || pamh.is_null() {
        anyhow::bail!("pam_start failed (ret={start_ret}) — is /etc/pam.d/linux-hello-cache-password installed?");
    }

    let auth_ret = unsafe { pam_authenticate(pamh, 0) };
    unsafe { pam_end(pamh, auth_ret) };

    Ok(auth_ret == PAM_SUCCESS)
}

fn current_username() -> anyhow::Result<String> {
    let uid = unsafe { libc::getuid() };
    let pwd = unsafe { libc::getpwuid(uid) };
    if pwd.is_null() {
        anyhow::bail!("could not resolve the current user's name (getpwuid failed)");
    }
    let name = unsafe { CStr::from_ptr((*pwd).pw_name) };
    Ok(name.to_string_lossy().into_owned())
}

// ============================================================================
// Cache socket client
// ============================================================================

/// Sends the verified password to `hello-daemon-system`'s cache socket —
/// wire-compatible with `hello_daemon::pam_helper`'s `CacheAuthtokRequest`/
/// `CacheAuthtokResponse` (built with `serde_json::json!`/`Value` here
/// rather than a matching `#[derive(Serialize)]` struct, to avoid adding a
/// direct `serde` dependency to this crate just for one request shape).
fn send_to_cache_socket(uid: u32, password: &str) -> anyhow::Result<()> {
    use std::io::Read;
    use std::os::unix::net::UnixStream;

    let socket_path = "/run/hello-pam/cache.socket";
    let mut stream = UnixStream::connect(socket_path).map_err(|e| {
        anyhow::anyhow!(
            "could not reach {socket_path}: {e}\n\
             Enable SDDM face-login first (sudo install-pam.sh --enable-sddm, \
             or the settings app's \"Login screen\" toggle) — the password \
             cache is sealed by the same root-owned service that backs it."
        )
    })?;

    let request = serde_json::json!({ "user_id": uid, "password": password });
    stream.write_all(request.to_string().as_bytes())?;
    stream.shutdown(std::net::Shutdown::Write).ok();

    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    let response: serde_json::Value = serde_json::from_slice(&response)?;

    match response.as_str() {
        Some("Ok") => Ok(()),
        _ => {
            let reason = response
                .get("Error")
                .and_then(|e| e.get("reason"))
                .and_then(|r| r.as_str())
                .unwrap_or("unknown error");
            anyhow::bail!("hello-daemon-system rejected the request: {reason}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercises `pam_conv_callback` directly, without a real PAM stack —
    /// this sandbox has no way to install a custom `/etc/pam.d/` service
    /// file to test the full `pam_start`/`pam_authenticate` path (needs
    /// root), so this instead validates the exact unsafe FFI logic that
    /// matters most: does it answer ECHO_OFF prompts with the password,
    /// leave other message styles alone, and produce buffers `libc::free`
    /// can release cleanly (mimicking what libpam itself does afterward).
    #[test]
    fn pam_conv_callback_answers_echo_prompts_with_the_password_and_frees_cleanly() {
        let password = CString::new("hunter2").unwrap();
        let appdata_ptr = &password as *const CString as *mut c_void;

        let prompt_msg = CString::new("Password: ").unwrap();
        let info_msg = CString::new("Some info").unwrap();

        let messages = [
            CPamMessage {
                msg_style: PAM_PROMPT_ECHO_OFF,
                msg: prompt_msg.as_ptr(),
            },
            CPamMessage {
                msg_style: 4, // PAM_TEXT_INFO — must not get a response
                msg: info_msg.as_ptr(),
            },
        ];
        let message_ptrs: Vec<*const CPamMessage> =
            messages.iter().map(|m| m as *const CPamMessage).collect();

        let mut resp: *mut CPamResponse = std::ptr::null_mut();
        let ret = unsafe {
            pam_conv_callback(
                message_ptrs.len() as c_int,
                message_ptrs.as_ptr() as *mut *const CPamMessage,
                &mut resp,
                appdata_ptr,
            )
        };
        assert_eq!(ret, PAM_SUCCESS);
        assert!(!resp.is_null());

        unsafe {
            let entry0 = &*resp.add(0);
            assert!(!entry0.resp.is_null());
            assert_eq!(CStr::from_ptr(entry0.resp).to_str().unwrap(), "hunter2");
            libc::free(entry0.resp as *mut c_void);

            let entry1 = &*resp.add(1);
            assert!(
                entry1.resp.is_null(),
                "a non-prompt message style must not get a password response"
            );

            libc::free(resp as *mut c_void);
        }
    }

    #[test]
    fn pam_conv_callback_rejects_a_null_appdata_pointer() {
        let prompt_msg = CString::new("Password: ").unwrap();
        let messages = [CPamMessage {
            msg_style: PAM_PROMPT_ECHO_OFF,
            msg: prompt_msg.as_ptr(),
        }];
        let message_ptrs: Vec<*const CPamMessage> =
            messages.iter().map(|m| m as *const CPamMessage).collect();
        let mut resp: *mut CPamResponse = std::ptr::null_mut();

        let ret = unsafe {
            pam_conv_callback(
                message_ptrs.len() as c_int,
                message_ptrs.as_ptr() as *mut *const CPamMessage,
                &mut resp,
                std::ptr::null_mut(),
            )
        };
        assert_eq!(ret, PAM_CONV_ERR);
        assert!(resp.is_null());
    }

    #[test]
    fn trim_newline_strips_trailing_crlf_and_lf() {
        assert_eq!(trim_newline("hunter2\r\n".to_string()), "hunter2");
        assert_eq!(trim_newline("hunter2\n".to_string()), "hunter2");
        assert_eq!(trim_newline("hunter2".to_string()), "hunter2");
    }
}
