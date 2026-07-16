import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import Linux.Hello 1.0

Kirigami.Page {
    id: enrollPage
    title: I18n.tr("enrollment.title")

    // Properties for pageStack
    Layout.fillWidth: true
    Layout.fillHeight: true

    padding: Kirigami.Units.largeSpacing

    // The daemon only has a frame to serve while a capture-stream is
    // actually running (start-capture ... stop-capture); outside that
    // window /snapshot 503s. Polling unconditionally from page load meant
    // constant failed requests before the user even clicked "Démarrer",
    // which reads as the preview blinking. Tie the timer directly to
    // AppController.capturing instead, and clear the image the moment
    // capture stops so no broken/stale frame lingers on screen.
    Connections {
        target: AppController
        function onCapturingChanged() {
            if (!AppController.capturing) {
                cameraPreview.source = "";
            }
        }
    }

    // Polls a single JPEG per tick instead of playing the daemon's MJPEG feed:
    // QtMultimedia's MediaPlayer cannot demux a raw multipart/x-mixed-replace
    // stream. 150ms was arbitrary; the daemon's V4L2 capture actually runs at
    // ~30fps (~33ms/frame, confirmed from hello-daemon's own frame logs), so
    // polling at 150ms was silently dropping 4 out of every 5 real frames —
    // that's what read as choppy. Poll close to the real source cadence
    // instead; the broadcast channel's capacity is 1, so polling faster than
    // the source just re-fetches the same frame, never wasted beyond that.
    Timer {
        id: snapshotTimer
        interval: 40
        repeat: true
        running: AppController.capturing
        // token= gates every request to hello-daemon's MJPEG server (see
        // hello_daemon::preview::start_mjpeg_server) — without it any local
        // process regardless of user could otherwise watch live video of
        // whoever is enrolling. QML's Image { source } can't set custom
        // request headers, so unlike the GUI's own control-server token
        // (sent as a header), this one has to travel in the URL.
        onTriggered: cameraPreview.source = "http://127.0.0.1:17823/snapshot?token=" + AppController.mjpegToken + "&t=" + Date.now()
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: Kirigami.Units.largeSpacing

        Label {
            text: I18n.tr("enrollment.registerNew")
            font.pixelSize: 18
            font.weight: Font.Bold
            color: Kirigami.Theme.textColor
        }

        // Camera preview with viewfinder-style corner brackets
        Item {
            id: previewRect
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.maximumHeight: Kirigami.Units.gridUnit * 16
            Layout.minimumHeight: Kirigami.Units.gridUnit * 10

            property string previewStatus: I18n.tr("enrollment.previewInactive")

            Rectangle {
                anchors.fill: parent
                radius: Kirigami.Units.smallSpacing * 1.4
                // Derived from the real Plasma background instead of a fixed
                // hex: still reads as a dark viewfinder (camera previews are
                // conventionally dark regardless of app theme, same as the
                // approved mockup), but its hue now follows the actual
                // color scheme instead of being identical in every theme.
                color: Qt.darker(Kirigami.Theme.backgroundColor, 400)
                border.width: 1
                // Deliberately a plain white rim, not theme-derived: it's a
                // highlight edge on what's always a dark surface above, the
                // same in every Plasma theme (matches the mockup, which has
                // no light/dark variant for this either).
                border.color: Qt.rgba(1, 1, 1, 0.18)
                clip: true

                Image {
                    id: cameraPreview
                    anchors.fill: parent
                    anchors.margins: 1
                    fillMode: Image.PreserveAspectFit
                    cache: false
                    asynchronous: true
                }
            }

            // Corner brackets — accent-colored, "scanner viewfinder" cue
            Repeater {
                model: 4
                Item {
                    readonly property bool isTop: index < 2
                    readonly property bool isLeft: index % 2 === 0
                    anchors.top: isTop ? parent.top : undefined
                    anchors.bottom: !isTop ? parent.bottom : undefined
                    anchors.left: isLeft ? parent.left : undefined
                    anchors.right: !isLeft ? parent.right : undefined
                    anchors.margins: Kirigami.Units.smallSpacing
                    width: Kirigami.Units.gridUnit
                    height: Kirigami.Units.gridUnit

                    Canvas {
                        anchors.fill: parent
                        onPaint: {
                            var ctx = getContext("2d");
                            ctx.reset();
                            ctx.strokeStyle = Kirigami.Theme.highlightColor;
                            ctx.lineWidth = 2.5;
                            ctx.lineCap = "round";
                            ctx.beginPath();
                            var s = parent.isLeft ? 0 : width;
                            var d = parent.isLeft ? 1 : -1;
                            var ty = parent.isTop ? 0 : height;
                            var td = parent.isTop ? 1 : -1;
                            ctx.moveTo(s + d * width * 0.75, ty);
                            ctx.lineTo(s, ty);
                            ctx.lineTo(s, ty + td * height * 0.75);
                            ctx.stroke();
                        }
                    }
                }
            }

            // Live status chip — a translucent dark HUD bubble over the video
            // image itself, same convention as a camera app's on-screen text
            // (always dark-on-video for legibility over any footage);
            // deliberately not theme-derived, matching the approved mockup,
            // which also keeps this fixed across light/dark.
            Rectangle {
                visible: AppController.capturing
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.bottom: parent.bottom
                anchors.bottomMargin: Kirigami.Units.smallSpacing
                radius: Kirigami.Units.gridUnit
                color: "#b3000000"
                implicitWidth: statusRow.implicitWidth + Kirigami.Units.largeSpacing
                implicitHeight: statusRow.implicitHeight + Kirigami.Units.smallSpacing

                RowLayout {
                    id: statusRow
                    anchors.centerIn: parent
                    spacing: Kirigami.Units.smallSpacing * 0.7

                    Rectangle {
                        width: Kirigami.Units.smallSpacing * 0.7
                        height: width
                        radius: width / 2
                        color: Kirigami.Theme.highlightColor
                        SequentialAnimation on opacity {
                            loops: Animation.Infinite
                            running: AppController.capturing
                            NumberAnimation { from: 1; to: 0.25; duration: 700 }
                            NumberAnimation { from: 0.25; to: 1; duration: 700 }
                        }
                    }
                    Label {
                        text: previewRect.previewStatus
                        color: "white"
                        font.pixelSize: 11
                        font.weight: Font.DemiBold
                    }
                }
            }
        }

        // Circular progress + copy
        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing

            Item {
                Layout.preferredWidth: Kirigami.Units.gridUnit * 3.6
                Layout.preferredHeight: Kirigami.Units.gridUnit * 3.6

                Canvas {
                    id: ringCanvas
                    anchors.fill: parent
                    property real value: AppController.progress

                    onValueChanged: requestPaint()
                    Component.onCompleted: requestPaint()

                    onPaint: {
                        var ctx = getContext("2d");
                        ctx.reset();
                        var cx = width / 2, cy = height / 2;
                        var r = Math.min(width, height) / 2 - 4;
                        var lineWidth = 5;

                        ctx.lineWidth = lineWidth;
                        ctx.lineCap = "round";

                        // Track
                        ctx.strokeStyle = Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, 0.18);
                        ctx.beginPath();
                        ctx.arc(cx, cy, r, 0, 2 * Math.PI);
                        ctx.stroke();

                        // Fill
                        if (value > 0) {
                            ctx.strokeStyle = Kirigami.Theme.highlightColor;
                            ctx.beginPath();
                            var start = -Math.PI / 2;
                            var end = start + (value / 100) * 2 * Math.PI;
                            ctx.arc(cx, cy, r, start, end);
                            ctx.stroke();
                        }
                    }
                }

                Label {
                    anchors.centerIn: parent
                    text: Math.round(AppController.progress) + "%"
                    font.pixelSize: 14
                    font.weight: Font.Bold
                    font.family: "monospace"
                    color: Kirigami.Theme.textColor
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2

                Label {
                    id: progressLabel
                    text: I18n.tr("enrollment.progress")
                    font.weight: Font.DemiBold
                    font.pixelSize: 13
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                }
                Label {
                    text: I18n.tr("enrollment.instructions")
                    font.pixelSize: 11
                    color: Kirigami.Theme.disabledTextColor
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }
            }
        }

        Item { Layout.fillHeight: true }

        // Action buttons
        RowLayout {
            spacing: Kirigami.Units.mediumSpacing * 1.5
            Layout.fillWidth: true

            Button {
                text: I18n.tr("enrollment.cancelBtn")
                flat: true
                onClicked: {
                    AppController.navigateToHomeImpl();
                    previewRect.previewStatus = I18n.tr("enrollment.previewCancelled");
                }
            }

            Item { Layout.fillWidth: true }

            Button {
                text: I18n.tr("enrollment.stopBtn")
                enabled: AppController.capturing
                onClicked: {
                    AppController.stopCapture();
                    previewRect.previewStatus = I18n.tr("enrollment.previewStopped");
                }
            }

            Button {
                text: I18n.tr("enrollment.startBtn")
                enabled: !AppController.capturing
                highlighted: true

                palette.buttonText: Kirigami.Theme.highlightedTextColor
                palette.button: Kirigami.Theme.highlightColor

                onClicked: {
                    AppController.startCapture();
                }
            }
        }
    }

    // Automatic return to home 2 seconds after success
    Timer {
        id: navigateHomeTimer
        interval: 2000
        repeat: false
        onTriggered: AppController.navigateToHomeImpl()
    }

    // Connections to app signals
    Connections {
        target: AppController

        function onAppProgressChanged(value) {
            if (value >= 100) {
                previewRect.previewStatus = I18n.tr("enrollment.previewAnalyzing");
                progressLabel.text = I18n.tr("enrollment.previewValidating");
            }
        }

        function onCaptureCompletedSignal() {
            previewRect.previewStatus = I18n.tr("enrollment.previewSuccess");
            navigateHomeTimer.start();
        }

        function onCaptureErrorSignal(message) {
            previewRect.previewStatus = I18n.tr("enrollment.previewError") + " " + message;
        }
    }
}
