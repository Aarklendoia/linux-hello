pragma Singleton
import QtQuick

QtObject {
    id: controller

    // Application state
    property bool capturing: false
    property int progress: 0
    property var facesList: []
    property bool daemonActive: false
    property bool sddmActive: false
    property bool sddmAvailable: false
    property bool sddmBusy: false
    property string sddmError: ""
    property string ctrlPort: "0"
    property string ctrlToken: ""
    property string mjpegToken: ""
    property string lastRegisteredFaceId: ""
    property var uidNameCache: ({})

    // Whether the active camera has an IR channel — defaults to true so the
    // Enrollment screen doesn't flash a false "weaker security" warning
    // before this loads (or if the daemon is briefly unreachable; see
    // /camera-info's own fail-open comment in main.rs).
    property bool hasIrCamera: true

    // State signals
    signal appProgressChanged(int value)
    signal captureCompletedSignal
    signal captureErrorSignal(string message)

    // Navigation signals — main.qml listens and performs the actual pageStack.replace
    signal navigateToHomeSignal
    signal navigateToEnrollSignal
    signal navigateToManageFacesSignal

    // Internal signal to restart the animation timer (which lives in main.qml)
    signal restartTimerNeeded

    Component.onCompleted: {
        // The port/token files live under /run/user/<uid> (the standard
        // $XDG_RUNTIME_DIR location) — not /tmp, which is shared with every
        // other local user. /tmp's sticky bit only stops other users from
        // deleting files *we* own; it does nothing to stop a different-UID
        // attacker from pre-planting our expected filename first, which we
        // could then never remove or replace ourselves (see main.rs's
        // `runtime_dir` doc comment). /run/user/<uid> is mode 0700, owned
        // solely by this UID, so that planting attack has no foothold there.
        // main.rs passes our own UID as the last qml6 argument
        // (Qt.environmentVariable is unavailable on this build, so
        // LINUX_HELLO_UID isn't readable directly; Qt.application.arguments is).
        var args = Qt.application.arguments;
        var myUid = args[args.length - 1];
        var runtimeDir = "/run/user/" + myUid;

        var xhr = new XMLHttpRequest();
        xhr.open("GET", "file://" + runtimeDir + "/linux-hello-ctrl.port", false);
        xhr.send();
        if (xhr.responseText !== "") {
            ctrlPort = xhr.responseText.trim();
            console.log("🔌 ctrlPort read:", ctrlPort);
        } else {
            console.log("⚠ Unable to read the control port");
        }
        // Control server routes require this token (0600 file, same
        // owner-only protection as the port file above) — the loopback
        // socket itself has no per-user ACL, so without it any local
        // process could hit routes like /sddm-enable, which now triggers a
        // real pkexec prompt.
        var xhrToken = new XMLHttpRequest();
        xhrToken.open("GET", "file://" + runtimeDir + "/linux-hello-ctrl.token", false);
        xhrToken.send();
        if (xhrToken.responseText !== "") {
            ctrlToken = xhrToken.responseText.trim();
        } else {
            console.log("⚠ Unable to read the control token");
        }
        // hello-daemon's own MJPEG preview server (127.0.0.1:17823) gates
        // every request on this separate token — written by the daemon
        // itself (see hello_daemon::preview::start_mjpeg_server), not by
        // this process, so it lives in the same runtime dir but isn't
        // guaranteed to exist yet if the daemon hasn't started (Enrollment.qml
        // treats a missing/wrong token the same as "daemon unavailable").
        var xhrMjpegToken = new XMLHttpRequest();
        xhrMjpegToken.open("GET", "file://" + runtimeDir + "/hello-daemon-mjpeg.token", false);
        xhrMjpegToken.send();
        if (xhrMjpegToken.responseText !== "") {
            mjpegToken = xhrMjpegToken.responseText.trim();
        } else {
            console.log("⚠ Unable to read the MJPEG token");
        }
        // Build the UID → account name cache from /etc/passwd
        var px = new XMLHttpRequest();
        px.open("GET", "file:///etc/passwd", false);
        px.send();
        var cache = {};
        px.responseText.split("\n").forEach(function (line) {
            var parts = line.split(":");
            if (parts.length >= 3)
                cache[parseInt(parts[2])] = parts[0];
        });
        controller.uidNameCache = cache;
        checkDaemonStatus();
        checkSddmStatus();
        loadFaces();
        loadCameraInfo();
    }

    function uidToName(uid) {
        return controller.uidNameCache[uid] || ("UID " + uid);
    }

    // Opens a request against the control server with the auth token
    // attached — every route requires it now (see main.rs's
    // handle_ctrl_connection), so this replaces every direct xhr.open(...)
    // call to that server.
    function openAuthedRequest(xhr, path, async) {
        xhr.open("GET", "http://127.0.0.1:" + ctrlPort + path, async !== false);
        xhr.setRequestHeader("X-Linux-Hello-Token", ctrlToken);
    }

    // Cheap D-Bus liveness check (com.linuxhello.FaceAuth ownership) —
    // doesn't touch the camera. Re-run whenever Home becomes visible so the
    // status card reflects reality rather than a one-time startup snapshot.
    function checkDaemonStatus() {
        if (ctrlPort === "0")
            return;
        var xhr = new XMLHttpRequest();
        openAuthedRequest(xhr, "/daemon-status");
        xhr.onreadystatechange = function () {
            if (xhr.readyState !== XMLHttpRequest.DONE)
                return;
            if (xhr.status === 200) {
                try {
                    controller.daemonActive = !!JSON.parse(xhr.responseText).active;
                } catch (e) {
                    controller.daemonActive = false;
                }
            } else {
                controller.daemonActive = false;
            }
        };
        xhr.send();
    }

    // SDDM (login screen) status — a plain file read on the backend, no
    // elevation needed, safe to poll like checkDaemonStatus().
    function checkSddmStatus() {
        if (ctrlPort === "0")
            return;
        var xhr = new XMLHttpRequest();
        openAuthedRequest(xhr, "/sddm-status");
        xhr.onreadystatechange = function () {
            if (xhr.readyState !== XMLHttpRequest.DONE)
                return;
            if (xhr.status === 200) {
                try {
                    var resp = JSON.parse(xhr.responseText);
                    controller.sddmActive = !!resp.active;
                    controller.sddmAvailable = !!resp.available;
                } catch (e) {
                    controller.sddmActive = false;
                    controller.sddmAvailable = false;
                }
            } else {
                controller.sddmActive = false;
                controller.sddmAvailable = false;
            }
        };
        xhr.send();
    }

    // Enables or disables SDDM login-screen face auth, depending on the
    // last-known state. Triggers a real pkexec prompt on the backend — can
    // take several seconds while the user interacts with it.
    function toggleSddm() {
        if (ctrlPort === "0" || sddmBusy)
            return;
        controller.sddmBusy = true;
        controller.sddmError = "";
        var route = sddmActive ? "/sddm-disable" : "/sddm-enable";
        var xhr = new XMLHttpRequest();
        openAuthedRequest(xhr, route);
        xhr.onreadystatechange = function () {
            if (xhr.readyState !== XMLHttpRequest.DONE)
                return;
            controller.sddmBusy = false;
            if (xhr.status === 200) {
                try {
                    var resp = JSON.parse(xhr.responseText);
                    // resp.error, when present, is free-form text from
                    // install-pam.sh's stderr — not a translation key, shown
                    // as-is. The two fallbacks below are untranslated error
                    // codes; Home.qml resolves them to localized text via
                    // I18n.tr() at display time (this singleton doesn't
                    // import Linux.Hello itself, to avoid a self-referential
                    // module import).
                    if (!resp.ok)
                        controller.sddmError = resp.error || "sddm-error:unknown";
                } catch (e) {
                    controller.sddmError = "sddm-error:invalid-response";
                }
            } else {
                controller.sddmError = "sddm-error:http:" + xhr.status;
            }
            controller.checkSddmStatus();
        };
        xhr.send();
    }

    function startCapture() {
        capturing = true;
        progress = 0;
        var xhr = new XMLHttpRequest();
        openAuthedRequest(xhr, "/start-capture");
        xhr.onreadystatechange = function () {
            if (xhr.readyState === XMLHttpRequest.DONE)
                console.log("✓ start-capture HTTP response", xhr.status, ":", xhr.responseText);
        };
        xhr.send();
        animateProgress();
    }

    function registerFace() {
        console.log("📸 Sending /register-face to port:", ctrlPort);
        var xhr = new XMLHttpRequest();
        openAuthedRequest(xhr, "/register-face");
        xhr.onreadystatechange = function () {
            if (xhr.readyState !== XMLHttpRequest.DONE)
                return;
            controller.capturing = false;
            if (xhr.status === 200) {
                try {
                    var resp = JSON.parse(xhr.responseText);
                    if (resp.ok) {
                        controller.lastRegisteredFaceId = resp.face_id || "";
                        console.log("✓ Face registered, face_id:", controller.lastRegisteredFaceId);
                        controller.captureCompletedSignal();
                    } else {
                        console.log("✗ Registration error:", resp.error);
                        controller.captureErrorSignal(resp.error || "Erreur inconnue");
                    }
                } catch (e) {
                    console.log("✗ Invalid response:", xhr.responseText);
                    controller.captureErrorSignal("Réponse invalide du serveur");
                }
            } else {
                console.log("✗ HTTP error:", xhr.status, xhr.responseText);
                controller.captureErrorSignal("Erreur HTTP " + xhr.status);
            }
        };
        xhr.send();
    }

    function stopCapture() {
        capturing = false;
        var xhr = new XMLHttpRequest();
        openAuthedRequest(xhr, "/stop-capture");
        xhr.send();
    }

    function loadFaces() {
        console.log("🔄 loadFaces called, ctrlPort=", ctrlPort);
        if (ctrlPort === "0")
            return;
        var xhr = new XMLHttpRequest();
        openAuthedRequest(xhr, "/list-faces");
        xhr.onreadystatechange = function () {
            if (xhr.readyState !== XMLHttpRequest.DONE)
                return;
            if (xhr.status === 200) {
                try {
                    var parsed = JSON.parse(xhr.responseText);
                    controller.facesList = Array.isArray(parsed) ? parsed : [];
                    console.log("🔄 facesList updated:", controller.facesList.length, "faces");
                } catch (e) {
                    console.log("✗ Error parsing faces:", e);
                    controller.facesList = [];
                }
            }
        };
        xhr.send();
    }

    function deleteFace(faceId) {
        if (ctrlPort === "0")
            return;
        var xhr = new XMLHttpRequest();
        openAuthedRequest(xhr, "/delete-face?id=" + encodeURIComponent(faceId));
        xhr.onreadystatechange = function () {
            if (xhr.readyState === XMLHttpRequest.DONE && xhr.status === 200)
                controller.loadFaces();
        };
        xhr.send();
    }

    // Whether the active camera has an IR channel — drives Enrollment.qml's
    // weaker-security warning banner. Same fail-open reasoning as the route
    // itself: an unreachable daemon here shouldn't flash a false warning.
    function loadCameraInfo() {
        if (ctrlPort === "0")
            return;
        var xhr = new XMLHttpRequest();
        openAuthedRequest(xhr, "/camera-info");
        xhr.onreadystatechange = function () {
            if (xhr.readyState !== XMLHttpRequest.DONE)
                return;
            if (xhr.status === 200) {
                try {
                    var resp = JSON.parse(xhr.responseText);
                    controller.hasIrCamera = resp.has_ir !== false;
                } catch (e) {
                    console.log("✗ Error parsing camera info:", e);
                }
            }
        };
        xhr.send();
    }

    function navigateToHomeImpl() {
        checkDaemonStatus();
        checkSddmStatus();
        loadFaces();
        navigateToHomeSignal();
    }

    function navigateToEnrollImpl() {
        navigateToEnrollSignal();
    }

    function navigateToManageFacesImpl() {
        loadFaces();
        navigateToManageFacesSignal();
    }

    function animateProgress() {
        if (capturing && progress < 100) {
            progress += Math.random() * 15;
            if (progress > 100)
                progress = 100;
            appProgressChanged(progress);
            if (progress >= 100) {
                registerFace();
            } else {
                restartTimerNeeded();
            }
        }
    }
}
