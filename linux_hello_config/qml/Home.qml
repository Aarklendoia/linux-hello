import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Linux.Hello 1.0

Kirigami.Page {
    id: homePage

    title: I18n.tr("home.title")

    // Properties for pageStack
    Layout.fillWidth: true
    Layout.fillHeight: true

    padding: Kirigami.Units.largeSpacing
    topPadding: Kirigami.Units.largeSpacing * 5

    ColumnLayout {
        anchors.fill: parent
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

        // Action cards — same neutral card style for both; only the icon
        // badge is accent-filled on the primary one. A solid-blue card here
        // used to read as a permanently "selected" list row rather than a
        // normal button.
        ColumnLayout {
            spacing: Kirigami.Units.smallSpacing * 1.3
            Layout.fillWidth: true

            AbstractButton {
                id: enrollCard
                Layout.fillWidth: true
                implicitHeight: enrollRow.implicitHeight + Kirigami.Units.largeSpacing * 1.6
                onClicked: AppController.navigateToEnrollImpl()

                background: Rectangle {
                    radius: Kirigami.Units.smallSpacing * 1.4
                    color: enrollCard.hovered ? Kirigami.Theme.hoverColor : Kirigami.Theme.backgroundColor
                    border.width: 1
                    border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.15)
                    Behavior on color { ColorAnimation { duration: 120 } }
                }

                contentItem: RowLayout {
                    id: enrollRow
                    anchors.fill: parent
                    anchors.margins: Kirigami.Units.largeSpacing * 0.8
                    spacing: Kirigami.Units.largeSpacing * 0.8

                    Rectangle {
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 2.1
                        Layout.preferredHeight: Kirigami.Units.gridUnit * 2.1
                        radius: width * 0.26
                        color: Kirigami.Theme.highlightColor

                        Kirigami.Icon {
                            anchors.centerIn: parent
                            width: Kirigami.Units.gridUnit * 1.05
                            height: width
                            source: "camera-photo-symbolic"
                            color: Kirigami.Theme.highlightedTextColor
                            isMask: true
                        }
                    }
                    ColumnLayout {
                        spacing: 1
                        Layout.fillWidth: true
                        Label {
                            text: I18n.tr("home.registerBtn")
                            font.weight: Font.DemiBold
                            font.pixelSize: 14
                            color: Kirigami.Theme.textColor
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                        }
                        Label {
                            text: I18n.tr("home.registerBtnDesc")
                            font.pixelSize: 11
                            color: Kirigami.Theme.disabledTextColor
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                        }
                    }
                    Kirigami.Icon {
                        source: "go-next-symbolic"
                        Layout.preferredWidth: Kirigami.Units.gridUnit
                        Layout.preferredHeight: Kirigami.Units.gridUnit
                        color: Kirigami.Theme.disabledTextColor
                        isMask: true
                    }
                }
            }

            AbstractButton {
                id: manageCard
                Layout.fillWidth: true
                implicitHeight: manageRow.implicitHeight + Kirigami.Units.largeSpacing * 1.6
                onClicked: AppController.navigateToManageFacesImpl()

                background: Rectangle {
                    radius: Kirigami.Units.smallSpacing * 1.4
                    color: manageCard.hovered ? Kirigami.Theme.hoverColor : Kirigami.Theme.backgroundColor
                    border.width: 1
                    border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.15)
                    Behavior on color { ColorAnimation { duration: 120 } }
                }

                contentItem: RowLayout {
                    id: manageRow
                    anchors.fill: parent
                    anchors.margins: Kirigami.Units.largeSpacing * 0.8
                    spacing: Kirigami.Units.largeSpacing * 0.8

                    Rectangle {
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 2.1
                        Layout.preferredHeight: Kirigami.Units.gridUnit * 2.1
                        radius: width * 0.26
                        color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.15)

                        Kirigami.Icon {
                            anchors.centerIn: parent
                            width: Kirigami.Units.gridUnit * 1.05
                            height: width
                            source: "system-users-symbolic"
                            color: Kirigami.Theme.highlightColor
                            isMask: true
                        }
                    }
                    ColumnLayout {
                        spacing: 1
                        Layout.fillWidth: true
                        Label {
                            text: I18n.tr("home.manageFacesBtn")
                            font.weight: Font.DemiBold
                            font.pixelSize: 14
                            color: Kirigami.Theme.textColor
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                        }
                        Label {
                            text: {
                                var n = AppController.facesList.length;
                                if (n === 0)
                                    return I18n.tr("manageFaces.noFaces");
                                if (n === 1)
                                    return I18n.tr("home.manageFacesBtnDescOne");
                                return I18n.tr("home.manageFacesBtnDesc").replace("%1", n);
                            }
                            font.pixelSize: 11
                            color: Kirigami.Theme.disabledTextColor
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                        }
                    }
                    Kirigami.Icon {
                        source: "go-next-symbolic"
                        Layout.preferredWidth: Kirigami.Units.gridUnit
                        Layout.preferredHeight: Kirigami.Units.gridUnit
                        color: Kirigami.Theme.disabledTextColor
                        isMask: true
                    }
                }
            }

            // SDDM (login screen) toggle — unlike the two cards above, this
            // one is a direct action, not navigation to a sub-page: clicking
            // it enables/disables face auth on the SDDM login screen right
            // away, via a real pkexec prompt on the backend (can take
            // several seconds — the user has to interact with the dialog).
            AbstractButton {
                id: sddmCard
                Layout.fillWidth: true
                implicitHeight: sddmRow.implicitHeight + Kirigami.Units.largeSpacing * 1.6
                enabled: AppController.sddmAvailable && !AppController.sddmBusy
                onClicked: AppController.toggleSddm()

                background: Rectangle {
                    radius: Kirigami.Units.smallSpacing * 1.4
                    color: sddmCard.hovered ? Kirigami.Theme.hoverColor : Kirigami.Theme.backgroundColor
                    border.width: 1
                    border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.15)
                    opacity: sddmCard.enabled ? 1 : 0.6
                    Behavior on color { ColorAnimation { duration: 120 } }
                }

                contentItem: RowLayout {
                    id: sddmRow
                    anchors.fill: parent
                    anchors.margins: Kirigami.Units.largeSpacing * 0.8
                    spacing: Kirigami.Units.largeSpacing * 0.8

                    Rectangle {
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 2.1
                        Layout.preferredHeight: Kirigami.Units.gridUnit * 2.1
                        radius: width * 0.26
                        color: AppController.sddmActive
                            ? Qt.rgba(Kirigami.Theme.positiveTextColor.r, Kirigami.Theme.positiveTextColor.g, Kirigami.Theme.positiveTextColor.b, 0.15)
                            : Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.15)

                        Kirigami.Icon {
                            anchors.centerIn: parent
                            width: Kirigami.Units.gridUnit * 1.05
                            height: width
                            source: "system-switch-user-symbolic"
                            color: AppController.sddmActive ? Kirigami.Theme.positiveTextColor : Kirigami.Theme.highlightColor
                            isMask: true
                        }
                    }
                    ColumnLayout {
                        spacing: 1
                        Layout.fillWidth: true
                        Label {
                            text: I18n.tr("home.sddmTitle")
                            font.weight: Font.DemiBold
                            font.pixelSize: 14
                            color: Kirigami.Theme.textColor
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                        }
                        Label {
                            text: {
                                if (!AppController.sddmAvailable)
                                    return I18n.tr("home.sddmUnavailableSub");
                                if (AppController.sddmBusy)
                                    return I18n.tr("home.sddmBusySub");
                                return AppController.sddmActive ? I18n.tr("home.sddmActiveSub") : I18n.tr("home.sddmInactiveSub");
                            }
                            font.pixelSize: 11
                            color: Kirigami.Theme.disabledTextColor
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                        }
                    }
                    BusyIndicator {
                        Layout.preferredWidth: Kirigami.Units.gridUnit
                        Layout.preferredHeight: Kirigami.Units.gridUnit
                        visible: AppController.sddmBusy
                        running: AppController.sddmBusy
                    }
                }
            }
        }

        // SDDM toggle error — no toast/notification system in this app yet,
        // so a plain inline line is the simplest honest feedback for a
        // failed/cancelled pkexec attempt.
        Label {
            visible: AppController.sddmError !== ""
            text: AppController.sddmError
            font.pixelSize: 10
            color: Kirigami.Theme.negativeTextColor
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }

        // SDDM only starts checking once a login attempt is actually
        // submitted (pressing Enter/clicking the login button) — it can't
        // scan passively just from the greeter being on screen, since PAM
        // itself only runs at that point. Not obvious from the greeter
        // alone (confirmed: a real user tried it and asked "how do I
        // explain this?"), so spell it out here rather than only in docs.
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

        // Flexible spacer — only this gap is elastic, so the fallback note
        // below is pinned to the bottom of the window while everything
        // above it (hero, status, actions) keeps its natural top-down flow,
        // matching the approved mockup instead of centering the whole block.
        Item { Layout.fillHeight: true }

        // Password-fallback reassurance
        RowLayout {
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
                text: I18n.tr("home.fallbackNote")
                font.pixelSize: 10
                color: Kirigami.Theme.disabledTextColor
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
        }
    }
}
