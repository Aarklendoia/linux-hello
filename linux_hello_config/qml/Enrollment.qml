import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import org.kde.kirigami 2.13 as Kirigami

Kirigami.Page {
    id: enrollPage
    title: "Face Registration"
    
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing * 1.5
        spacing: Kirigami.Units.largeSpacing
        
        // Title
        Label {
            text: "Register a New Face"
            font.pixelSize: 20
            font.weight: Font.Bold
            color: Kirigami.Theme.textColor
        }
        
        // Camera preview area
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 300
            color: Kirigami.Theme.backgroundColor
            border.color: Kirigami.Theme.textColor
            border.width: 1
            
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: Kirigami.Units.mediumSpacing
                spacing: Kirigami.Units.mediumSpacing
                
                Label {
                    text: "Camera Preview"
                    color: Kirigami.Theme.disabledTextColor
                    Layout.alignment: Qt.AlignHCenter
                    Layout.fillHeight: true
                    Layout.fillWidth: true
                    verticalAlignment: Text.AlignVCenter
                    horizontalAlignment: Text.AlignHCenter
                }
            }
        }
        
        // Progress bar
        ColumnLayout {
            spacing: Kirigami.Units.smallSpacing * 1.5
            Layout.fillWidth: true
            
            Label {
                text: "Progress: 0%"
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
            text: "Present your face to the camera. The system will capture multiple angles for better recognition."
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
                text: "Start Capture"
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.2
                enabled: !mainWindow.appController.capturing
                
                palette.buttonText: Kirigami.Theme.highlightedTextColor
                palette.button: Kirigami.Theme.highlightColor
                
                onClicked: {
                    mainWindow.startCapture()
                }
            }
            
            Button {
                text: "Stop"
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.2
                enabled: mainWindow.appController.capturing
                onClicked: {
                    mainWindow.stopCapture()
                }
            }
            
            Button {
                text: "Cancel"
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.2
                onClicked: {
                    mainWindow.navigateToHome()
                }
            }
        }
    }
    
    // Connections to app signals
    Connections {
        target: mainWindow
        
        function onAppProgressChanged(value) {
            progressBar.value = value / 100.0
            progressLabel.text = "Progress: " + value + "%"
        }
        
        function onCaptureCompletedSignal() {
            progressBar.value = 1.0
            progressLabel.text = "Progress: 100%"
        }
    }
}
