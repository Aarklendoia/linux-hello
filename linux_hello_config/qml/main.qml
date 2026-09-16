import QtQuick
import QtQuick.Window
import org.kde.kirigami as Kirigami
import Linux.Hello 1.0

Kirigami.ApplicationWindow {
    id: mainWindow
    title: qsTr("Linux Hello - Configuration")
    width: 480
    // Was 640/560 — the Home screen grew a fourth action card ("Cache
    // session password") plus its info note, which no longer fit; 720/620
    // (a first attempt) turned out to overshoot, leaving visible empty
    // space below the last note on a real run. This value was confirmed
    // against that same real run — see Home.qml/CachePassword.qml's own
    // ScrollablePage comments for the belt-and-suspenders fallback if
    // content grows further (a longer translation, another card, …).
    height: 680
    minimumWidth: 420
    minimumHeight: 580
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

        // push (not replace): About/License are peek-and-return sub-pages,
        // so Home stays underneath and Kirigami's own back button/gesture
        // returns to it — unlike Enroll/ManageFaces above, which fully take
        // over the single-column stack.
        function onNavigateToAboutSignal() {
            mainWindow.pageStack.push(Qt.resolvedUrl("About.qml"));
        }

        function onNavigateToLicenseSignal() {
            mainWindow.pageStack.push(Qt.resolvedUrl("License.qml"));
        }

        // push (not replace): peek-and-return, same as About/License —
        // Home stays underneath so the back button/gesture returns to it.
        function onNavigateToCachePasswordSignal() {
            mainWindow.pageStack.push(Qt.resolvedUrl("CachePassword.qml"));
        }
    }

    // Home page (the only pre-created page, no ProgressBar)
    Component {
        id: homeComponent
        Home {}
    }
}
