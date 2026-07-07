import org.kde.breeze.components

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2

import org.kde.plasma.components as PlasmaComponents3
import org.kde.plasma.extras as PlasmaExtras
import org.kde.kirigami as Kirigami
import org.kde.plasma.plasma5support as P5Support

SessionManagementScreen {
    id: root
    property Item mainPasswordBox: passwordBox

    property bool showUsernamePrompt: !showUserList

    property string lastUserName
    property bool loginScreenUiVisible: false

    //the y position that should be ensured visible when the on screen keyboard is visible
    property int visibleBoundary: mapFromItem(loginButton, 0, 0).y
    onHeightChanged: visibleBoundary = mapFromItem(loginButton, 0, 0).y + loginButton.height + Kirigami.Units.smallSpacing

    property real fontSize: Kirigami.Theme.defaultFont.pointSize

    signal loginRequest(string username, string password)

    onShowUsernamePromptChanged: {
        if (!showUsernamePrompt) {
            lastUserName = ""
        }
    }

    onUserSelected: {
        // Don't startLogin() here, because the signal is connected to the
        // Escape key as well, for which it wouldn't make sense to trigger
        // login.
        passwordBox.clear()
        focusFirstVisibleFormControl();
    }

    QQC2.StackView.onActivating: {
        // Controls are not visible yet.
        Qt.callLater(focusFirstVisibleFormControl);
    }

    function focusFirstVisibleFormControl() {
        const nextControl = (userNameInput.visible
            ? userNameInput
            : (passwordBox.visible
                ? passwordBox
                : loginButton));
        // Using TabFocusReason, so that the loginButton gets the visual highlight.
        nextControl.forceActiveFocus(Qt.TabFocusReason);
    }

    /*
     * Login has been requested with the following username and password
     * If username field is visible, it will be taken from that, otherwise from the "name" property of the currentIndex
     */
    function startLogin() {
        const username = showUsernamePrompt ? userNameInput.text : userList.selectedUser
        const password = passwordBox.text

        footer.enabled = false
        mainStack.enabled = false
        userListComponent.userList.opacity = 0.75

        // This is partly because it looks nicer, but more importantly it
        // works round a Qt bug that can trigger if the app is closed with a
        // TextField focused.
        //
        // See https://bugreports.qt.io/browse/QTBUG-55460
        loginButton.forceActiveFocus();
        loginRequest(username, password);
    }

    PlasmaComponents3.TextField {
        id: userNameInput
        font.pointSize: fontSize + 1
        Layout.fillWidth: true

        text: lastUserName
        visible: showUsernamePrompt
        focus: showUsernamePrompt && !lastUserName //if there's a username prompt it gets focus first, otherwise password does
        placeholderText: i18ndc("plasma-desktop-sddm-theme", "@info:placeholder in textfield", "Username")

        onAccepted: {
            if (root.loginScreenUiVisible) {
                passwordBox.forceActiveFocus()
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true

        PlasmaExtras.PasswordField {
            id: passwordBox
            font.pointSize: fontSize + 1
            Layout.fillWidth: true

            placeholderText: i18ndc("plasma-desktop-sddm-theme",  "@info:placeholder in textfield", "Password")
            focus: !showUsernamePrompt || lastUserName

            // Disable reveal password action because SDDM does not have the breeze icon set loaded
            rightActions: []

            onAccepted: {
                if (root.loginScreenUiVisible) {
                    startLogin();
                }
            }

            visible: root.showUsernamePrompt || userList.currentItem.needsPassword

            Keys.onEscapePressed: {
                mainStack.currentItem.forceActiveFocus();
            }

            //if empty and left or right is pressed change selection in user switch
            //this cannot be in keys.onLeftPressed as then it doesn't reach the password box
            Keys.onPressed: event => {
                if (event.key === Qt.Key_Left && !text) {
                    userList.decrementCurrentIndex();
                    event.accepted = true
                }
                if (event.key === Qt.Key_Right && !text) {
                    userList.incrementCurrentIndex();
                    event.accepted = true
                }
            }

            Connections {
                target: sddm
                function onLoginFailed() {
                    passwordBox.selectAll()
                    passwordBox.forceActiveFocus()
                }
            }
        }

        PlasmaComponents3.Button {
            id: loginButton
            Accessible.name: i18ndc("plasma-desktop-sddm-theme", "@action:button Accessible name", "Log in")
            Layout.preferredHeight: passwordBox.implicitHeight
            Layout.preferredWidth: text.length === 0 ? loginButton.Layout.preferredHeight : -1

            icon.name: text.length === 0 ? (root.LayoutMirroring.enabled ? "go-previous" : "go-next") : ""

            text: root.showUsernamePrompt || userList.currentItem.needsPassword ? "" : i18nc("@action:button", "Log In")
            onClicked: startLogin()
            Keys.onEnterPressed: clicked()
            Keys.onReturnPressed: clicked()
        }
    }

    // Linux Hello — live status of the SDDM biometric login attempt,
    // polled from hello-daemon-system's control server (fixed loopback
    // port — no per-user session/$XDG_RUNTIME_DIR exists pre-login, unlike
    // the KDE lock screen's equivalent). Same technique as
    // qml/lockscreen/MainBlock.qml: this greeter's QML engine is expected to
    // block XMLHttpRequest's real network access the same way
    // kscreenlocker_greet's does, so status goes through a
    // Plasma5Support.DataSource shelling out to curl instead — a spawned
    // process isn't subject to that policy.
    //
    // No retry button here (unlike the lock screen): selecting the
    // account/pressing Enter again already re-triggers a fresh PAM attempt.
    PlasmaComponents3.Label {
        Layout.fillWidth: true
        horizontalAlignment: Text.AlignHCenter
        textFormat: Text.PlainText
        visible: text.length > 0
        text: {
            switch (lhControl.sddmState) {
            case "recognizing": return "🔍 Reconnaissance en cours…"
            case "success": return "✓ Visage reconnu"
            case "failed": return "✗ Non reconnu — saisissez votre mot de passe"
            default: return ""
            }
        }
    }

    // sessionManager's (root's) default property list only accepts
    // QQuickItem children; Timer/DataSource aren't QQuickItem, so nesting
    // them directly here fails the whole component load ("Cannot assign
    // object of type QQmlTimer to list property _children" — the same
    // crash already hit and fixed in MainBlock.qml). A plain Item's default
    // property accepts any QtObject, so it's a safe container.
    Item {
        id: lhControl

        property string sddmState: "idle"

        function pollStatus() {
            if (lhStatusSource.connectedSources.length === 0) {
                lhStatusSource.connectSource(
                    "sh -c 'curl -s http://127.0.0.1:17825/status 2>/dev/null'")
            }
        }

        P5Support.DataSource {
            id: lhStatusSource
            engine: "executable"
            onNewData: (sourceName, data) => {
                disconnectSource(sourceName)
                var out = data["stdout"]
                if (!out) return
                try {
                    var parsed = JSON.parse(out)
                    lhControl.sddmState = parsed.state || "idle"
                } catch (e) {
                    // Empty/malformed response (listener not up) — ignore.
                }
            }
        }

        Timer {
            interval: 1000
            running: root.loginScreenUiVisible
            repeat: true
            onTriggered: lhControl.pollStatus()
        }
    }
}
