pragma Singleton
import QtQuick 2.15

QtObject {
    id: i18n

    property var translations: ({})
    property string currentLanguage: "en"

    // Liste des langues disponibles
    readonly property var languages: ["en", "zh", "es", "hi", "ar", "pt", "ru", "ja", "de", "fr"]
    readonly property var languageNames: ({
        "en": "English",
        "zh": "中文 (Chinese)",
        "es": "Español (Spanish)",
        "hi": "हिंदी (Hindi)",
        "ar": "العربية (Arabic)",
        "pt": "Português (Portuguese)",
        "ru": "Русский (Russian)",
        "ja": "日本語 (Japanese)",
        "de": "Deutsch (German)",
        "fr": "Français (French)"
    })

    function loadLanguage(lang) {
        try {
            var fileUrl = Qt.resolvedUrl("./i18n/" + lang + ".json")
            var xhr = new XMLHttpRequest()
            xhr.open("GET", fileUrl, false)
            xhr.send()
            
            if (xhr.status === 200) {
                translations = JSON.parse(xhr.responseText)
                currentLanguage = lang
                return true
            }
        } catch (e) {
            console.error("Failed to load language:", lang, e.message)
            // Essayer de charger l'anglais par défaut si la langue demandée échoue
            if (lang !== "en") {
                return loadLanguage("en")
            }
        }
        return false
    }

    function tr(key) {
        if (!key || key === "") {
            return key
        }
        
        var keys = key.split('.')
        var value = translations
        
        for (var i = 0; i < keys.length; i++) {
            if (value && typeof value === 'object' && keys[i] in value) {
                value = value[keys[i]]
            } else {
                console.warn("Translation key not found:", key)
                return key
            }
        }
        
        return typeof value === 'string' ? value : key
    }

    Component.onCompleted: {
        // Charger la langue anglaise par défaut
        loadLanguage("en")
    }
}

