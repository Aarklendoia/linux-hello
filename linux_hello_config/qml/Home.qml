import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import org.kde.kirigami 2.13 as Kirigami

Kirigami.Page {
    id: homePage
    
    // Connexion au gestionnaire i18n
    Component.onCompleted: {
        mainWindow.languageChanged.connect(updateTexts)
    }
    
    Connections {
        target: mainWindow
        function onLanguageChanged() { updateTexts() }
    }
    
    function updateTexts() {
        title = i18n.tr("home.title")
    }
    
    title: i18n.tr("home.title")
    
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing * 1.5
        spacing: Kirigami.Units.largeSpacing
        
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
            text: i18n.tr("app.subtitle")
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
            Layout.maximumWidth: 400
            
            Label {
                text: i18n.tr("home.welcome")
                wrapMode: Text.WordWrap
                color: Kirigami.Theme.textColor
                Layout.fillWidth: true
            }
            
            Label {
                text: i18n.tr("home.youCan")
                font.weight: Font.Bold
                color: Kirigami.Theme.textColor
                Layout.fillWidth: true
            }
            
            ColumnLayout {
                spacing: Kirigami.Units.smallSpacing * 2
                Layout.leftMargin: Kirigami.Units.largeSpacing * 1.5
                
                Label {
                    text: i18n.tr("home.action1")
                    color: Kirigami.Theme.textColor
                    wrapMode: Text.WordWrap
                }
                
                Label {
                    text: i18n.tr("home.action2")
                    color: Kirigami.Theme.textColor
                    wrapMode: Text.WordWrap
                }
                
                Label {
                    text: i18n.tr("home.action3")
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
            
            Button {
                text: i18n.tr("home.registerBtn")
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.5
                onClicked: mainWindow.navigateToEnroll()
                
                palette.buttonText: Kirigami.Theme.highlightedTextColor
                palette.button: Kirigami.Theme.highlightColor
            }
            
            Button {
                text: i18n.tr("home.manageFacesBtn")
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.5
                onClicked: mainWindow.navigateToManageFaces()
            }
            
            Button {
                text: i18n.tr("home.settingsBtn")
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.5
                onClicked: mainWindow.navigateToSettings()
            }
        }
    }
}
