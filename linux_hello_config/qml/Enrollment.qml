import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.Page {
    id: enrollPage
    title: I18n.tr("enrollment.title")

    // Propriétés pour pageStack
    Layout.fillWidth: true
    Layout.fillHeight: true

    // Au chargement de la page, commencer à afficher les images
    Component.onCompleted: {
        console.log("📄 Page Enrollment chargée");
        frameRefreshTimer.start();
        console.log("✓ frameRefreshTimer lancé automatiquement");
    }

    Component.onDestruction: {
        frameRefreshTimer.stop();
    }

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
            id: previewRect
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.maximumHeight: 320
            Layout.minimumHeight: 180
            color: Kirigami.Theme.backgroundColor
            border.color: Kirigami.Theme.textColor
            border.width: 2
            radius: Kirigami.Units.smallSpacing
            property string previewStatus: qsTr("Aperçu inactif")

            Image {
                id: cameraPreview
                anchors.fill: parent
                anchors.margins: Kirigami.Units.smallSpacing
                fillMode: Image.PreserveAspectCrop
                cache: false
                smooth: true
                source: ""
                onStatusChanged: {
                    if (status === Image.Ready) {
                        previewRect.previewStatus = qsTr("Aperçu à jour");
                    } else if (status === Image.Loading) {
                        previewRect.previewStatus = qsTr("Chargement en cours…");
                    }
                    if (status === Image.Error) {
                        previewRect.previewStatus = qsTr("Erreur : ") + cameraPreview.errorString;
                    }
                    console.log("🎬 cameraPreview.status =", status);
                }
            }

            Label {
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.bottom: parent.bottom
                anchors.bottomMargin: Kirigami.Units.mediumSpacing
                text: previewRect.previewStatus
                color: Kirigami.Theme.textColor
                font.pixelSize: 16
                background: Rectangle {
                    color: "#00000080"
                    radius: 4
                }
            }
        }

        // Progress bar
        ColumnLayout {
            spacing: Kirigami.Units.smallSpacing * 1.5
            Layout.fillWidth: true

            Label {
                id: progressLabel
                text: I18n.tr("enrollment.progress") + " " + appController.progress + "%"
                color: Kirigami.Theme.textColor
            }

            ProgressBar {
                id: progressBar
                value: 0
                Layout.fillWidth: true
            }
        }

        // Instructions
        Label {
            text: I18n.tr("enrollment.instructions")
            wrapMode: Text.WordWrap
            color: Kirigami.Theme.disabledTextColor
            Layout.fillWidth: true
        }

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

                onClicked: {
                    console.log("🎬 Bouton Démarrer cliqué");
                    mainWindow.startCapture();
                    console.log("✓ startCapture() appelé");
                    frameRefreshTimer.start();
                    console.log("✓ frameRefreshTimer démarré");
                    refreshPreview();
                }
            }

            Button {
                text: I18n.tr("enrollment.stopBtn")
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.2
                enabled: appController.capturing
                onClicked: {
                    mainWindow.stopCapture();
                    frameRefreshTimer.stop();
                    previewRect.previewStatus = qsTr("Capture arrêtée");
                }
            }

            Button {
                text: I18n.tr("enrollment.cancelBtn")
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.2
                onClicked: {
                    frameRefreshTimer.stop();
                    mainWindow.navigateToHome();
                    previewRect.previewStatus = qsTr("Capture annulée");
                }
            }
        }
    }

    function refreshPreview() {
        if (!appController.capturing) {
            previewRect.previewStatus = qsTr("Capture inactive — appuyez sur \"Démarrer\"");
            return;
        }

        var timestamp = new Date().getTime();
        var path = "file:///tmp/linux-hello-preview.jpg?" + timestamp;
        cameraPreview.source = path;
        previewRect.previewStatus = qsTr("Chargement de l'image…");
        console.log("🔄 Mise à jour de l'aperçu :", path);
    }

    // Timer pour actualiser l'image toutes les 500 ms
    Timer {
        id: frameRefreshTimer
        interval: 500
        running: true
        repeat: true

        onTriggered: refreshPreview()
    }

    // Retour automatique à l'accueil 2 secondes après succès
    Timer {
        id: navigateHomeTimer
        interval: 2000
        repeat: false
        onTriggered: mainWindow.navigateToHome()
    }

    // Connections to app signals
    Connections {
        target: appController

        function onAppProgressChanged(value) {
            progressBar.value = value / 100.0;
            progressLabel.text = I18n.tr("enrollment.progress") + " " + value + "%";

            // Lancer le Timer au premier changement de progression
            if (value > 0 && !frameRefreshTimer.running) {
                console.log("🎬 Progression détectée, lancement du Timer");
                frameRefreshTimer.start();
            }
        }

        function onCaptureCompletedSignal() {
            progressBar.value = 1.0;
            progressLabel.text = I18n.tr("enrollment.progress") + " 100%";
            frameRefreshTimer.stop();
            previewRect.previewStatus = qsTr("✓ Visage enregistré avec succès !");
            navigateHomeTimer.start();
        }

        function onCaptureErrorSignal(message) {
            frameRefreshTimer.stop();
            previewRect.previewStatus = qsTr("✗ Erreur : ") + message;
        }
    }
}
