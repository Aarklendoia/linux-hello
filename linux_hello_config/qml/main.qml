import QtQuick
import QtQuick.Window
import org.kde.kirigami as Kirigami
import Linux.Hello 1.0

Kirigami.ApplicationWindow {
    id: mainWindow
    title: qsTr("Linux Hello - Configuration")
    width: 480
    height: 640
    minimumWidth: 420
    minimumHeight: 560
    visible: true

    // Signal for language changes
    signal languageChanged

    // Automatic Breeze theme via Kirigami
    palette.buttonText: Kirigami.Theme.textColor
    color: Kirigami.Theme.backgroundColor

    // Page stack for navigation
    pageStack {
        initialPage: homeComponent
        // Force single-column mode in Kirigami 6
        columnView.columnWidth: mainWindow.width
    }

    // Disable Kirigami's complex ToolTips to avoid binding loops
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

        function onNavigateToManageFacesSignal() {
            mainWindow.pageStack.replace(Qt.resolvedUrl("ManageFaces.qml"));
        }
    }

    // Home page (the only pre-created page, no ProgressBar)
    Component {
        id: homeComponent
        Home {}
    }
}
