import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Linux.Hello 1.0

Kirigami.Page {
    id: aboutPage

    title: I18n.tr("about.title")

    Layout.fillWidth: true
    Layout.fillHeight: true

    padding: Kirigami.Units.largeSpacing
    topPadding: Kirigami.Units.largeSpacing * 3

    ColumnLayout {
        anchors.fill: parent
        spacing: Kirigami.Units.largeSpacing * 1.5

        ColumnLayout {
            spacing: Kirigami.Units.smallSpacing / 2
            Layout.alignment: Qt.AlignHCenter
            Layout.bottomMargin: Kirigami.Units.smallSpacing

            Image {
                source: "icons/app-icon.svg"
                Layout.preferredWidth: Kirigami.Units.gridUnit * 2.6
                Layout.preferredHeight: Kirigami.Units.gridUnit * 2.6
                Layout.alignment: Qt.AlignHCenter
                Layout.bottomMargin: Kirigami.Units.smallSpacing
                sourceSize.width: width
                sourceSize.height: height
                fillMode: Image.PreserveAspectFit
            }

            Label {
                text: "Linux Hello"
                font.pixelSize: 20
                font.weight: Font.Bold
                font.letterSpacing: -0.2
                color: Kirigami.Theme.textColor
                Layout.alignment: Qt.AlignHCenter
            }

            // Pulled from Cargo.toml via AppController.appVersion (see
            // main.rs's APP_VERSION) — never hand-typed, so a release bump
            // can't leave this screen showing a stale number.
            Rectangle {
                visible: AppController.appVersion !== ""
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: 2
                radius: height / 2
                color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.15)
                implicitWidth: versionLabel.implicitWidth + Kirigami.Units.largeSpacing
                implicitHeight: versionLabel.implicitHeight + Kirigami.Units.smallSpacing * 0.8

                Label {
                    id: versionLabel
                    anchors.centerIn: parent
                    text: I18n.tr("about.version").replace("%1", AppController.appVersion)
                    font.pixelSize: 12
                    font.weight: Font.DemiBold
                    font.family: "monospace"
                    color: Kirigami.Theme.highlightColor
                }
            }

            Label {
                text: I18n.tr("about.tagline")
                textFormat: Text.StyledText
                font.pixelSize: 12
                color: Kirigami.Theme.disabledTextColor
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: Kirigami.Units.smallSpacing
                Layout.maximumWidth: Kirigami.Units.gridUnit * 18
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
            }
        }

        // Info card — same neutral card shape as Home's status card (plain
        // Rectangle + 1px 15%-opacity border, not Kirigami.Card; see
        // Home.qml's comment on why).
        Rectangle {
            Layout.fillWidth: true
            radius: Kirigami.Units.smallSpacing * 1.4
            color: Kirigami.Theme.backgroundColor
            border.width: 1
            border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.15)
            implicitHeight: infoColumn.implicitHeight
            clip: true

            ColumnLayout {
                id: infoColumn
                width: parent.width
                spacing: 0

                AboutInfoRow {
                    iconSource: "license-symbolic"
                    label: I18n.tr("about.license")
                    value: AppController.appLicense
                    showChevron: true
                    roundTop: true
                    onClicked: AppController.navigateToLicenseImpl()
                }
                Kirigami.Separator {
                    Layout.fillWidth: true
                }
                AboutInfoRow {
                    iconSource: "im-user-symbolic"
                    label: I18n.tr("about.author")
                    value: AppController.appAuthor
                }
                Kirigami.Separator {
                    Layout.fillWidth: true
                }
                AboutInfoRow {
                    iconSource: "link-symbolic"
                    label: I18n.tr("about.repository")
                    value: "github.com/Aarklendoia/linux-hello"
                    showChevron: true
                    roundBottom: true
                    onClicked: Qt.openUrlExternally("https://github.com/Aarklendoia/linux-hello")
                }
            }
        }

        Item { Layout.fillHeight: true }

        Label {
            text: I18n.tr("about.builtWith")
            font.pixelSize: 10
            color: Kirigami.Theme.disabledTextColor
            Layout.alignment: Qt.AlignHCenter
            Layout.bottomMargin: Kirigami.Units.smallSpacing
        }
    }
}
