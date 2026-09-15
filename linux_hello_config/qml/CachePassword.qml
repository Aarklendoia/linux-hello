import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Linux.Hello 1.0

// ScrollablePage, not plain Page: the consent text alone is several
// paragraphs — confirmed on Home.qml that a fixed-height, non-scrolling
// Page just clips content that doesn't fit rather than making it reachable.
Kirigami.ScrollablePage {
    id: cachePasswordPage

    title: I18n.tr("cachePassword.title")

    Layout.fillWidth: true
    Layout.fillHeight: true

    padding: Kirigami.Units.largeSpacing
    topPadding: Kirigami.Units.largeSpacing * 2

    Connections {
        target: AppController
        function onPasswordCacheBusyChanged() {
            if (!AppController.passwordCacheBusy && AppController.passwordCacheError === "") {
                // A request just finished with no error recorded — that's
                // only true right after a successful submit (checked before
                // the request, cleared at its start), so treat it as
                // success and clear the field rather than leaving a typed
                // password sitting in a TextField.
                if (passwordField.text !== "" && submitted) {
                    passwordField.text = "";
                    successLabel.visible = true;
                }
            }
        }
    }

    property bool submitted: false

    ColumnLayout {
        // Width-only, not anchors.fill — see Home.qml's identical comment:
        // a ScrollablePage's direct child must size its height from its own
        // content for the wrapping Flickable to know how far to scroll.
        width: parent.width
        spacing: Kirigami.Units.largeSpacing

        Label {
            text: I18n.tr("cachePassword.intro")
            font.pixelSize: 13
            color: Kirigami.Theme.textColor
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }

        Label {
            text: I18n.tr("cachePassword.warningBody")
            font.pixelSize: 12
            color: Kirigami.Theme.neutralTextColor
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }

        // Shown instead of the form when the feature's hard precondition
        // (hello-daemon-system reachable, which only runs once SDDM
        // face-login is enabled) isn't met — matches this app's existing
        // "disable and explain" convention (see Home.qml's SDDM card).
        Label {
            visible: !AppController.passwordCacheAvailable
            text: I18n.tr("cachePassword.unavailableNote")
            font.pixelSize: 12
            color: Kirigami.Theme.negativeTextColor
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }

        ColumnLayout {
            visible: AppController.passwordCacheAvailable
            spacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing

            Label {
                text: I18n.tr("cachePassword.passwordLabel")
                font.weight: Font.DemiBold
                font.pixelSize: 12
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing

                TextField {
                    id: passwordField
                    Layout.fillWidth: true
                    echoMode: revealPasswordButton.checked ? TextInput.Normal : TextInput.Password
                    placeholderText: I18n.tr("cachePassword.passwordPlaceholder")
                    enabled: !AppController.passwordCacheBusy
                    onAccepted: confirmButton.clicked()
                }

                ToolButton {
                    id: revealPasswordButton
                    checkable: true
                    enabled: !AppController.passwordCacheBusy
                    icon.name: checked ? "view-visible-off-symbolic" : "view-visible-symbolic"
                    ToolTip.visible: hovered
                    ToolTip.text: checked ? I18n.tr("cachePassword.hidePassword") : I18n.tr("cachePassword.showPassword")
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: Kirigami.Units.smallSpacing

                Button {
                    id: confirmButton
                    text: AppController.passwordCacheBusy ? I18n.tr("cachePassword.busy") : I18n.tr("cachePassword.confirmButton")
                    enabled: passwordField.text.length > 0 && !AppController.passwordCacheBusy
                    onClicked: {
                        cachePasswordPage.submitted = true;
                        successLabel.visible = false;
                        AppController.submitCachePassword(passwordField.text);
                    }
                }

                BusyIndicator {
                    Layout.preferredWidth: Kirigami.Units.gridUnit
                    Layout.preferredHeight: Kirigami.Units.gridUnit
                    visible: AppController.passwordCacheBusy
                    running: AppController.passwordCacheBusy
                }

                Item { Layout.fillWidth: true }

                Button {
                    text: I18n.tr("cachePassword.cancelButton")
                    enabled: !AppController.passwordCacheBusy
                    onClicked: AppController.navigateToHomeImpl()
                }
            }
        }

        Label {
            id: successLabel
            visible: false
            text: "✓ " + I18n.tr("cachePassword.successMessage")
            font.pixelSize: 12
            color: Kirigami.Theme.positiveTextColor
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }

        Label {
            visible: AppController.passwordCacheError !== ""
            text: AppController.passwordCacheError
            font.pixelSize: 11
            color: Kirigami.Theme.negativeTextColor
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }
    }
}
