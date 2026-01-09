import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Linux.Hello 1.0

Kirigami.Page {
    id: homePage
    
    title: I18n.tr("home.title")
    
    // Propriétés pour pageStack
    Layout.fillWidth: true
    Layout.fillHeight: true
    
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.largeSpacing
        
        Item { Layout.preferredHeight: Kirigami.Units.largeSpacing }
        
        // Main title
        Label {
            text: "Linux Hello"
            font.pixelSize: 32
            font.weight: Font.Bold
            color: Kirigami.Theme.textColor
            Layout.alignment: Qt.AlignHCenter
        }
        
        // Subtitle
        Label {
            text: I18n.tr("app.subtitle")
            font.pixelSize: 16
            color: Kirigami.Theme.disabledTextColor
            Layout.alignment: Qt.AlignHCenter
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
        }
        
        // Spacing
        Item { Layout.fillHeight: true }
        
        // Main content
        ColumnLayout {
            spacing: Kirigami.Units.mediumSpacing * 1.5
            Layout.alignment: Qt.AlignHCenter
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.maximumWidth: 400
            
            Label {
                text: I18n.tr("home.welcome")
                wrapMode: Text.WordWrap
                color: Kirigami.Theme.textColor
                Layout.fillWidth: true
            }
            
            Label {
                text: I18n.tr("home.youCan")
                font.weight: Font.Bold
                color: Kirigami.Theme.textColor
                Layout.fillWidth: true
            }
            
            ColumnLayout {
                spacing: Kirigami.Units.smallSpacing * 2
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing * 1.5
                
                Label {
                    text: I18n.tr("home.action1")
                    color: Kirigami.Theme.textColor
                    wrapMode: Text.WordWrap
                }
                
                Label {
                    text: I18n.tr("home.action2")
                    color: Kirigami.Theme.textColor
                    wrapMode: Text.WordWrap
                }
                
                Label {
                    text: I18n.tr("home.action3")
                    color: Kirigami.Theme.textColor
                    wrapMode: Text.WordWrap
                }
            }
        }
        
        // Spacing
        Item { Layout.fillHeight: true }
        
        // Navigation buttons
        ColumnLayout {
            spacing: Kirigami.Units.mediumSpacing * 1.5
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignBottom
            
            Button {
                text: I18n.tr("home.registerBtn")
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.5
                onClicked: mainWindow.navigateToEnroll()
                
                palette.buttonText: Kirigami.Theme.highlightedTextColor
                palette.button: Kirigami.Theme.highlightColor
            }
            
            Button {
                text: I18n.tr("home.manageFacesBtn")
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.5
                onClicked: mainWindow.navigateToManageFaces()
            }
            
            Button {
                text: I18n.tr("home.settingsBtn")
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.5
                onClicked: mainWindow.navigateToSettings()
            }
        }
    }
}
