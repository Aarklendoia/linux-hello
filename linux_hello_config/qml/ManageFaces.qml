pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Linux.Hello 1.0

Kirigami.Page {
    id: manageFacesPage
    title: I18n.tr("manageFaces.title")

    property var appController: null

    Layout.fillWidth: true
    Layout.fillHeight: true

    ColumnLayout {
        anchors {
            fill: parent
            margins: Kirigami.Units.largeSpacing
        }
        spacing: Kirigami.Units.largeSpacing

        // Title
        Label {
            text: I18n.tr("manageFaces.registeredFaces")
            font.pixelSize: 20
            font.weight: Font.Bold
            color: Kirigami.Theme.textColor
        }

        // Faces list
        ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true

            ListView {
                id: facesListView
                model: manageFacesPage.appController.facesList

                delegate: ItemDelegate {
                    id: faceItem
                    required property var modelData

                    width: manageFacesPage.width - 2 * Kirigami.Units.largeSpacing
                    height: Kirigami.Units.gridUnit * 4

                    RowLayout {
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.smallSpacing
                        spacing: Kirigami.Units.mediumSpacing * 1.5

                        // Thumbnail (placeholder)
                        Rectangle {
                            Layout.preferredWidth: Kirigami.Units.gridUnit * 2.5
                            Layout.preferredHeight: Kirigami.Units.gridUnit * 2.5
                            color: Kirigami.Theme.highlightColor
                            radius: 4

                            Label {
                                anchors.centerIn: parent
                                text: "👤"
                                font.pixelSize: 24
                            }
                        }

                        // Info
                        ColumnLayout {
                            spacing: Kirigami.Units.smallSpacing * 1.5
                            Layout.fillWidth: true

                            Label {
                                text: (faceItem.modelData.context || "face") + " — " + (faceItem.modelData.face_id || "").substring(0, 12)
                                font.weight: Font.Bold
                                color: Kirigami.Theme.textColor
                                elide: Text.ElideRight
                            }

                            Label {
                                text: I18n.tr("manageFaces.confidence") + " " + Math.round((faceItem.modelData.quality_score || 0) * 100) + "%"
                                font.pixelSize: 11
                                color: Kirigami.Theme.disabledTextColor
                            }

                            Label {
                                text: I18n.tr("manageFaces.registered") + " " + (faceItem.modelData.registered_at ? new Date(faceItem.modelData.registered_at * 1000).toLocaleDateString() : I18n.tr("manageFaces.unknown"))
                                font.pixelSize: 10
                                color: Kirigami.Theme.disabledTextColor
                            }
                        }

                        // Delete button
                        Button {
                            text: I18n.tr("manageFaces.deleteBtn")
                            implicitHeight: Kirigami.Units.gridUnit * 2

                            onClicked: manageFacesPage.appController.deleteFace(faceItem.modelData.face_id)
                        }
                    }
                }

                // Message if no faces
                Label {
                    visible: manageFacesPage.appController.facesList.length === 0
                    text: I18n.tr("manageFaces.noFaces")
                    color: Kirigami.Theme.disabledTextColor
                    horizontalAlignment: Text.AlignHCenter
                    anchors.centerIn: parent
                }
            }
        }

        // Action buttons
        RowLayout {
            spacing: Kirigami.Units.mediumSpacing * 1.5
            Layout.fillWidth: true

            Button {
                text: I18n.tr("manageFaces.registerNewBtn")
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.2

                palette.buttonText: Kirigami.Theme.highlightedTextColor
                palette.button: Kirigami.Theme.highlightColor

                onClicked: manageFacesPage.appController.navigateToEnrollImpl()
            }

            Button {
                text: I18n.tr("manageFaces.backBtn")
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.2
                onClicked: manageFacesPage.appController.navigateToHomeImpl()
            }
        }
    }
}
