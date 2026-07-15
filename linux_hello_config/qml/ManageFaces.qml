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

    padding: Kirigami.Units.largeSpacing

    // Reload the list every time the page becomes visible
    Component.onCompleted: AppController.loadFaces()

    ColumnLayout {
        anchors.fill: parent
        spacing: Kirigami.Units.largeSpacing

        Label {
            text: I18n.tr("manageFaces.registeredFaces")
            font.pixelSize: 18
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
                spacing: Kirigami.Units.smallSpacing
                clip: true

                delegate: Kirigami.Card {
                    id: faceItem
                    required property var modelData
                    required property int index

                    width: ListView.view.width
                    padding: Kirigami.Units.mediumSpacing

                    contentItem: RowLayout {
                        spacing: Kirigami.Units.largeSpacing * 0.7

                        // Avatar
                        Rectangle {
                            Layout.preferredWidth: Kirigami.Units.gridUnit * 2
                            Layout.preferredHeight: Kirigami.Units.gridUnit * 2
                            radius: Kirigami.Units.smallSpacing
                            color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.15)

                            Kirigami.Icon {
                                anchors.centerIn: parent
                                width: Kirigami.Units.gridUnit
                                height: width
                                source: "im-user-symbolic"
                                color: Kirigami.Theme.highlightColor
                                isMask: true
                            }
                        }

                        // Meta
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 3

                            RowLayout {
                                spacing: Kirigami.Units.smallSpacing * 0.6
                                Label {
                                    text: AppController.uidToName(faceItem.modelData.user_id)
                                    font.weight: Font.DemiBold
                                    font.pixelSize: 13
                                    color: Kirigami.Theme.textColor
                                    elide: Text.ElideRight
                                }
                                Label {
                                    // Enrollment sessions aren't scoped to a context — any
                                    // registered face can authenticate any context (sudo,
                                    // screenlock, ...); this just distinguishes multiple
                                    // independent enrollments for the same account.
                                    text: "· " + I18n.tr("manageFaces.sample") + " " + (faceItem.index + 1)
                                    font.pixelSize: 11
                                    color: Kirigami.Theme.disabledTextColor
                                }
                            }

                            RowLayout {
                                spacing: Kirigami.Units.largeSpacing * 0.7

                                RowLayout {
                                    spacing: Kirigami.Units.smallSpacing * 0.6
                                    Label {
                                        text: Math.round((faceItem.modelData.quality_score || 0) * 100)
                                        font.pixelSize: 11
                                        font.family: "monospace"
                                        color: Kirigami.Theme.disabledTextColor
                                    }
                                    Rectangle {
                                        Layout.preferredWidth: Kirigami.Units.gridUnit * 2.2
                                        Layout.preferredHeight: 4
                                        radius: 2
                                        color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.15)
                                        Rectangle {
                                            width: parent.width * Math.min(1, Math.max(0, faceItem.modelData.quality_score || 0))
                                            height: parent.height
                                            radius: parent.radius
                                            color: Kirigami.Theme.positiveTextColor
                                        }
                                    }
                                }

                                Label {
                                    text: faceItem.modelData.registered_at
                                        ? new Date(faceItem.modelData.registered_at * 1000).toLocaleDateString()
                                        : I18n.tr("manageFaces.unknown")
                                    font.pixelSize: 11
                                    color: Kirigami.Theme.disabledTextColor
                                }
                            }
                        }

                        // Delete
                        ToolButton {
                            icon.name: "edit-delete-symbolic"
                            display: AbstractButton.IconOnly
                            Layout.preferredWidth: Kirigami.Units.gridUnit * 1.8
                            Layout.preferredHeight: Kirigami.Units.gridUnit * 1.8
                            onClicked: AppController.deleteFace(faceItem.modelData.face_id)

                            Accessible.name: I18n.tr("manageFaces.deleteBtn")
                            ToolTip.visible: hovered
                            ToolTip.text: I18n.tr("manageFaces.deleteBtn")
                        }
                    }
                }

                // Message if no faces
                ColumnLayout {
                    visible: AppController.facesList.length === 0
                    anchors.centerIn: parent
                    spacing: Kirigami.Units.smallSpacing

                    Kirigami.Icon {
                        source: "im-user-symbolic"
                        width: Kirigami.Units.gridUnit * 2.5
                        height: width
                        color: Kirigami.Theme.disabledTextColor
                        isMask: true
                        opacity: 0.5
                        Layout.alignment: Qt.AlignHCenter
                    }
                    Label {
                        text: I18n.tr("manageFaces.noFaces")
                        color: Kirigami.Theme.disabledTextColor
                        horizontalAlignment: Text.AlignHCenter
                    }
                }
            }
        }

        Label {
            visible: AppController.facesList.length > 0
            text: I18n.tr("manageFaces.anyFaceNote")
            font.pixelSize: 10
            color: Kirigami.Theme.disabledTextColor
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }

        // Action buttons
        RowLayout {
            spacing: Kirigami.Units.mediumSpacing * 1.5
            Layout.fillWidth: true

            Button {
                text: I18n.tr("manageFaces.backBtn")
                flat: true
                onClicked: AppController.navigateToHomeImpl()
            }

            Item { Layout.fillWidth: true }

            Button {
                text: I18n.tr("manageFaces.registerNewBtn")
                highlighted: true

                palette.buttonText: Kirigami.Theme.highlightedTextColor
                palette.button: Kirigami.Theme.highlightColor

                onClicked: AppController.navigateToEnrollImpl()
            }
        }
    }
}
