import QtQuick
import QtQuick.Controls
import org.kde.kirigami as Kirigami
import Linux.Hello 1.0

// Full GPL text, fetched lazily by AppController.navigateToLicenseImpl()
// (see AppController.qml's loadLicenseText) — nothing is requested until
// the user actually taps the "License" row in About.qml.
Kirigami.ScrollablePage {
    id: licensePage

    title: I18n.tr("about.licenseTitle")

    Label {
        width: licensePage.width - Kirigami.Units.largeSpacing * 4
        text: AppController.licenseText !== "" ? AppController.licenseText : I18n.tr("about.licenseLoading")
        textFormat: Text.PlainText
        wrapMode: Text.WordWrap
        font.family: "monospace"
        font.pixelSize: 11
        color: Kirigami.Theme.textColor
    }
}
