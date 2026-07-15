import org.kde.breeze.components

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2

import org.kde.plasma.components as PlasmaComponents3
import org.kde.plasma.extras as PlasmaExtras
import org.kde.kirigami as Kirigami

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

        // Clear any Linux Hello status text left over from a previous
        // attempt (e.g. "✗ Visage non reconnu") so it can't be mistaken
        // for feedback on this new one.
        lhLastMessage.text = ""

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

    // Linux Hello — live status of the SDDM biometric login attempt.
    //
    // First attempt at this used a Plasma5Support.DataSource shelling out
    // to curl, polling hello-daemon-system's HTTP status endpoint every
    // 1000ms (same technique as qml/lockscreen/MainBlock.qml, whose QML
    // engine really does block XMLHttpRequest's real network access).
    // Confirmed on real hardware that was the wrong approach here: a full
    // recognition attempt (prompt → matched → LoginSucceeded) completes in
    // well under a second, so a 1-second poll interval — plus whatever
    // delay before root.loginScreenUiVisible even goes true — routinely
    // never fires even once before the greeter has already moved on.
    // Meanwhile pam_linux_hello's actual PAM_TEXT_INFO/PAM_ERROR_MSG
    // messages (already localized, already emoji-prefixed — see
    // pam_linux_hello::pam_t) were confirmed via journalctl to reach the
    // greeter process itself within milliseconds ("Information Message
    // received from daemon") — SDDM's own greeter↔daemon PAM-conversation
    // channel, not something this theme needs to build itself. The only
    // real gap (also true of the stock Breeze/Kubuntu themes, matching
    // upstream SDDM's default behavior) was that nothing displayed it.
    PlasmaComponents3.Label {
        Layout.fillWidth: true
        horizontalAlignment: Text.AlignHCenter
        textFormat: Text.PlainText
        visible: text.length > 0
        text: lhLastMessage.text
    }

    // sessionManager's (root's) default property list only accepts
    // QQuickItem children — a plain Item's default property accepts any
    // QtObject, so it's a safe container for the Connections below (same
    // reasoning as the Timer/DataSource container this replaced).
    Item {
        id: lhLastMessage
        property string text: ""
    }

    Connections {
        target: sddm
        function onInformationMessage(message) { lhLastMessage.text = message }
        function onErrorMessage(message) { lhLastMessage.text = message }
    }
}
