import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import org.kde.kirigami 2.13 as Kirigami

Kirigami.Page {
    id: enrollPage
    title: i18n.tr("enrollment.title")
    
    Connections {
        target: mainWindow
        function onLanguageChanged() { 
            enrollPage.title = i18n.tr("enrollment.title")
        }
    }
    
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing * 1.5
        spacing: Kirigami.Units.largeSpacing
        
        // Title
        Label {
            text: i18n.tr("enrollment.registerNew")
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
                    text: i18n.tr("enrollment.cameraPreview")
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
                text: i18n.tr("enrollment.progress") + " " + appController.progress + "%"
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
            text: i18n.tr("enrollment.instructions")
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
                text: i18n.tr("enrollment.startBtn")
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
                text: i18n.tr("enrollment.stopBtn")
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.2
                enabled: mainWindow.appController.capturing
                onClicked: {
                    mainWindow.stopCapture()
                }
            }
            
            Button {
                text: i18n.tr("enrollment.cancelBtn")
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
            progressLabel.text = i18n.tr("enrollment.progress") + " " + value + "%"
        }
        
        function onCaptureCompletedSignal() {
            progressBar.value = 1.0
            progressLabel.text = i18n.tr("enrollment.progress") + " 100%"
        }
    }
}
