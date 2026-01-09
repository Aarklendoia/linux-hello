import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Linux.Hello 1.0

Kirigami.Page {
    id: manageFacesPage
    title: I18n.tr("manageFaces.title")
    
    // Propriétés pour pageStack
    Layout.fillWidth: true
    Layout.fillHeight: true
    
    // Accès à appController depuis le contexte global
    property QtObject appController: typeof mainWindow !== 'undefined' && mainWindow ? mainWindow.appController : null
    
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
                id: facesList
                model: appController ? appController.facesList : []
                
                delegate: ItemDelegate {
                    width: manageFacesPage.width - 2 * Kirigami.Units.largeSpacing
                    height: Kirigami.Units.gridUnit * 4
                    
                    RowLayout {
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
                                text: modelData.name || "Face " + (index + 1)
                                font.weight: Font.Bold
                                color: Kirigami.Theme.textColor
                            }
                            
                            Label {
                                text: I18n.tr("manageFaces.confidence") + " " + (modelData.confidence || 85) + "%"
                                font.pixelSize: 11
                                color: Kirigami.Theme.disabledTextColor
                            }
                            
                            Label {
                                text: I18n.tr("manageFaces.registered") + " " + (modelData.date || I18n.tr("manageFaces.unknown"))
                                font.pixelSize: 10
                                color: Kirigami.Theme.disabledTextColor
                            }
                        }
                        
                        // Action buttons
                        Button {
                            text: I18n.tr("manageFaces.deleteBtn")
                            implicitHeight: Kirigami.Units.gridUnit * 2
                            
                            onClicked: mainWindow.deleteFace(index)
                        }
                    }
                }
                
                // Message if no faces
                Label {
                    visible: !appController || !appController.facesList || appController.facesList.length === 0
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
                
                onClicked: mainWindow.navigateToEnroll()
            }
            
            Button {
                text: I18n.tr("manageFaces.backBtn")
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.2
                onClicked: mainWindow.navigateToHome()
            }
        }
    }
}
