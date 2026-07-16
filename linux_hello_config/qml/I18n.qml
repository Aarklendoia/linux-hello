pragma Singleton
import QtQuick

QtObject {
    id: i18n

    // Populated by loadLanguage() from i18n/<lang>.json, including for
    // English — en.json is this app's single source of truth for English
    // strings, not duplicated here. If loading ever fails (e.g. a packaging
    // issue that ships the QML without its i18n/ directory), tr() below
    // already has its own fallback: it returns the raw key string rather
    // than crashing, which is a more honest failure mode than silently
    // serving a second, easily-drifting copy of every English string.
    property var translations: ({})
    property string currentLanguage: "en"

    // List of available languages
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
        // Requires QML_XHR_ALLOW_FILE_READ=1 (set by linux_hello_config's
        // main.rs when launching qml6) — XMLHttpRequest on a local file is
        // blocked by default otherwise.
        var qmlPath = Qt.resolvedUrl("./i18n/" + lang + ".json")
        var xhr = new XMLHttpRequest()
        xhr.open("GET", qmlPath, false)
        try {
            xhr.send()
            if (xhr.status === 200) {
                var loaded = JSON.parse(xhr.responseText)
                if (loaded && typeof loaded === 'object') {
                    translations = loaded
                    currentLanguage = lang
                    return true
                }
            }
        } catch (e) {
            // Silent fallback — tr() returns raw keys for anything missing.
        }

        currentLanguage = lang
        return true
    }

    function tr(key) {
        if (!key || key === "") {
            return key
        }
        
        // First check if the key exists directly (for flat keys)
        if (key in translations) {
            return translations[key]
        }
        
        // Try navigating by dots (for nested keys)
        var keys = key.split('.')
        var value = translations
        
        for (var i = 0; i < keys.length; i++) {
            if (value && typeof value === 'object' && keys[i] in value) {
                value = value[keys[i]]
            } else {
                // If the key does not exist, return the key itself as a fallback
                return key
            }
        }
        
        return typeof value === 'string' ? value : key
    }

    Component.onCompleted: {
        // Load the system language
        var systemLang = Qt.locale().name.substring(0, 2).toLowerCase()
        if (languages.includes(systemLang)) {
            loadLanguage(systemLang)
        } else {
            // Default to English
            loadLanguage("en")
        }
    }
}

