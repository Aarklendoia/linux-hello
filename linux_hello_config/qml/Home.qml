import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Linux.Hello 1.0

// ScrollablePage rather than plain Page: this page's content already grows
// with the number of action cards (Register/Manage/Login screen/Cache
// password, and possibly more later), and a fixed-height, non-scrolling
// Page just clips whatever doesn't fit — confirmed on a real run once the
// "Cache session password" card was added, which pushed the fallback note
// below the window's bottom edge with no way to reach it.
Kirigami.ScrollablePage {
    id: homePage

    title: I18n.tr("home.title")

    // Properties for pageStack
    Layout.fillWidth: true
    Layout.fillHeight: true

    // Standard KDE placement for an "About" entry — an icon in the page's
    // own toolbar, always reachable regardless of daemon/SDDM state, rather
    // than another row competing with the primary actions below.
    actions: [
        Kirigami.Action {
            icon.name: "help-about-symbolic"
            text: I18n.tr("about.title")
            onTriggered: AppController.navigateToAboutImpl()
        }
    ]

    padding: Kirigami.Units.largeSpacing
    topPadding: Kirigami.Units.largeSpacing * 5

    // Resolves AppController.sddmError for display. Most of that property's
    // possible values are untranslated error *codes* set by AppController
    // (which can't import I18n itself without a self-referential module
    // import) — this maps them to localized text. `resp.error` (free-form
    // text from install-pam.sh's stderr, passed straight through) doesn't
    // match any of these prefixes and falls through unchanged.
    function sddmErrorText(code) {
        if (code === "sddm-error:unknown")
            return I18n.tr("home.sddmErrorUnknown");
        if (code === "sddm-error:invalid-response")
            return I18n.tr("home.sddmErrorInvalidResponse");
        if (code.indexOf("sddm-error:http:") === 0)
            return I18n.tr("home.sddmErrorHttp").replace("%1", code.substring("sddm-error:http:".length));
        return code;
    }

    // Same for AppController.modelsError; the generic texts are shared with
    // the SDDM toggle's.
    function modelsErrorText(code) {
        if (code.indexOf("models-error:") === 0)
            return sddmErrorText("sddm-error:" + code.substring("models-error:".length));
        return code;
    }

    ColumnLayout {
        // Width-only, not anchors.fill: a ScrollablePage's direct child must
        // size its own height from its content (the sum of its children) so
        // the wrapping Flickable knows how far there is to scroll — filling
        // the viewport's height here would make it always exactly as tall
        // as the visible area, which is what caused the clipping this
        // ScrollablePage change fixes in the first place.
        width: parent.width
        spacing: Kirigami.Units.largeSpacing * 1.5

        // Hero mark — the project's own app icon (face-ID corners + verified
        // badge), not a generic Breeze icon standing in for it.
        Image {
            source: "icons/app-icon.svg"
            Layout.preferredWidth: Kirigami.Units.gridUnit * 3.4
            Layout.preferredHeight: Kirigami.Units.gridUnit * 3.4
            Layout.alignment: Qt.AlignHCenter
            sourceSize.width: width
            sourceSize.height: height
            fillMode: Image.PreserveAspectFit
        }

        ColumnLayout {
            spacing: Kirigami.Units.smallSpacing / 2
            Layout.alignment: Qt.AlignHCenter
            Layout.bottomMargin: Kirigami.Units.largeSpacing * 2

            Label {
                text: "Linux Hello"
                font.pixelSize: 26
                font.weight: Font.Bold
                font.letterSpacing: -0.3
                color: Kirigami.Theme.textColor
                Layout.alignment: Qt.AlignHCenter
            }

            Label {
                text: I18n.tr("app.subtitle")
                textFormat: Text.StyledText
                font.pixelSize: 13
                color: Kirigami.Theme.disabledTextColor
                Layout.alignment: Qt.AlignHCenter
                Layout.maximumWidth: Kirigami.Units.gridUnit * 20
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
            }
        }

        // Status card — real daemon liveness + real enrolled-face count,
        // both refreshed via AppController.navigateToHomeImpl(). A plain
        // Rectangle instead of Kirigami.Card: the Card template reserves
        // asymmetric top/bottom inset space for its hover shadow, which was
        // throwing off vertical centering of the icon+text row no matter
        // what alignment was set on them.
        Rectangle {
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
            Layout.bottomMargin: Kirigami.Units.largeSpacing
            implicitHeight: statusRow.implicitHeight + Kirigami.Units.largeSpacing * 2
            radius: Kirigami.Units.smallSpacing * 1.4
            color: Kirigami.Theme.backgroundColor
            border.width: 1
            border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.15)

            RowLayout {
                id: statusRow
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.largeSpacing * 0.8

                Rectangle {
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 1.9
                    Layout.preferredHeight: Kirigami.Units.gridUnit * 1.9
                    Layout.alignment: Qt.AlignVCenter
                    radius: width * 0.26
                    color: AppController.daemonActive
                        ? Qt.rgba(Kirigami.Theme.positiveTextColor.r, Kirigami.Theme.positiveTextColor.g, Kirigami.Theme.positiveTextColor.b, 0.15)
                        : Qt.rgba(Kirigami.Theme.neutralTextColor.r, Kirigami.Theme.neutralTextColor.g, Kirigami.Theme.neutralTextColor.b, 0.15)

                    Kirigami.Icon {
                        anchors.centerIn: parent
                        width: Kirigami.Units.gridUnit
                        height: width
                        source: AppController.daemonActive ? "checkmark-symbolic" : "dialog-warning"
                        color: AppController.daemonActive ? Kirigami.Theme.positiveTextColor : Kirigami.Theme.neutralTextColor
                        isMask: true
                    }
                }

                ColumnLayout {
                    spacing: 1
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignVCenter

                    Label {
                        text: AppController.daemonActive ? I18n.tr("home.daemonActive") : I18n.tr("home.daemonInactive")
                        font.weight: Font.DemiBold
                        font.pixelSize: 13
                        color: Kirigami.Theme.textColor
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                    }
                    Label {
                        text: AppController.daemonActive ? I18n.tr("home.daemonActiveSub") : I18n.tr("home.daemonInactiveSub")
                        font.pixelSize: 11
                        color: Kirigami.Theme.disabledTextColor
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                    }
                }
            }
        }

        // Face models notice (#211): Linux Hello doesn't ship them, since
        // InsightFace's terms only allow non-commercial research use. Shown
        // until they're installed, with those terms spelled out before the
        // button that downloads them (pkexec linux-hello models install
        // --accept-license — accepting here is what that flag stands for).
        Rectangle {
            visible: !AppController.modelsInstalled
            Layout.fillWidth: true
            implicitHeight: modelsColumn.implicitHeight + Kirigami.Units.largeSpacing * 2
            radius: Kirigami.Units.smallSpacing * 1.4
            color: Qt.rgba(Kirigami.Theme.neutralTextColor.r, Kirigami.Theme.neutralTextColor.g, Kirigami.Theme.neutralTextColor.b, 0.08)
            border.width: 1
            border.color: Qt.rgba(Kirigami.Theme.neutralTextColor.r, Kirigami.Theme.neutralTextColor.g, Kirigami.Theme.neutralTextColor.b, 0.4)

            ColumnLayout {
                id: modelsColumn
                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.smallSpacing * 1.5

                RowLayout {
                    spacing: Kirigami.Units.smallSpacing * 1.5
                    Layout.fillWidth: true

                    Kirigami.Icon {
                        source: "dialog-warning"
                        Layout.preferredWidth: Kirigami.Units.iconSizes.small
                        Layout.preferredHeight: Kirigami.Units.iconSizes.small
                        color: Kirigami.Theme.neutralTextColor
                        isMask: true
                    }
                    Label {
                        text: I18n.tr("home.modelsTitle")
                        font.weight: Font.DemiBold
                        font.pixelSize: 13
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                    }
                }

                Label {
                    text: I18n.tr("home.modelsBody")
                    font.pixelSize: 12
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                }

                Label {
                    text: "<a href=\"https://github.com/deepinsight/insightface#license\">" + I18n.tr("home.modelsTermsLink") + "</a>"
                    textFormat: Text.StyledText
                    font.pixelSize: 12
                    onLinkActivated: function (link) {
                        Qt.openUrlExternally(link);
                    }
                    HoverHandler {
                        cursorShape: Qt.PointingHandCursor
                    }
                }

                RowLayout {
                    spacing: Kirigami.Units.smallSpacing * 1.5
                    Layout.fillWidth: true

                    Button {
                        visible: AppController.modelsAvailable
                        enabled: !AppController.modelsBusy
                        text: AppController.modelsBusy ? I18n.tr("home.modelsBusy") : I18n.tr("home.modelsAcceptBtn")
                        icon.name: "download"
                        onClicked: AppController.installModels()
                    }
                    BusyIndicator {
                        visible: AppController.modelsBusy
                        running: AppController.modelsBusy
                        Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
                        Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
                    }
                    Label {
                        visible: !AppController.modelsAvailable
                        text: I18n.tr("home.modelsUnavailable")
                        font.pixelSize: 12
                        color: Kirigami.Theme.disabledTextColor
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                    }
                }
            }

            Connections {
                target: AppController
                function onModelsErrorChanged() {
                    if (AppController.modelsError !== "")
                        applicationWindow().showPassiveNotification(modelsErrorText(AppController.modelsError));
                }
                function onModelsInstalledChanged() {
                    if (AppController.modelsInstalled)
                        applicationWindow().showPassiveNotification(I18n.tr("home.modelsInstalled"));
                }
            }
        }

        // Action cards — same neutral card style for both; only the icon
        // badge is accent-filled on the primary one. A solid-blue card here
        // used to read as a permanently "selected" list row rather than a
        // normal button. Shared shape factored into ActionCard.qml.
        ColumnLayout {
            spacing: Kirigami.Units.smallSpacing * 1.3
            Layout.fillWidth: true

            ActionCard {
                // Enrolling without the models can't work: the daemon
                // refuses to capture (see the notice above).
                enabled: AppController.modelsInstalled
                iconSource: "camera-photo-symbolic"
                iconColor: Kirigami.Theme.highlightedTextColor
                badgeColor: Kirigami.Theme.highlightColor
                title: I18n.tr("home.registerBtn")
                subtitle: AppController.modelsInstalled ? I18n.tr("home.registerBtnDesc") : I18n.tr("home.registerBtnNoModels")
                onClicked: AppController.navigateToEnrollImpl()
            }

            ActionCard {
                iconSource: "system-users-symbolic"
                iconColor: Kirigami.Theme.highlightColor
                badgeColor: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.15)
                title: I18n.tr("home.manageFacesBtn")
                subtitle: {
                    var n = AppController.facesList.length;
                    if (n === 0)
                        return I18n.tr("manageFaces.noFaces");
                    if (n === 1)
                        return I18n.tr("home.manageFacesBtnDescOne");
                    return I18n.tr("home.manageFacesBtnDesc").replace("%1", n);
                }
                onClicked: AppController.navigateToManageFacesImpl()
            }

            // SDDM (login screen) toggle — unlike the two cards above, this
            // one is a direct action, not navigation to a sub-page: clicking
            // it enables/disables face auth on the SDDM login screen right
            // away, via a real pkexec prompt on the backend (can take
            // several seconds — the user has to interact with the dialog).
            // Its trailing element is a busy spinner instead of the default
            // chevron, since there's no sub-page to navigate to.
            ActionCard {
                enabled: AppController.sddmAvailable && !AppController.sddmBusy
                iconSource: "system-switch-user-symbolic"
                iconColor: AppController.sddmActive ? Kirigami.Theme.positiveTextColor : Kirigami.Theme.highlightColor
                badgeColor: AppController.sddmActive
                    ? Qt.rgba(Kirigami.Theme.positiveTextColor.r, Kirigami.Theme.positiveTextColor.g, Kirigami.Theme.positiveTextColor.b, 0.15)
                    : Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.15)
                title: I18n.tr("home.sddmTitle")
                subtitle: {
                    if (!AppController.sddmAvailable)
                        return I18n.tr("home.sddmUnavailableSub");
                    if (AppController.sddmBusy)
                        return I18n.tr("home.sddmBusySub");
                    return AppController.sddmActive ? I18n.tr("home.sddmActiveSub") : I18n.tr("home.sddmInactiveSub");
                }
                trailingComponent: Component {
                    BusyIndicator {
                        Layout.preferredWidth: Kirigami.Units.gridUnit
                        Layout.preferredHeight: Kirigami.Units.gridUnit
                        visible: AppController.sddmBusy
                        running: AppController.sddmBusy
                    }
                }
                onClicked: AppController.toggleSddm()
            }

            // SDDM toggle feedback — a passive (auto-dismissing) notification
            // rather than a permanent inline line, using Kirigami's own
            // toast mechanism (no custom component needed). Cancelling
            // pkexec's own prompt is treated as a silent no-op by
            // toggleSddm() itself (see AppController.qml) — indistinguishable
            // from a genuine auth failure at the process-exit-code level, but
            // by far the more common reason this fires, so it's not worth
            // alarming the user over. This notification only ever fires for
            // an actual failure (script error, unreachable helper, etc.).
            Connections {
                target: AppController
                function onSddmErrorChanged() {
                    if (AppController.sddmError !== "")
                        applicationWindow().showPassiveNotification(sddmErrorText(AppController.sddmError));
                }
            }

            // SDDM only starts checking once a login attempt is actually
            // submitted (pressing Enter/clicking the login button) — it
            // can't scan passively just from the greeter being on screen,
            // since PAM itself only runs at that point. Not obvious from
            // the greeter alone (confirmed: a real user tried it and asked
            // "how do I explain this?"), so spell it out here, directly
            // under the SDDM card it explains rather than after every card
            // on the page (where an unrelated card could end up sitting
            // between the two, as the Cache password card originally did).
            RowLayout {
                visible: AppController.sddmActive
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing

                Kirigami.Icon {
                    source: "info-symbolic"
                    width: Kirigami.Units.gridUnit * 0.9
                    height: width
                    color: Kirigami.Theme.disabledTextColor
                    isMask: true
                    Layout.alignment: Qt.AlignTop
                }
                Label {
                    text: I18n.tr("home.sddmHowToNote")
                    font.pixelSize: 10
                    color: Kirigami.Theme.disabledTextColor
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }
            }

            // Session-password cache — navigates to a sub-page (the consent
            // warning + password field don't fit a one-line card action),
            // unlike the SDDM toggle above which acts immediately. Disabled
            // until SDDM face-login is enabled: hello-daemon-system, the
            // only process with TPM access, doesn't run otherwise.
            ActionCard {
                enabled: AppController.passwordCacheAvailable
                iconSource: "dialog-password-symbolic"
                iconColor: AppController.passwordCacheActive ? Kirigami.Theme.positiveTextColor : Kirigami.Theme.highlightColor
                badgeColor: AppController.passwordCacheActive
                    ? Qt.rgba(Kirigami.Theme.positiveTextColor.r, Kirigami.Theme.positiveTextColor.g, Kirigami.Theme.positiveTextColor.b, 0.15)
                    : Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.15)
                title: I18n.tr("home.cachePasswordTitle")
                subtitle: {
                    if (!AppController.passwordCacheAvailable)
                        return I18n.tr("home.cachePasswordUnavailableSub");
                    return AppController.passwordCacheActive ? I18n.tr("home.cachePasswordActiveSub") : I18n.tr("home.cachePasswordInactiveSub");
                }
                onClicked: AppController.navigateToCachePasswordImpl()
            }
        }

    }

    // Password-fallback reassurance — pinned in the page's `footer` rather
    // than living in the scrolling ColumnLayout above, so it always sits at
    // the window's bottom edge regardless of how many action cards fit
    // above the fold, instead of just trailing the last card in the
    // scrollable content (which pushed it out of sight once a 4th card was
    // added, and left it "floating" mid-page on a tall window otherwise).
    footer: Rectangle {
        implicitHeight: fallbackRow.implicitHeight + Kirigami.Units.largeSpacing * 2
        color: Kirigami.Theme.backgroundColor
        border.width: 1
        border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.08)

        RowLayout {
            id: fallbackRow
            anchors.fill: parent
            anchors.margins: Kirigami.Units.largeSpacing
            spacing: Kirigami.Units.smallSpacing

            Kirigami.Icon {
                source: "info-symbolic"
                width: Kirigami.Units.gridUnit * 0.9
                height: width
                color: Kirigami.Theme.disabledTextColor
                isMask: true
                Layout.alignment: Qt.AlignTop
            }
            Label {
                text: I18n.tr("home.fallbackNote")
                font.pixelSize: 10
                color: Kirigami.Theme.disabledTextColor
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
        }
    }
}
