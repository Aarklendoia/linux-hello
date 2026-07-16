import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Shared shape for Home.qml's action cards (enroll / manage faces / SDDM
// toggle): icon badge + title/subtitle + a trailing element. Extracted
// because the three cards were hand-rolling the same
// AbstractButton/background/contentItem structure, varying only in icon,
// colors, text, and (for the SDDM card) a busy spinner instead of the
// usual "this navigates" chevron.
AbstractButton {
    id: card

    property string iconSource
    property color iconColor: Kirigami.Theme.highlightedTextColor
    property color badgeColor: Kirigami.Theme.highlightColor
    property string title
    property string subtitle
    // Overridden by the SDDM card, which shows a busy spinner instead of
    // the default chevron (it's a direct action, not navigation to a
    // sub-page — see the caller's own comment).
    property Component trailingComponent: chevronComponent

    Layout.fillWidth: true
    implicitHeight: cardRow.implicitHeight + Kirigami.Units.largeSpacing * 1.6

    background: Rectangle {
        radius: Kirigami.Units.smallSpacing * 1.4
        color: card.hovered ? Kirigami.Theme.hoverColor : Kirigami.Theme.backgroundColor
        border.width: 1
        border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.15)
        opacity: card.enabled ? 1 : 0.6
        Behavior on color { ColorAnimation { duration: 120 } }
    }

    contentItem: RowLayout {
        id: cardRow
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing * 0.8
        spacing: Kirigami.Units.largeSpacing * 0.8

        Rectangle {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 2.1
            Layout.preferredHeight: Kirigami.Units.gridUnit * 2.1
            radius: width * 0.26
            color: card.badgeColor

            Kirigami.Icon {
                anchors.centerIn: parent
                width: Kirigami.Units.gridUnit * 1.05
                height: width
                source: card.iconSource
                color: card.iconColor
                isMask: true
            }
        }

        ColumnLayout {
            spacing: 1
            Layout.fillWidth: true

            Label {
                text: card.title
                font.weight: Font.DemiBold
                font.pixelSize: 14
                color: Kirigami.Theme.textColor
                Layout.fillWidth: true
                elide: Text.ElideRight
            }
            Label {
                text: card.subtitle
                font.pixelSize: 11
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
                elide: Text.ElideRight
            }
        }

        Loader {
            sourceComponent: card.trailingComponent
        }
    }

    Component {
        id: chevronComponent
        Kirigami.Icon {
            source: "go-next-symbolic"
            Layout.preferredWidth: Kirigami.Units.gridUnit
            Layout.preferredHeight: Kirigami.Units.gridUnit
            color: Kirigami.Theme.disabledTextColor
            isMask: true
        }
    }
}
