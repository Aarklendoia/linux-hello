import QtQuick
import QtQuick.Window
import org.kde.kirigami as Kirigami
import Linux.Hello 1.0

Kirigami.ApplicationWindow {
    id: mainWindow
    title: qsTr("Linux Hello - Configuration")
    width: 800
    height: 700
    visible: true

    // Signal pour les changements de langue
    signal languageChanged

    // Thème Breeze automatique via Kirigami
    palette.buttonText: Kirigami.Theme.textColor
    color: Kirigami.Theme.backgroundColor

    // Stack de pages pour navigation
    pageStack {
        initialPage: homeComponent
        // Forcer le mode une seule colonne en Kirigami 6
        columnView.columnWidth: mainWindow.width
    }

    // Désactiver les ToolTips complexes de Kirigami pour éviter les binding loops
    property bool showToolTips: false

    Timer {
        id: progressTimer
        interval: 500
        onTriggered: AppController.animateProgress()
    }

    Connections {
        target: AppController

        function onRestartTimerNeeded() {
            progressTimer.restart();
        }

        function onNavigateToHomeSignal() {
            mainWindow.pageStack.clear();
            mainWindow.pageStack.push(homeComponent);
        }

        function onNavigateToEnrollSignal() {
            mainWindow.pageStack.replace(Qt.resolvedUrl("Enrollment.qml"));
        }

        function onNavigateToSettingsSignal() {
            mainWindow.pageStack.replace(Qt.resolvedUrl("Settings.qml"));
        }

        function onNavigateToManageFacesSignal() {
            mainWindow.pageStack.replace(Qt.resolvedUrl("ManageFaces.qml"));
        }
    }

    // Page d'accueil (seule page pré-créée, pas de ProgressBar)
    Component {
        id: homeComponent
        Home {}
    }
}
