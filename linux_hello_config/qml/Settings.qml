import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Linux.Hello 1.0

Kirigami.Page {
    id: settingsPage
    title: I18n.tr("settings.title")
    
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
            text: I18n.tr("settings.configuration")
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
                        text: I18n.tr("settings.authentication")
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
                            text: I18n.tr("settings.minConfidence")
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
                            text: I18n.tr("settings.timeout")
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
                        text: I18n.tr("settings.camera")
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
                            text: I18n.tr("settings.resolution")
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
                            text: I18n.tr("settings.fps")
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
                        text: I18n.tr("settings.system")
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
                            text: I18n.tr("settings.pamIntegrated")
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
                            text: I18n.tr("settings.dbusActive")
                            color: Kirigami.Theme.textColor
                            Layout.fillWidth: true
                        }
                        
                        Label {
                            text: "✓"
                            color: Kirigami.Theme.positiveTextColor
                            font.pixelSize: 16
                        }
                    }
                    
                    RowLayout {
                        spacing: Kirigami.Units.largeSpacing
                        Layout.fillWidth: true
                        Layout.leftMargin: Kirigami.Units.largeSpacing
                        Layout.rightMargin: Kirigami.Units.largeSpacing
                        
                        Label {
                            text: I18n.tr("settings.language")
                            color: Kirigami.Theme.textColor
                            Layout.fillWidth: true
                        }
                        
                        ComboBox {
                            model: i18n.languages
                            
                            // Create display text with language names
                            delegate: ItemDelegate {
                                width: parent.width
                                text: i18n.languageNames[modelData]
                                highlighted: ListView.isCurrentItem
                            }
                            
                            currentIndex: i18n.languages.indexOf(i18n.currentLanguage)
                            
                            contentItem: Text {
                                text: i18n.languageNames[i18n.languages[currentIndex]]
                                color: Kirigami.Theme.textColor
                                verticalAlignment: Text.AlignVCenter
                                horizontalAlignment: Text.AlignLeft
                            }
                            
                            onCurrentIndexChanged: {
                                if (currentIndex >= 0 && currentIndex < i18n.languages.length) {
                                    i18n.loadLanguage(i18n.languages[currentIndex])
                                }
                            }
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
                text: I18n.tr("settings.saveBtn")
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.2
                
                palette.buttonText: Kirigami.Theme.highlightedTextColor
                palette.button: Kirigami.Theme.highlightColor
                
                onClicked: {
                    mainWindow.saveSettings()
                }
            }
            
            Button {
                text: I18n.tr("settings.backBtn")
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.2
                onClicked: {
                    mainWindow.navigateToHome()
                }
            }
        }
    }
}
