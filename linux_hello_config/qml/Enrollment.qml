import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Linux.Hello 1.0

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
        
        // Camera preview area
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 300
            color: Kirigami.Theme.backgroundColor
            border.color: Kirigami.Theme.textColor
            border.width: 1
            
            Label {
                text: I18n.tr("enrollment.cameraPreview")
                color: Kirigami.Theme.disabledTextColor
                anchors.centerIn: parent
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
                
                onClicked: mainWindow.startCapture()
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
