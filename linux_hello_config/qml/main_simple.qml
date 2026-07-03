import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window

import Linux.Hello 1.0
ApplicationWindow {
    id: mainWindow
    title: "Linux Hello - Configuration"
    width: 800
    height: 600
    visible: true
    
    color: "#f5f5f5"
    
    // Global app properties
    QtObject {
        id: appController

        // State
        property bool capturing: false
        property int progress: 0
        property var facesList: [
            { name: "Visage 1", confidence: 92, date: "2026-01-08" },
            { name: "Visage 2", confidence: 88, date: "2026-01-07" }
        ]

        // Methods
        function startCapture() {
            capturing = true
            progress = 0
            animateProgress()
        }
        
        function stopCapture() {
            capturing = false
        }
        
        function saveSettings() {
            console.log("Settings saved")
        }
        
        function deleteFace(index) {
            facesList.splice(index, 1)
            facesList = facesList
        }
        
        function animateProgress() {
            if (capturing && progress < 100) {
                progress += Math.random() * 15
                if (progress > 100) progress = 100
                if (progress >= 100) {
                    capturing = false
                } else {
                    progressTimer.restart()
                }
            }
        }
    }
    
    Timer {
        id: progressTimer
        interval: 500
        onTriggered: appController.animateProgress()
    }
    
    // Stack view for navigation
    StackView {
        id: stack
        anchors.fill: parent
        initialItem: homeComponent
    }
    
    // Home page
    Component {
        id: homeComponent
        Rectangle {
            color: "#f5f5f5"
            
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 40
                spacing: 20
                
                Label {
                    text: "Linux Hello"
                    font.pixelSize: 32
                    font.weight: Font.Bold
                    color: "#333333"
                    Layout.alignment: Qt.AlignHCenter
                }
                
                Label {
                    text: "Configuration d'authentification biométrique"
                    font.pixelSize: 16
                    color: "#666666"
                    Layout.alignment: Qt.AlignHCenter
                }
                
                Item { Layout.fillHeight: true }
                
                ColumnLayout {
                    spacing: 10
                    Layout.alignment: Qt.AlignHCenter
                    Layout.maximumWidth: 500
                    
                    Label {
                        text: "Bienvenue dans Linux Hello, le système d'authentification biométrique pour KDE."
                        wrapMode: Text.WordWrap
                        color: "#333333"
                        Layout.fillWidth: true
                    }
                    
                    Button {
                        text: "📷 Enregistrer un Visage"
                        Layout.fillWidth: true
                        implicitHeight: 50
                        onClicked: stack.push(enrollComponent)
                    }
                    
                    Button {
                        text: "👤 Gérer les Visages"
                        Layout.fillWidth: true
                        implicitHeight: 50
                        onClicked: stack.push(manageFacesComponent)
                    }
                    
                    Button {
                        text: "⚙️ Paramètres"
                        Layout.fillWidth: true
                        implicitHeight: 50
                        onClicked: stack.push(settingsComponent)
                    }
                }
                
                Item { Layout.fillHeight: true }
            }
        }
    }
    
    // Enrollment page
    Component {
        id: enrollComponent
        Rectangle {
            color: "#f5f5f5"
            
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 40
                spacing: 20
                
                Button {
                    text: "← Retour"
                    onClicked: stack.pop()
                }
                
                Label {
                    text: "Enregistrement"
                    font.pixelSize: 20
                    font.weight: Font.Bold
                    color: "#333333"
                }
                
                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 300
                    color: "#e0e0e0"
                    border.color: "#999999"
                    border.width: 1
                    
                    Label {
                        anchors.centerIn: parent
                        text: "Aperçu caméra"
                        color: "#666666"
                    }
                }
                
                Label {
                    text: "Progression : " + appController.progress + "%"
                    color: "#333333"
                }
                
                ProgressBar {
                    value: appController.progress / 100.0
                    Layout.fillWidth: true
                }
                
                Item { Layout.fillHeight: true }
                
                RowLayout {
                    spacing: 10
                    Layout.fillWidth: true
                    
                    Button {
                        text: "Démarrer"
                        Layout.fillWidth: true
                        enabled: !appController.capturing
                        onClicked: appController.startCapture()
                    }
                    
                    Button {
                        text: "Arrêter"
                        Layout.fillWidth: true
                        enabled: appController.capturing
                        onClicked: appController.stopCapture()
                    }
                }
            }
        }
    }
    
    // Settings page
    Component {
        id: settingsComponent
        Rectangle {
            color: "#f5f5f5"
            
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 40
                spacing: 20
                
                Button {
                    text: "← Retour"
                    onClicked: stack.pop()
                }
                
                Label {
                    text: "Paramètres"
                    font.pixelSize: 20
                    font.weight: Font.Bold
                    color: "#333333"
                }
                
                ScrollView {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    
                    ColumnLayout {
                        width: parent.width
                        spacing: 20
                        
                        ColumnLayout {
                            spacing: 10
                            Layout.fillWidth: true
                            
                            Label {
                                text: "Authentification"
                                font.weight: Font.Bold
                                color: "#333333"
                            }
                            
                            RowLayout {
                                Layout.fillWidth: true
                                Layout.leftMargin: 20
                                
                                Label {
                                    text: "Confiance minimale :"
                                    Layout.fillWidth: true
                                }
                                
                                SpinBox {
                                    from: 0
                                    to: 100
                                    value: 85
                                }
                            }
                        }
                        
                        Item { Layout.fillHeight: true }
                    }
                }
                
                Button {
                    text: "Enregistrer"
                    Layout.fillWidth: true
                    onClicked: appController.saveSettings()
                }
            }
        }
    }
    
    // Face management page
    Component {
        id: manageFacesComponent
        Rectangle {
            color: "#f5f5f5"
            
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 40
                spacing: 20
                
                Button {
                    text: "← Retour"
                    onClicked: stack.pop()
                }
                
                Label {
                    text: "Gérer les Visages"
                    font.pixelSize: 20
                    font.weight: Font.Bold
                    color: "#333333"
                }
                
                ScrollView {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    
                    ColumnLayout {
                        width: parent.width
                        spacing: 10
                        
                        Repeater {
                            model: appController.facesList
                            
                            Rectangle {
                                color: "#ffffff"
                                border.color: "#e0e0e0"
                                border.width: 1
                                radius: 4
                                Layout.fillWidth: true
                                implicitHeight: 80
                                
                                RowLayout {
                                    anchors.fill: parent
                                    anchors.margins: 10
                                    spacing: 10
                                    
                                    Label {
                                        text: "👤"
                                        font.pixelSize: 24
                                    }
                                    
                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: 5
                                        
                                        Label {
                                            text: modelData.name || "Visage " + (index + 1)
                                            font.weight: Font.Bold
                                            color: "#333333"
                                        }
                                        
                                        Label {
                                            text: "Confiance: " + modelData.confidence + "%"
                                            font.pixelSize: 12
                                            color: "#666666"
                                        }
                                    }
                                    
                                    Button {
                                        text: "🗑️ Supprimer"
                                        onClicked: appController.deleteFace(index)
                                    }
                                }
                            }
                        }
                        
                        Item { Layout.fillHeight: true }
                    }
                }
                
                Button {
                    text: "+ Enregistrer un nouveau visage"
                    Layout.fillWidth: true
                    onClicked: stack.push(enrollComponent)
                }
            }
        }
    }
}
