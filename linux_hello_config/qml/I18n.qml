pragma Singleton
import QtQuick

QtObject {
    id: i18n

    // Traductions en dur en anglais (fallback)
    property var translations: ({
        "home.title": "Home",
        "home.welcome": "Welcome to Linux Hello",
        "home.youCan": "You can:",
        "home.action1": "Register a new face for authentication",
        "home.action2": "Manage your registered faces",
        "home.action3": "Configure authentication settings",
        "home.registerBtn": "Register Face",
        "home.manageFacesBtn": "Manage Faces",
        "home.settingsBtn": "Settings",
        "app.subtitle": "Advanced face recognition authentication for Linux",
        "enrollment.title": "Register New Face",
        "enrollment.registerNew": "Register a New Face",
        "enrollment.cameraPreview": "Camera Preview",
        "enrollment.progress": "Progress",
        "enrollment.instructions": "Position your face in front of the camera and look towards it",
        "enrollment.startBtn": "Start Capture",
        "enrollment.stopBtn": "Stop Capture",
        "enrollment.cancelBtn": "Cancel",
        "manageFaces.title": "Manage Faces",
        "manageFaces.registeredFaces": "Registered Faces",
        "manageFaces.confidence": "Confidence",
        "manageFaces.registered": "Registered",
        "manageFaces.unknown": "Unknown",
        "manageFaces.deleteBtn": "Delete",
        "manageFaces.noFaces": "No faces registered yet",
        "manageFaces.registerNewBtn": "Register New Face",
        "manageFaces.backBtn": "Back",
        "settings.title": "Settings",
        "settings.general": "General Settings",
        "settings.language": "Language",
        "settings.theme": "Theme"
    })
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
        // En Qt6 avec XMLHttpRequest, le chargement de fichiers locaux est bloqué
        // Nous utilisons les traductions hardcodées comme fallback
        try {
            var paths = [
                "file:///usr/share/linux-hello/qml-modules/Linux/Hello/i18n/" + lang + ".json",
                Qt.resolvedUrl("./i18n/" + lang + ".json"),
                "qrc:/i18n/" + lang + ".json"
            ]
            
            // Essayer d'abord via Qt
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
                // Fallback silencieux
            }
        } catch (e) {
            // Silencieux
        }
        
        // Si pas de fichier JSON, utiliser les traductions hardcodées en anglais
        currentLanguage = "en"
        return true
    }

    function tr(key) {
        if (!key || key === "") {
            return key
        }
        
        // Vérifier d'abord si la clé existe directement (pour les clés plates)
        if (key in translations) {
            return translations[key]
        }
        
        // Essayer de naviguer par points (pour les clés imbriquées)
        var keys = key.split('.')
        var value = translations
        
        for (var i = 0; i < keys.length; i++) {
            if (value && typeof value === 'object' && keys[i] in value) {
                value = value[keys[i]]
            } else {
                // Si la clé n'existe pas, retourner la clé elle-même comme fallback
                return key
            }
        }
        
        return typeof value === 'string' ? value : key
    }

    Component.onCompleted: {
        // Charger la langue du système
        var systemLang = Qt.locale().name.substring(0, 2).toLowerCase()
        if (languages.includes(systemLang)) {
            loadLanguage(systemLang)
        } else {
            // English par défaut
            loadLanguage("en")
        }
    }
}

