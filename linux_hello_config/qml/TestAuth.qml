import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtMultimedia
import org.kde.kirigami as Kirigami
import Linux.Hello 1.0

Kirigami.Page {
    id: testAuthPage
    title: I18n.tr("testAuth.title")

    Layout.fillWidth: true
    Layout.fillHeight: true

    Component.onCompleted: {
        mjpegPlayer.play();
    }

    Component.onDestruction: {
        mjpegPlayer.stop();
    }

    // Aperçu caméra en direct pendant le test
    MediaPlayer {
        id: mjpegPlayer
        videoOutput: cameraPreview
        source: "http://127.0.0.1:17823"
    }

    // États internes
    property bool testing: false
    property string resultState: "idle"  // "idle" | "success" | "nomatch" | "noface" | "error"
    property string resultMessage: ""
    property real resultScore: 0.0

    ColumnLayout {
        anchors {
            fill: parent
            margins: Kirigami.Units.largeSpacing
        }
        spacing: Kirigami.Units.largeSpacing

        // Titre
        Label {
            text: I18n.tr("testAuth.subtitle")
            font.pixelSize: 20
            font.weight: Font.Bold
            color: Kirigami.Theme.textColor
        }

        // Aperçu caméra
        Rectangle {
            id: previewRect
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.maximumHeight: 280
            Layout.minimumHeight: 160
            color: Kirigami.Theme.backgroundColor
            border.color: resultBorderColor
            border.width: 2
            radius: Kirigami.Units.smallSpacing

            property color resultBorderColor: {
                if (testAuthPage.resultState === "success")
                    return "#4CAF50";
                if (testAuthPage.resultState === "nomatch")
                    return "#F44336";
                if (testAuthPage.resultState === "noface")
                    return "#FF9800";
                return Kirigami.Theme.textColor;
            }

            VideoOutput {
                id: cameraPreview
                anchors.fill: parent
                anchors.margins: Kirigami.Units.smallSpacing
            }

            // Overlay résultat centré
            Rectangle {
                id: resultOverlay
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.bottom: parent.bottom
                anchors.bottomMargin: Kirigami.Units.mediumSpacing
                visible: testAuthPage.resultState !== "idle"
                color: resultOverlayColor
                radius: 6
                width: resultLabel.width + Kirigami.Units.largeSpacing * 2
                height: resultLabel.height + Kirigami.Units.mediumSpacing * 2

                property color resultOverlayColor: {
                    if (testAuthPage.resultState === "success")
                        return "#CC4CAF50";
                    if (testAuthPage.resultState === "nomatch")
                        return "#CCF44336";
                    return "#CC555555";
                }

                Label {
                    id: resultLabel
                    anchors.centerIn: parent
                    text: testAuthPage.resultMessage
                    color: "white"
                    font.pixelSize: 15
                    font.weight: Font.Medium
                }
            }

            // Spinner pendant le test
            BusyIndicator {
                anchors.centerIn: parent
                running: testAuthPage.testing
                visible: testAuthPage.testing
            }
        }

        // Sélecteur de contexte
        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.mediumSpacing

            Label {
                text: I18n.tr("testAuth.context")
                color: Kirigami.Theme.textColor
            }

            ComboBox {
                id: contextSelector
                Layout.fillWidth: true
                model: [
                    {
                        text: I18n.tr("testAuth.ctx.gui"),
                        value: "gui"
                    },
                    {
                        text: I18n.tr("testAuth.ctx.sudo"),
                        value: "sudo"
                    },
                    {
                        text: I18n.tr("testAuth.ctx.login"),
                        value: "login"
                    },
                    {
                        text: I18n.tr("testAuth.ctx.screenlock"),
                        value: "screenlock"
                    },
                    {
                        text: I18n.tr("testAuth.ctx.sddm"),
                        value: "sddm"
                    },
                ]
                textRole: "text"
                valueRole: "value"
                currentIndex: 0

                // Réinitialiser le résultat au changement de contexte
                onCurrentIndexChanged: {
                    testAuthPage.resultState = "idle";
                    testAuthPage.resultMessage = "";
                }
            }
        }

        // Score de similarité (affiché uniquement quand pertinent)
        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.mediumSpacing
            visible: testAuthPage.resultState === "success" || testAuthPage.resultState === "nomatch"

            Label {
                text: I18n.tr("testAuth.score")
                color: Kirigami.Theme.textColor
            }

            ProgressBar {
                id: scoreBar
                Layout.fillWidth: true
                from: 0.0
                to: 1.0
                value: testAuthPage.resultScore
            }

            Label {
                text: (testAuthPage.resultScore * 100).toFixed(1) + "%"
                color: Kirigami.Theme.textColor
                Layout.minimumWidth: Kirigami.Units.gridUnit * 3
                horizontalAlignment: Text.AlignRight
            }
        }

        // Bouton principal
        Button {
            id: testButton
            text: testAuthPage.testing ? I18n.tr("testAuth.testing") : I18n.tr("testAuth.startBtn")
            Layout.fillWidth: true
            implicitHeight: Kirigami.Units.gridUnit * 2.5
            enabled: !testAuthPage.testing
            palette.buttonText: Kirigami.Theme.highlightedTextColor
            palette.button: Kirigami.Theme.highlightColor

            onClicked: {
                testAuthPage.testing = true;
                testAuthPage.resultState = "idle";
                testAuthPage.resultMessage = "";
                testAuthPage.resultScore = 0.0;

                var ctx = contextSelector.model[contextSelector.currentIndex].value;
                AppController.testAuth(ctx, function (ok, data) {
                    testAuthPage.testing = false;
                    if (!ok) {
                        testAuthPage.resultState = "error";
                        testAuthPage.resultMessage = "⚠ " + (data || I18n.tr("testAuth.err.daemon"));
                        return;
                    }
                    // data est l'objet VerifyResult désérialisé
                    if (data.Success !== undefined) {
                        testAuthPage.resultState = "success";
                        testAuthPage.resultScore = data.Success.similarity_score || 0.0;
                        testAuthPage.resultMessage = "✓ " + I18n.tr("testAuth.result.success");
                    } else if (data.NoMatch !== undefined) {
                        testAuthPage.resultState = "nomatch";
                        testAuthPage.resultScore = data.NoMatch.best_score || 0.0;
                        testAuthPage.resultMessage = "✗ " + I18n.tr("testAuth.result.noMatch");
                    } else if (data.NoFaceDetected !== undefined) {
                        testAuthPage.resultState = "noface";
                        testAuthPage.resultScore = 0.0;
                        testAuthPage.resultMessage = "👤 " + I18n.tr("testAuth.result.noFace");
                    } else if (data.NoEnrollment !== undefined) {
                        testAuthPage.resultState = "error";
                        testAuthPage.resultMessage = "📋 " + I18n.tr("testAuth.result.noEnrollment");
                    } else if (data.Cancelled !== undefined) {
                        testAuthPage.resultState = "idle";
                        testAuthPage.resultMessage = I18n.tr("testAuth.result.cancelled");
                    } else {
                        testAuthPage.resultState = "error";
                        var msg = data.Error ? data.Error.message : I18n.tr("testAuth.err.unknown");
                        testAuthPage.resultMessage = "⚠ " + msg;
                    }
                });
            }
        }
    }
}
