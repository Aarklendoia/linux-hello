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

    // Propriétés globales de l'app
    QtObject {
        id: appController

        // État de l'application
        property bool capturing: false
        property int progress: 0
        property var facesList: [
            {
                name: "Visage 1",
                confidence: 92,
                date: "2026-01-08"
            },
            {
                name: "Visage 2",
                confidence: 88,
                date: "2026-01-07"
            }
        ]

        // Signaux
        signal appProgressChanged(int value)
        signal captureCompletedSignal
        signal captureErrorSignal(string message)
        signal navigateToHomeSignal
        signal navigateToEnrollSignal
        signal navigateToSettingsSignal
        signal navigateToManageFacesSignal

        // Méthodes
        property string ctrlPort: "0"
        property string lastRegisteredFaceId: ""

        Component.onCompleted: {
            // Qt.environmentVariable n'est pas disponible sur ce build — lire depuis fichier
            var xhr = new XMLHttpRequest();
            xhr.open("GET", "file:///tmp/linux-hello-ctrl.port", false);
            xhr.send();
            if (xhr.responseText !== "") {
                ctrlPort = xhr.responseText.trim();
                console.log("🔌 ctrlPort lu:", ctrlPort);
            } else {
                console.log("⚠ Impossible de lire le port de contrôle");
            }
        }

        function startCapture() {
            capturing = true;
            progress = 0;
            console.log("🔌 Serveur de contrôle port:", ctrlPort);
            // Appeler le daemon via le serveur de contrôle local
            var xhr = new XMLHttpRequest();
            xhr.open("GET", "http://127.0.0.1:" + ctrlPort + "/start-capture", true);
            xhr.onreadystatechange = function () {
                if (xhr.readyState === XMLHttpRequest.DONE)
                    console.log("✓ start-capture réponse HTTP", xhr.status, ":", xhr.responseText);
            };
            xhr.send();
            animateProgress();
        }

        function registerFace() {
            console.log("📸 Envoi /register-face vers port:", ctrlPort);
            var xhr = new XMLHttpRequest();
            xhr.open("GET", "http://127.0.0.1:" + ctrlPort + "/register-face", true);
            xhr.onreadystatechange = function () {
                if (xhr.readyState !== XMLHttpRequest.DONE)
                    return;
                capturing = false;
                if (xhr.status === 200) {
                    try {
                        var resp = JSON.parse(xhr.responseText);
                        if (resp.ok) {
                            lastRegisteredFaceId = resp.face_id || "";
                            console.log("✓ Visage enregistré, face_id:", lastRegisteredFaceId);
                            captureCompletedSignal();
                        } else {
                            console.log("✗ Erreur enregistrement:", resp.error);
                            captureErrorSignal(resp.error || "Erreur inconnue");
                        }
                    } catch (e) {
                        console.log("✗ Réponse invalide:", xhr.responseText);
                        captureErrorSignal("Réponse invalide du serveur");
                    }
                } else {
                    console.log("✗ Erreur HTTP:", xhr.status, xhr.responseText);
                    captureErrorSignal("Erreur HTTP " + xhr.status);
                }
            };
            xhr.send();
        }

        function stopCapture() {
            capturing = false;
            var xhr = new XMLHttpRequest();
            xhr.open("GET", "http://127.0.0.1:" + ctrlPort + "/stop-capture", true);
            xhr.send();
        }

        function saveSettings() {
            console.log("Paramètres sauvegardés");
        }

        function deleteFace(index) {
            facesList.splice(index, 1);
            facesList = facesList;  // Trigger update
        }

        function navigateToHomeImpl() {
            mainWindow.pageStack.clear();
            mainWindow.pageStack.push(homeComponent);
        }

        function navigateToEnrollImpl() {
            mainWindow.pageStack.replace(enrollComponent);
        }

        function navigateToSettingsImpl() {
            mainWindow.pageStack.replace(settingsComponent);
        }

        function navigateToManageFacesImpl() {
            mainWindow.pageStack.replace(manageFacesComponent);
        }

        function animateProgress() {
            if (capturing && progress < 100) {
                progress += Math.random() * 15;
                if (progress > 100)
                    progress = 100;
                appProgressChanged(progress);
                if (progress >= 100) {
                    // Capture preview terminée — enregistrer le visage pour de vrai
                    registerFace();
                } else {
                    progressTimer.restart();
                }
            }
        }
    }

    Timer {
        id: progressTimer
        interval: 500
        onTriggered: appController.animateProgress()
    }

    // Raccourcis globaux
    function startCapture() {
        appController.startCapture();
    }
    function stopCapture() {
        appController.stopCapture();
    }
    function saveSettings() {
        appController.saveSettings();
    }
    function deleteFace(index) {
        appController.deleteFace(index);
    }
    function navigateToHome() {
        appController.navigateToHomeImpl();
    }
    function navigateToEnroll() {
        appController.navigateToEnrollImpl();
    }
    function navigateToSettings() {
        appController.navigateToSettingsImpl();
    }
    function navigateToManageFaces() {
        appController.navigateToManageFacesImpl();
    }

    // Page d'accueil
    Component {
        id: homeComponent
        Home {}
    }

    // Page d'enregistrement
    Component {
        id: enrollComponent
        Enrollment {}
    }

    // Page de paramètres
    Component {
        id: settingsComponent
        Settings {}
    }

    // Page de gestion des visages
    Component {
        id: manageFacesComponent
        ManageFaces {}
    }
}
