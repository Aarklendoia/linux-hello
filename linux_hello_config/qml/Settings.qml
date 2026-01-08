import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import org.kde.kirigami 2.13 as Kirigami

Kirigami.Page {
    id: settingsPage
    title: "Settings"
    
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing * 1.5
        spacing: Kirigami.Units.largeSpacing
        
        // Title
        Label {
            text: "Configuration"
            font.pixelSize: 20
            font.weight: Font.Bold
            color: Kirigami.Theme.textColor
        }
        
        // Scrollable area
        ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            
            ColumnLayout {
                spacing: Kirigami.Units.largeSpacing * 1.5
                width: settingsPage.width - 4 * Kirigami.Units.largeSpacing
                
                // Authentication Section
                ColumnLayout {
                    spacing: Kirigami.Units.mediumSpacing * 1.5
                    Layout.fillWidth: true
                    Layout.topMargin: Kirigami.Units.mediumSpacing
                    
                    Label {
                        text: "Authentication"
                        font.weight: Font.Bold
                        font.pixelSize: 14
                        color: Kirigami.Theme.textColor
                    }
                    
                    RowLayout {
                        spacing: Kirigami.Units.largeSpacing
                        Layout.fillWidth: true
                        Layout.leftMargin: Kirigami.Units.largeSpacing
                        Layout.rightMargin: Kirigami.Units.largeSpacing
                        
                        Label {
                            text: "Minimum Confidence:"
                            color: Kirigami.Theme.textColor
                            Layout.fillWidth: true
                        }
                        
                        SpinBox {
                            from: 0
                            to: 100
                            value: 85
                            editable: true
                        }
                    }
                    
                    RowLayout {
                        spacing: Kirigami.Units.largeSpacing
                        Layout.fillWidth: true
                        Layout.leftMargin: Kirigami.Units.largeSpacing
                        Layout.rightMargin: Kirigami.Units.largeSpacing
                        
                        Label {
                            text: "Timeout (seconds):"
                            color: Kirigami.Theme.textColor
                            Layout.fillWidth: true
                        }
                        
                        SpinBox {
                            from: 1
                            to: 60
                            value: 30
                            editable: true
                        }
                    }
                }
                
                Rectangle {
                    Layout.fillWidth: true
                    height: 1
                    color: Kirigami.Theme.backgroundColor
                }
                
                // Camera Section
                ColumnLayout {
                    spacing: Kirigami.Units.mediumSpacing * 1.5
                    Layout.fillWidth: true
                    Layout.topMargin: Kirigami.Units.mediumSpacing
                    
                    Label {
                        text: "Camera"
                        font.weight: Font.Bold
                        font.pixelSize: 14
                        color: Kirigami.Theme.textColor
                    }
                    
                    RowLayout {
                        spacing: Kirigami.Units.largeSpacing
                        Layout.fillWidth: true
                        Layout.leftMargin: Kirigami.Units.largeSpacing
                        Layout.rightMargin: Kirigami.Units.largeSpacing
                        
                        Label {
                            text: "Resolution:"
                            color: Kirigami.Theme.textColor
                            Layout.fillWidth: true
                        }
                        
                        ComboBox {
                            model: ["1280x720", "1920x1080", "640x480"]
                            currentIndex: 0
                        }
                    }
                    
                    RowLayout {
                        spacing: Kirigami.Units.largeSpacing
                        Layout.fillWidth: true
                        Layout.leftMargin: Kirigami.Units.largeSpacing
                        Layout.rightMargin: Kirigami.Units.largeSpacing
                        
                        Label {
                            text: "FPS:"
                            color: Kirigami.Theme.textColor
                            Layout.fillWidth: true
                        }
                        
                        SpinBox {
                            from: 15
                            to: 60
                            value: 30
                            editable: true
                        }
                    }
                }
                
                Rectangle {
                    Layout.fillWidth: true
                    height: 1
                    color: Kirigami.Theme.backgroundColor
                }
                
                // System Section
                ColumnLayout {
                    spacing: Kirigami.Units.mediumSpacing * 1.5
                    Layout.fillWidth: true
                    Layout.topMargin: Kirigami.Units.mediumSpacing
                    
                    Label {
                        text: "System"
                        font.weight: Font.Bold
                        font.pixelSize: 14
                        color: Kirigami.Theme.textColor
                    }
                    
                    RowLayout {
                        spacing: Kirigami.Units.largeSpacing
                        Layout.fillWidth: true
                        Layout.leftMargin: Kirigami.Units.largeSpacing
                        Layout.rightMargin: Kirigami.Units.largeSpacing
                        
                        Label {
                            text: "PAM Integrated:"
                            color: Kirigami.Theme.textColor
                            Layout.fillWidth: true
                        }
                        
                        CheckBox {
                            checked: true
                        }
                    }
                    
                    RowLayout {
                        spacing: Kirigami.Units.largeSpacing
                        Layout.fillWidth: true
                        Layout.leftMargin: Kirigami.Units.largeSpacing
                        Layout.rightMargin: Kirigami.Units.largeSpacing
                        
                        Label {
                            text: "DBus Active:"
                            color: Kirigami.Theme.textColor
                            Layout.fillWidth: true
                        }
                        
                        Label {
                            text: "✓"
                            color: Kirigami.Theme.positiveTextColor
                            font.pixelSize: 16
                        }
                    }
                }
                
                Item { Layout.fillHeight: true }
            }
        }
        
        // Action buttons
        RowLayout {
            spacing: Kirigami.Units.largeSpacing
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.mediumSpacing
            Layout.rightMargin: Kirigami.Units.mediumSpacing
            
            Button {
                text: "Save"
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.2
                
                palette.buttonText: Kirigami.Theme.highlightedTextColor
                palette.button: Kirigami.Theme.highlightColor
                
                onClicked: {
                    mainWindow.saveSettings()
                }
            }
            
            Button {
                text: "Back"
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.2
                onClicked: {
                    mainWindow.navigateToHome()
                }
            }
        }
    }
}
