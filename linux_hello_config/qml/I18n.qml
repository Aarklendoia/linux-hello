pragma Singleton
import QtQuick

QtObject {
    id: i18n

    // Hardcoded English translations (fallback)
    property var translations: ({
        "home.title": "Home",
        "home.welcome": "Welcome to Linux Hello",
        "home.youCan": "You can:",
        "home.action1": "Register a new face for authentication",
        "home.action2": "Manage your registered faces",
        "home.registerBtn": "Register Face",
        "home.registerBtnDesc": "Add a new biometric profile",
        "home.manageFacesBtn": "Manage Faces",
        "home.manageFacesBtnDesc": "%1 faces registered",
        "home.manageFacesBtnDescOne": "1 face registered",
        "home.daemonActive": "Daemon active",
        "home.daemonInactive": "Daemon unavailable",
        "home.daemonActiveSub": "Ready to authenticate",
        "home.daemonInactiveSub": "Check the hello-daemon service",
        "home.fallbackNote": "If your face isn't recognized, your password always works — face recognition only ever adds a faster option, it never locks you out.",
        "app.subtitle": "Advanced face recognition authentication for Linux",
        "enrollment.title": "Register New Face",
        "enrollment.registerNew": "Register a New Face",
        "enrollment.cameraPreview": "Camera Preview",
        "enrollment.progress": "Progress",
        "enrollment.instructions": "Position your face in front of the camera and look towards it",
        "enrollment.startBtn": "Start Capture",
        "enrollment.stopBtn": "Stop Capture",
        "enrollment.cancelBtn": "Cancel",
        "enrollment.previewInactive": "Preview inactive",
        "enrollment.previewCancelled": "Capture cancelled",
        "enrollment.previewStopped": "Capture stopped",
        "enrollment.previewAnalyzing": "Analyzing face…",
        "enrollment.previewValidating": "Validating…",
        "enrollment.previewSuccess": "✓ Face registered successfully!",
        "enrollment.previewError": "✗ Error:",
        "manageFaces.title": "Manage Faces",
        "manageFaces.registeredFaces": "Registered Faces",
        "manageFaces.confidence": "Confidence",
        "manageFaces.registered": "Registered",
        "manageFaces.unknown": "Unknown",
        "manageFaces.deleteBtn": "Delete",
        "manageFaces.noFaces": "No faces registered yet",
        "manageFaces.registerNewBtn": "Register New Face",
        "manageFaces.backBtn": "Back",
        "manageFaces.sample": "sample",
        "manageFaces.anyFaceNote": "Any registered face can authenticate you — for sudo, screen unlock, and anywhere else Linux Hello is enabled."
    })
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
        // In Qt6 with XMLHttpRequest, loading local files is blocked
        // We use the hardcoded translations as a fallback
        try {
            var paths = [
                "file:///usr/share/linux-hello/qml-modules/Linux/Hello/i18n/" + lang + ".json",
                Qt.resolvedUrl("./i18n/" + lang + ".json"),
                "qrc:/i18n/" + lang + ".json"
            ]
            
            // Try via Qt first
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
                // Silent fallback
            }
        } catch (e) {
            // Silent
        }

        // If there is no JSON file, use the hardcoded English translations
        currentLanguage = "en"
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

