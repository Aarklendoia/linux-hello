pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Linux.Hello 1.0

Kirigami.Page {
    id: manageFacesPage
    title: I18n.tr("manageFaces.title")

    Layout.fillWidth: true
    Layout.fillHeight: true

    // Recharger la liste à chaque fois que la page devient visible
    Component.onCompleted: AppController.loadFaces()

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
            clip: true

            ListView {
                id: facesListView
                model: AppController.facesList
                clip: true

                delegate: Item {
                    id: faceItem
                    required property var modelData
                    required property int index

                    width: ListView.view.width
                    height: Kirigami.Units.gridUnit * 4

                    readonly property string contextLabel: {
                        var ctx = modelData.context || "";
                        if (ctx === "gui")
                            return qsTr("Interface graphique");
                        if (ctx === "screenlock")
                            return qsTr("Écran de verrouillage");
                        if (ctx === "sudo")
                            return qsTr("Commande sudo");
                        if (ctx === "login")
                            return qsTr("Connexion");
                        if (ctx === "sddm")
                            return qsTr("Écran de connexion");
                        if (ctx === "test")
                            return qsTr("Test");
                        return ctx;
                    }

                    RowLayout {
                        anchors {
                            left: parent.left
                            right: parent.right
                            verticalCenter: parent.verticalCenter
                            margins: Kirigami.Units.smallSpacing
                        }
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
                                text: AppController.uidToName(faceItem.modelData.user_id) + " — " + faceItem.contextLabel + " #" + (faceItem.index + 1)
                                font.weight: Font.Bold
                                color: Kirigami.Theme.textColor
                                elide: Text.ElideRight
                                Layout.fillWidth: true
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

                        // Delete button — largeur fixe, toujours à droite
                        Button {
                            text: I18n.tr("manageFaces.deleteBtn")
                            Layout.preferredWidth: Kirigami.Units.gridUnit * 6
                            Layout.preferredHeight: Kirigami.Units.gridUnit * 2
                            Layout.alignment: Qt.AlignRight | Qt.AlignVCenter

                            onClicked: AppController.deleteFace(faceItem.modelData.face_id)
                        }
                    }
                }

                // Message if no faces
                Label {
                    visible: AppController.facesList.length === 0
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

                onClicked: AppController.navigateToEnrollImpl()
            }

            Button {
                text: I18n.tr("manageFaces.backBtn")
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.2
                onClicked: AppController.navigateToHomeImpl()
            }
        }
    }
}
