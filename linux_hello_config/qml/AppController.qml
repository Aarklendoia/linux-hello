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
    property string lastRegisteredFaceId: ""
    property var uidNameCache: ({})

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
        var xhr = new XMLHttpRequest();
        xhr.open("GET", "file:///tmp/linux-hello-ctrl.port", false);
        xhr.send();
        if (xhr.responseText !== "") {
            ctrlPort = xhr.responseText.trim();
            console.log("🔌 ctrlPort read:", ctrlPort);
        } else {
            console.log("⚠ Unable to read the control port");
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
    }

    function uidToName(uid) {
        return controller.uidNameCache[uid] || ("UID " + uid);
    }

    // Cheap D-Bus liveness check (com.linuxhello.FaceAuth ownership) —
    // doesn't touch the camera. Re-run whenever Home becomes visible so the
    // status card reflects reality rather than a one-time startup snapshot.
    function checkDaemonStatus() {
        if (ctrlPort === "0")
            return;
        var xhr = new XMLHttpRequest();
        xhr.open("GET", "http://127.0.0.1:" + ctrlPort + "/daemon-status", true);
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
        xhr.open("GET", "http://127.0.0.1:" + ctrlPort + "/sddm-status", true);
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
        xhr.open("GET", "http://127.0.0.1:" + ctrlPort + route, true);
        xhr.onreadystatechange = function () {
            if (xhr.readyState !== XMLHttpRequest.DONE)
                return;
            controller.sddmBusy = false;
            if (xhr.status === 200) {
                try {
                    var resp = JSON.parse(xhr.responseText);
                    if (!resp.ok)
                        controller.sddmError = resp.error || "Erreur inconnue";
                } catch (e) {
                    controller.sddmError = "Réponse invalide du serveur";
                }
            } else {
                controller.sddmError = "Erreur HTTP " + xhr.status;
            }
            controller.checkSddmStatus();
        };
        xhr.send();
    }

    function startCapture() {
        capturing = true;
        progress = 0;
        var xhr = new XMLHttpRequest();
        xhr.open("GET", "http://127.0.0.1:" + ctrlPort + "/start-capture", true);
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
        xhr.open("GET", "http://127.0.0.1:" + ctrlPort + "/register-face", true);
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
        xhr.open("GET", "http://127.0.0.1:" + ctrlPort + "/stop-capture", true);
        xhr.send();
    }

    function loadFaces() {
        console.log("🔄 loadFaces called, ctrlPort=", ctrlPort);
        if (ctrlPort === "0")
            return;
        var xhr = new XMLHttpRequest();
        xhr.open("GET", "http://127.0.0.1:" + ctrlPort + "/list-faces", true);
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
        xhr.open("GET", "http://127.0.0.1:" + ctrlPort + "/delete-face?id=" + encodeURIComponent(faceId), true);
        xhr.onreadystatechange = function () {
            if (xhr.readyState === XMLHttpRequest.DONE && xhr.status === 200)
                controller.loadFaces();
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
