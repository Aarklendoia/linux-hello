import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.Page {
    id: enrollPage
    title: I18n.tr("enrollment.title")
    
    // Propriétés pour pageStack
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
            text: I18n.tr("enrollment.registerNew")
            font.pixelSize: 20
            font.weight: Font.Bold
            color: Kirigami.Theme.textColor
        }
        
        // Camera preview area - reads JPEG frames from /tmp/linux-hello-preview.jpg
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 400
            color: "#000000"
            border.color: Kirigami.Theme.textColor
            border.width: 2
            radius: Kirigami.Units.smallSpacing
            
            Image {
                id: cameraPreview
                anchors.fill: parent
                anchors.margins: 0
                source: ""
                fillMode: Image.PreserveAspectFit
                asynchronous: true
                cache: false  // Important: evite que le cache masque les MAJ
                
                // Affiche un placeholder si pas d'image
                Rectangle {
                    anchors.fill: parent
                    color: "#1a1a1a"
                    visible: cameraPreview.status !== Image.Ready
                    
                    Column {
                        anchors.centerIn: parent
                        spacing: Kirigami.Units.largeSpacing
                        width: parent.width * 0.8
                        
                        Text {
                            text: "📹"
                            font.pixelSize: 64
                            anchors.horizontalCenter: parent.horizontalCenter
                        }
                        
                        Label {
                            text: appController.capturing ? 
                                  I18n.tr("enrollment.capturingVideo") : 
                                  I18n.tr("enrollment.cameraPreview")
                            color: Kirigami.Theme.disabledTextColor
                            anchors.horizontalCenter: parent.horizontalCenter
                            wrapMode: Text.WordWrap
                            width: parent.width
                            horizontalAlignment: Text.AlignHCenter
                        }
                    }
                }
            }
        }
        
        // Progress bar
        ColumnLayout {
            spacing: Kirigami.Units.smallSpacing * 1.5
            Layout.fillWidth: true
            
            Label {
                text: I18n.tr("enrollment.progress") + " " + appController.progress + "%"
                color: Kirigami.Theme.textColor
                id: progressLabel
            }
            
            ProgressBar {
                value: 0
                Layout.fillWidth: true
                id: progressBar
            }
        }
        
        // Instructions
        Label {
            text: I18n.tr("enrollment.instructions")
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.disabledTextColor
            Layout.fillWidth: true
        }
        
        // Flexible spacing
        Item { Layout.fillHeight: true }
        
        // Action buttons
        RowLayout {
            spacing: Kirigami.Units.mediumSpacing * 1.5
            Layout.fillWidth: true
            
            Button {
                text: I18n.tr("enrollment.startBtn")
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.2
                enabled: !appController.capturing
                
                palette.buttonText: Kirigami.Theme.highlightedTextColor
                palette.button: Kirigami.Theme.highlightColor
                
                onClicked: {
                    mainWindow.startCapture()
                    frameRefreshTimer.start()
                }
            }
            
            Button {
                text: I18n.tr("enrollment.stopBtn")
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.2
                enabled: appController.capturing
                onClicked: {
                    mainWindow.stopCapture()
                    frameRefreshTimer.stop()
                }
            }
            
            Button {
                text: I18n.tr("enrollment.cancelBtn")
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.2
                onClicked: {
                    frameRefreshTimer.stop()
                    mainWindow.navigateToHome()
                }
            }
        }
    }
    
    // Timer pour rafraîchir la preview vidéo
    Timer {
        id: frameRefreshTimer
        interval: 33  // ~30 fps
        running: false
        repeat: true
        
        onTriggered: {
            // Force le rechargement de l'image depuis le disque
            // Le daemon exporte /tmp/linux-hello-preview.jpg
            var timestamp = new Date().getTime()
            cameraPreview.source = "file:///tmp/linux-hello-preview.jpg?" + timestamp
        }
    }
    
    // Connections to app signals
    Connections {
        target: appController
        
        function onAppProgressChanged(value) {
            progressBar.value = value / 100.0
            progressLabel.text = I18n.tr("enrollment.progress") + " " + value + "%"
        }
        
        function onCaptureCompletedSignal() {
            progressBar.value = 1.0
            progressLabel.text = I18n.tr("enrollment.progress") + " 100%"
            frameRefreshTimer.stop()
        }
    }
}
            }
            
            Button {
                text: I18n.tr("enrollment.stopBtn")
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.2
                enabled: appController.capturing
                onClicked: mainWindow.stopCapture()
            }
            
            Button {
                text: I18n.tr("enrollment.cancelBtn")
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.2
                onClicked: mainWindow.navigateToHome()
            }
        }
    }
    
    // Connections to app signals
    Connections {
        target: appController
        
        function onAppProgressChanged(value) {
            progressBar.value = value / 100.0
            progressLabel.text = I18n.tr("enrollment.progress") + " " + value + "%"
        }
        
        function onCaptureCompletedSignal() {
            progressBar.value = 1.0
            progressLabel.text = I18n.tr("enrollment.progress") + " 100%"
        }
    }
}
