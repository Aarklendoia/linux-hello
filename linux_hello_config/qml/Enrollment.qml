import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtMultimedia
import org.kde.kirigami as Kirigami

Kirigami.Page {
    id: enrollPage
    title: I18n.tr("enrollment.title")

    property var appController: null

    // Propriétés pour pageStack
    Layout.fillWidth: true
    Layout.fillHeight: true

    // Au chargement de la page, connecter le lecteur MJPEG
    Component.onCompleted: {
        console.log("📄 Page Enrollment chargée");
        mjpegPlayer.play();
        console.log("✓ Lecteur MJPEG démarré");
    }

    Component.onDestruction: {
        mjpegPlayer.stop();
    }
    // Lecteur MJPEG — se connecte au serveur du daemon sur 127.0.0.1:17823
    MediaPlayer {
        id: mjpegPlayer
        videoOutput: cameraPreview
        source: "http://127.0.0.1:17823"
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

            VideoOutput {
                id: cameraPreview
                anchors.fill: parent
                anchors.margins: Kirigami.Units.smallSpacing
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
                text: I18n.tr("enrollment.progress") + " " + enrollPage.appController.progress + "%"
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
                enabled: !enrollPage.appController.capturing

                palette.buttonText: Kirigami.Theme.highlightedTextColor
                palette.button: Kirigami.Theme.highlightColor

                onClicked: {
                    console.log("🎬 Bouton Démarrer cliqué");
                    enrollPage.appController.startCapture();
                }
            }

            Button {
                text: I18n.tr("enrollment.stopBtn")
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.2
                enabled: enrollPage.appController.capturing
                onClicked: {
                    enrollPage.appController.stopCapture();
                    previewRect.previewStatus = qsTr("Capture arrêtée");
                }
            }

            Button {
                text: I18n.tr("enrollment.cancelBtn")
                Layout.fillWidth: true
                implicitHeight: Kirigami.Units.gridUnit * 2.2
                onClicked: {
                    enrollPage.appController.navigateToHomeImpl();
                    previewRect.previewStatus = qsTr("Capture annulée");
                }
            }
        }
    }

    // Retour automatique à l'accueil 2 secondes après succès
    Timer {
        id: navigateHomeTimer
        interval: 2000
        repeat: false
        onTriggered: enrollPage.appController.navigateToHomeImpl()
    }

    // Connections to app signals
    Connections {
        target: enrollPage.appController

        function onAppProgressChanged(value) {
            progressBar.value = value / 100.0;
            progressLabel.text = I18n.tr("enrollment.progress") + " " + Math.round(value) + "%";
            if (value >= 100) {
                previewRect.previewStatus = qsTr("Analyse du visage en cours…");
                progressLabel.text = qsTr("Validation en cours…");
            }
        }

        function onCaptureCompletedSignal() {
            progressBar.value = 1.0;
            progressLabel.text = I18n.tr("enrollment.progress") + " 100%";
            previewRect.previewStatus = qsTr("✓ Visage enregistré avec succès !");
            navigateHomeTimer.start();
        }

        function onCaptureErrorSignal(message) {
            previewRect.previewStatus = qsTr("✗ Erreur : ") + message;
        }
    }
}
