import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Shared row shape for About.qml's info card (license / author / ...):
// small icon + label/value pair, optionally clickable with a trailing
// chevron. Extracted for the same reason ActionCard.qml was: three rows
// hand-rolling the same RowLayout/Label structure.
AbstractButton {
    id: row

    property string iconSource
    property string label
    property string value
    property bool showChevron: false
    // Set by the row that sits at the top/bottom edge of the enclosing
    // card, so its hover background can round the matching corners instead
    // of poking square corners past the card's own rounded outline (a plain
    // Rectangle's `clip: true` only clips to its bounding box, not to its
    // rounded silhouette — the card can't do this clipping for us).
    property bool roundTop: false
    property bool roundBottom: false

    Layout.fillWidth: true
    implicitHeight: rowContent.implicitHeight + Kirigami.Units.largeSpacing * 1.1
    // Not `enabled: showChevron` — Breeze's disabled-button palette dims
    // every child (including the icon badge, already a subtle 15% wash) to
    // the point of invisibility. A non-clickable row should still look
    // normal: only its hover feedback is conditional here: whether a click
    // does anything is entirely down to whether the caller (About.qml)
    // bothered to bind onClicked at all — the Author row never does.
    hoverEnabled: showChevron

    background: Rectangle {
        // Not Theme.hoverColor: see ActionCard.qml's identical fix — it's a
        // solid saturated blue that swallows the same-hue icon badge.
        color: row.hovered ? Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.06) : "transparent"
        topLeftRadius: row.roundTop ? Kirigami.Units.smallSpacing * 1.4 : 0
        topRightRadius: row.roundTop ? Kirigami.Units.smallSpacing * 1.4 : 0
        bottomLeftRadius: row.roundBottom ? Kirigami.Units.smallSpacing * 1.4 : 0
        bottomRightRadius: row.roundBottom ? Kirigami.Units.smallSpacing * 1.4 : 0
        Behavior on color { ColorAnimation { duration: 120 } }
    }

    contentItem: RowLayout {
        id: rowContent
        anchors.fill: parent
        anchors.leftMargin: Kirigami.Units.largeSpacing * 0.8
        anchors.rightMargin: Kirigami.Units.largeSpacing * 0.8
        spacing: Kirigami.Units.largeSpacing * 0.7

        Rectangle {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 1.5
            Layout.preferredHeight: Kirigami.Units.gridUnit * 1.5
            radius: Kirigami.Units.smallSpacing
            // Same washed-highlight + solid-highlight-icon combo as Home's
            // "Manage Faces" card (ActionCard.qml) — Theme.hoverColor +
            // disabledTextColor here previously left the glyph nearly
            // invisible against its own badge (too little contrast between
            // the two muted tones).
            color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.15)

            Kirigami.Icon {
                anchors.centerIn: parent
                width: Kirigami.Units.gridUnit * 0.75
                height: width
                source: row.iconSource
                color: Kirigami.Theme.highlightColor
                isMask: true
            }
        }

        ColumnLayout {
            spacing: 1
            Layout.fillWidth: true

            Label {
                text: row.label
                font.pixelSize: 9
                font.weight: Font.DemiBold
                font.letterSpacing: 0.3
                color: Kirigami.Theme.disabledTextColor
                Layout.fillWidth: true
                elide: Text.ElideRight
            }
            Label {
                text: row.value
                font.pixelSize: 12
                font.weight: Font.Medium
                color: Kirigami.Theme.textColor
                Layout.fillWidth: true
                elide: Text.ElideRight
            }
        }

        Kirigami.Icon {
            visible: row.showChevron
            source: "go-next-symbolic"
            Layout.preferredWidth: Kirigami.Units.gridUnit * 0.85
            Layout.preferredHeight: Kirigami.Units.gridUnit * 0.85
            color: Kirigami.Theme.disabledTextColor
            isMask: true
        }
    }
}
