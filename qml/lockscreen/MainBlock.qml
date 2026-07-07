/*
    SPDX-FileCopyrightText: 2016 David Edmundson <davidedmundson@kde.org>

    SPDX-License-Identifier: LGPL-2.0-or-later
*/

import QtQuick

import QtQuick.Layouts
import QtQuick.Controls as QQC2

import org.kde.plasma.components as PlasmaComponents3
import org.kde.plasma.extras as PlasmaExtras
import org.kde.kirigami as Kirigami
import org.kde.kscreenlocker as ScreenLocker

import org.kde.breeze.components
import org.kde.plasma.plasma5support as P5Support

SessionManagementScreen {
    id: sessionManager

    readonly property alias mainPasswordBox: passwordBox
    property bool lockScreenUiVisible: false
    property alias showPassword: passwordBox.showPassword

    //the y position that should be ensured visible when the on screen keyboard is visible
    property int visibleBoundary: mapFromItem(loginButton, 0, 0).y
    onHeightChanged: visibleBoundary = mapFromItem(loginButton, 0, 0).y + loginButton.height + Kirigami.Units.smallSpacing
    /*
     * Login has been requested with the following username and password
     * If username field is visible, it will be taken from that, otherwise from the "name" property of the currentIndex
     */
    signal passwordResult(string password)

    onUserSelected: {
        const nextControl = (passwordBox.visible ? passwordBox : loginButton);
        // Don't startLogin() here, because the signal is connected to the
        // Escape key as well, for which it wouldn't make sense to trigger
        // login. Using TabFocusReason, so that the loginButton gets the
        // visual highlight.
        nextControl.forceActiveFocus(Qt.TabFocusReason);
    }

    function startLogin() {
        const password = passwordBox.text

        // This is partly because it looks nicer, but more importantly it
        // works round a Qt bug that can trigger if the app is closed with a
        // TextField focused.
        //
        // See https://bugreports.qt.io/browse/QTBUG-55460
        loginButton.forceActiveFocus();
        passwordResult(password);
    }

    RowLayout {
        Layout.fillWidth: true

        PlasmaExtras.PasswordField {
            id: passwordBox
            font.pointSize: Kirigami.Theme.defaultFont.pointSize + 1
            Layout.fillWidth: true
            text: PasswordSync.password

            placeholderText: i18ndc("plasma_shell_org.kde.plasma.desktop", "@info:placeholder in text field", "Password")
            focus: true
            enabled: !authenticator.graceLocked

            // In Qt this is implicitly active based on focus rather than visibility
            // in any other application having a focussed invisible object would be weird
            // but here we are using to wake out of screensaver mode
            // We need to explicitly disable cursor flashing to avoid unnecessary renders
            cursorVisible: visible

            onAccepted: {
                if (sessionManager.lockScreenUiVisible) {
                    sessionManager.startLogin();
                }
            }

            //if empty and left or right is pressed change selection in user switch
            //this cannot be in keys.onLeftPressed as then it doesn't reach the password box
            Keys.onPressed: event => {
                if (event.key === Qt.Key_Left && !text) {
                    sessionManager.userList.decrementCurrentIndex();
                    event.accepted = true
                }
                if (event.key === Qt.Key_Right && !text) {
                    sessionManager.userList.incrementCurrentIndex();
                    event.accepted = true
                }
                // Linux Hello — any keypress means the user is back and
                // paying attention (kscreenlocker_greet's QML tree stays
                // resident across DPMS blank/unblank, so there's no separate
                // "screen woke up" signal to hook — this is the reliable
                // proxy). Harmless to call when a capture is already running
                // or just finished: the control server no-ops on its own.
                lhControl.requestRetry();
            }

            Connections {
                target: root
                function onClearPassword() {
                    passwordBox.forceActiveFocus()
                    passwordBox.text = "";
                    passwordBox.text = Qt.binding(() => PasswordSync.password);
                }
                function onNotificationRepeated() {
                    sessionManager.playHighlightAnimation();
                }
            }
        }
        Binding {
            target: PasswordSync
            property: "password"
            value: passwordBox.text
        }

        PlasmaComponents3.Button {
            id: loginButton
            Accessible.name: i18ndc("plasma_shell_org.kde.plasma.desktop", "@action:button accessible only", "Unlock")
            Layout.preferredHeight: passwordBox.implicitHeight
            Layout.preferredWidth: loginButton.Layout.preferredHeight

            icon.name: LayoutMirroring.enabled ? "go-previous" : "go-next"

            onClicked: sessionManager.startLogin()
            Keys.onEnterPressed: clicked()
            Keys.onReturnPressed: clicked()
        }
    }

    component FailableLabel : PlasmaComponents3.Label {
        id: _failableLabel
        required property int kind
        required property string label

        visible: authenticator.authenticatorTypes & kind
        text: label
        textFormat: Text.PlainText
        horizontalAlignment: Text.AlignHCenter
        Layout.fillWidth: true

        RejectPasswordAnimation {
            id: _rejectAnimation
            target: _failableLabel
            onFinished: _timer.restart()
        }

        Connections {
            target: authenticator
            function onNoninteractiveError(kind, authenticator) {
                if (kind & _failableLabel.kind) {
                    _failableLabel.text = Qt.binding(() => authenticator.errorMessage)
                    _rejectAnimation.start()
                }
            }
        }
        Timer {
            id: _timer
            interval: Kirigami.Units.humanMoment
            onTriggered: {
                _failableLabel.text = Qt.binding(() => _failableLabel.label)
            }
        }
    }

    FailableLabel {
        kind: ScreenLocker.Authenticator.Fingerprint
        label: i18ndc("plasma_shell_org.kde.plasma.desktop", "@info:usagetip", "(or scan your fingerprint on the reader)")
    }
    FailableLabel {
        kind: ScreenLocker.Authenticator.Smartcard
        label: i18ndc("plasma_shell_org.kde.plasma.desktop", "@info:usagetip", "(or scan your smartcard)")
    }

    // Linux Hello — live screenlock status, polled from hello-daemon's local
    // control server (started alongside its existing lock-watcher; see
    // hello_daemon/src/screenlock.rs). kscreenlocker_greet's QML engine
    // blocks XMLHttpRequest's real network access (confirmed empirically:
    // requests complete with status=0), so status/retry go through a
    // Plasma5Support.DataSource shelling out to curl instead — a spawned
    // process isn't subject to that policy.
    ColumnLayout {
        Layout.fillWidth: true
        visible: lhControl.active

        PlasmaComponents3.Label {
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
            textFormat: Text.PlainText
            text: {
                switch (lhControl.screenlockState) {
                case "recognizing": return "🔍 Reconnaissance en cours…"
                case "success": return "✓ Visage reconnu"
                case "failed": return "✗ Non reconnu — réessayez ou saisissez votre mot de passe"
                case "offline": return "⚠ Service de reconnaissance injoignable — saisissez votre mot de passe"
                default: return "(ou regardez vers la caméra pour déverrouiller)"
                }
            }
        }

        RowLayout {
            Layout.alignment: Qt.AlignHCenter

            PlasmaComponents3.Button {
                text: "Réessayer"
                visible: lhControl.screenlockState !== "recognizing"
                onClicked: lhControl.requestRetry()
            }
            PlasmaComponents3.Button {
                text: "Utiliser le mot de passe"
                onClicked: {
                    lhControl.active = false
                    passwordBox.forceActiveFocus()
                }
            }
        }
    }

    // sessionManager's default property list only accepts QQuickItem children;
    // Timer/Connections/DataSource aren't QQuickItem, so nesting them
    // directly here fails the whole component load ("Cannot assign object of
    // type QQmlTimer to list property _children"). A plain Item's default
    // property accepts any QtObject, so it's a safe container for these
    // non-visual helpers.
    Item {
        id: lhControl

        property bool active: false
        property string screenlockState: "idle"

        function _portCmd() {
            return '$(cat "$XDG_RUNTIME_DIR/hello-daemon-screenlock-ctrl.port" 2>/dev/null)'
        }

        function pollStatus() {
            if (lhStatusSource.connectedSources.length === 0) {
                lhStatusSource.connectSource(
                    "sh -c 'curl -s http://127.0.0.1:" + _portCmd() + "/status 2>/dev/null'")
            }
        }

        function requestRetry() {
            lhRetrySource.connectSource(
                "sh -c 'curl -s -X POST http://127.0.0.1:" + _portCmd() + "/retry 2>/dev/null'")
        }

        P5Support.DataSource {
            id: lhStatusSource
            engine: "executable"
            onNewData: (sourceName, data) => {
                disconnectSource(sourceName)
                var out = data["stdout"]
                if (!out) {
                    // curl couldn't reach the control server at all (daemon
                    // down/restarting) — say so instead of silently freezing
                    // the label at its last value, which used to look like a
                    // stuck "Reconnaissance en cours…" with a dead Retry button.
                    lhControl.screenlockState = "offline"
                    return
                }
                try {
                    var parsed = JSON.parse(out)
                    lhControl.screenlockState = parsed.state || "idle"
                } catch (e) {
                    lhControl.screenlockState = "offline"
                }
            }
        }

        P5Support.DataSource {
            id: lhRetrySource
            engine: "executable"
            onNewData: (sourceName, data) => disconnectSource(sourceName)
        }

        Timer {
            interval: 1000
            running: sessionManager.lockScreenUiVisible && lhControl.active
            repeat: true
            onTriggered: lhControl.pollStatus()
        }

        // Gives the very first automatic attempt (fired by hello-daemon's
        // own lock-transition watcher) time to start before showing
        // anything, so the label doesn't flash "idle" uselessly right at
        // lock time.
        Timer {
            id: faceAuthDelayTimer
            interval: 1100
            running: sessionManager.lockScreenUiVisible
            repeat: false
            onTriggered: lhControl.active = true
        }

        Connections {
            target: sessionManager
            function onLockScreenUiVisibleChanged() {
                if (!sessionManager.lockScreenUiVisible) {
                    lhControl.active = false
                    lhControl.screenlockState = "idle"
                    faceAuthDelayTimer.stop()
                }
            }
        }
    }
}
