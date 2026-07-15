# Changelog

## [1.3.0](https://github.com/Aarklendoia/linux-hello/compare/v1.2.1...v1.3.0) (2026-07-15)


### Features

* **gui:** redesign Home/Enrollment/ManageFaces + smoother camera preview ([36fb789](https://github.com/Aarklendoia/linux-hello/commit/36fb7895b104c84362e0d2a021c3a428929ca323))
* **gui:** redesign Home/Enrollment/ManageFaces with a friendlier Kirigami look ([35cd98b](https://github.com/Aarklendoia/linux-hello/commit/35cd98bf3ed97ab5d6d193759cc75c5af8045f7b))


### Bug Fixes

* **gui:** remove the fake Settings page ([8d058c9](https://github.com/Aarklendoia/linux-hello/commit/8d058c95de8957722b078136721dfb0be2218586))
* **gui:** derive camera preview color from the Plasma theme and drop mid-sentence capitals in fr/es/pt/ru/de translations ([19a401e](https://github.com/Aarklendoia/linux-hello/commit/19a401ec921306fedfc5b507c0302e13e98ee6e0))


### Performance Improvements

* **gui:** smooth out the enrollment camera preview ([2e9b347](https://github.com/Aarklendoia/linux-hello/commit/2e9b3470e02904e3a440419caf6a6b364432235d))

## [1.2.1](https://github.com/Aarklendoia/linux-hello/compare/v1.2.0...v1.2.1) (2026-07-10)


### Bug Fixes

* install debhelper (and fakeroot/quilt) in publish-ppa.yml ([26a2595](https://github.com/Aarklendoia/linux-hello/commit/26a2595dc5a1bcce90bdcbd675591e585e1e0d25))

## [1.2.0](https://github.com/Aarklendoia/linux-hello/compare/v1.1.1...v1.2.0) (2026-07-10)


### Features

* turn linux-hello into a real metapackage ([64bbe8c](https://github.com/Aarklendoia/linux-hello/commit/64bbe8c1f2a86f5327f26761e8cf90b5b38b55b0))


### Bug Fixes

* make libpam-linux-hello pull in a complete, working setup ([49727d3](https://github.com/Aarklendoia/linux-hello/commit/49727d30e3dadbb13abcc850b2306e1996df00fd))

## [1.1.1](https://github.com/Aarklendoia/linux-hello/compare/v1.1.0...v1.1.1) (2026-07-10)


### Bug Fixes

* add missing cargo/rustc Build-Depends ([ebbae9c](https://github.com/Aarklendoia/linux-hello/commit/ebbae9ca85f2415d2845abf231d43e815dcc9c8d))
* add missing libv4l-dev/libjpeg-dev/libclang-dev/clang Build-Depends ([3c816a4](https://github.com/Aarklendoia/linux-hello/commit/3c816a4f835df11dccbd192ac3ac6aa78558567e))
* disable per-file checksum verification for vendored crates ([d48d752](https://github.com/Aarklendoia/linux-hello/commit/d48d752835af485de476e371c06254919484ac58))
* downgrade sqlx to 0.8 to unblock Launchpad's rustc 1.93.1 ([3d915c7](https://github.com/Aarklendoia/linux-hello/commit/3d915c73127298352bdf10dc7847fd776f27267e))
* downgrade sqlx to 0.8 to unblock Launchpad's rustc 1.93.1 ([a1facd6](https://github.com/Aarklendoia/linux-hello/commit/a1facd6f7e0c8f90e275fedf51642ad5347f9141))
* exclude vendor/ from dh_clean's default cruft removal ([01dc891](https://github.com/Aarklendoia/linux-hello/commit/01dc891d96d67c4f50e59af89fd281835fcafb61))
* remove redundant borrow in pam_linux_hello log format string ([200b707](https://github.com/Aarklendoia/linux-hello/commit/200b7076952b22af10a9835dce9bc78e58769c1a))
* ship ONNX models in-tree instead of the packager's \$HOME ([904caa9](https://github.com/Aarklendoia/linux-hello/commit/904caa96cb12d8c959fd7b96b3b3aeeba69cfbe4))
* stop using tar-ignore for target/, remove it outright instead ([00c10e8](https://github.com/Aarklendoia/linux-hello/commit/00c10e8098efb65507b8d7aebacaf44419342c0e))
* use a valid Debian archive section in debian/control ([07320ec](https://github.com/Aarklendoia/linux-hello/commit/07320ecb6b082ec00221e886d3e4f638e6c2c4e6))
* vendor with the target series' exact cargo version, not local stable ([821ec8c](https://github.com/Aarklendoia/linux-hello/commit/821ec8cad5806431362c424432a4ac817e8bab18))

## [1.1.0](https://github.com/Aarklendoia/linux-hello/compare/v1.0.6...v1.1.0) (2026-07-08)


### Features

* Add comprehensive CI/CD pipeline and development tooling ([508aedc](https://github.com/Aarklendoia/linux-hello/commit/508aedc8df4effb9082c58301b0ce1578aaeb7a0))
* Add Debian source format 3.0 (quilt) and CI/CD pipeline ([81ac0e1](https://github.com/Aarklendoia/linux-hello/commit/81ac0e17786c8e8d3b2e4074287476bcc1f0c950))
* Add GUI package for Linux Hello with configuration tool and QML assets ([76cc8ff](https://github.com/Aarklendoia/linux-hello/commit/76cc8ff7d8a37eaf6f71de0000f67f9c35d46f32))
* Add internationalization support and language management ([e59700f](https://github.com/Aarklendoia/linux-hello/commit/e59700f4b13d6e2508273f7a5948f3ec76d0e443))
* add live screenlock status, manual retry, and password fallback ([1cabd53](https://github.com/Aarklendoia/linux-hello/commit/1cabd53e8bb73cb70f9f40cbde1c97d0de00870e))
* add live screenlock status, manual retry, and password fallback ([88735b0](https://github.com/Aarklendoia/linux-hello/commit/88735b0d23226e12c95551dabb5145e3068dea63))
* add live status indicator to the SDDM login greeter ([3f8feb5](https://github.com/Aarklendoia/linux-hello/commit/3f8feb5e4995f97310ad03e2fc6e11bfafd7fd27))
* add live status indicator to the SDDM login greeter ([479ee51](https://github.com/Aarklendoia/linux-hello/commit/479ee517147287f5c6af9fb86a0466d1731574ef))
* Add post-installation, pre-installation, and post-removal scripts for Linux Hello ([43f17b1](https://github.com/Aarklendoia/linux-hello/commit/43f17b17b18ccae693132a93c5cd9e8a06b2e299))
* Add Qt6 GUI launcher and update installation scripts ([77fc17b](https://github.com/Aarklendoia/linux-hello/commit/77fc17bbb4cd42f5213cc6f7c9abbdff55d3d1a6))
* add SDDM (login screen) biometric authentication support ([55d05fa](https://github.com/Aarklendoia/linux-hello/commit/55d05fa173b2a99ab8b6595bd4eb0a48bfd82eb5))
* add SDDM (login screen) biometric authentication support ([e8d152d](https://github.com/Aarklendoia/linux-hello/commit/e8d152d4bd48f76ff4403a061cbe3a0dbe40d9be))
* ajouter des dépendances pour le support de la vidéo dans les workflows CI ([629ef88](https://github.com/Aarklendoia/linux-hello/commit/629ef88f3332b1525797457d0d5147096a0fab83))
* Ajouter des fichiers d'installation pour pam_linux_hello et linux-hello-tools ([86af9ff](https://github.com/Aarklendoia/linux-hello/commit/86af9ffc96e453e9a276060016db96cc0792495c))
* Ajouter des fichiers de documentation pour le projet Linux Hello, y compris le rapport d'achèvement et les résultats des tests ([821bddf](https://github.com/Aarklendoia/linux-hello/commit/821bddf51ee0d1ff78fe5b5fd9b7a3c4a9c5dfe7))
* Ajouter l'implémentation du gestionnaire de caméra et du module de matching des visages ([5d30239](https://github.com/Aarklendoia/linux-hello/commit/5d30239062633da14911bc772abc49938ab7bea8))
* Ajouter l'infrastructure d'animation et les tests pour la barre de progression et les effets visuels ([c43a409](https://github.com/Aarklendoia/linux-hello/commit/c43a409b5b138c6b540553e560bf78b2e4f65ce3))
* Ajouter l'intégration du ticker d'animation et les effets de transition des boutons ([e55d7c8](https://github.com/Aarklendoia/linux-hello/commit/e55d7c8f7eab01ac918f2ade1534ebb900bdf093))
* Ajouter l'interface D-Bus pour le daemon d'authentification faciale ([f947912](https://github.com/Aarklendoia/linux-hello/commit/f9479129fb06804df9e49366cc0bb04fa8320660))
* Ajouter la fonction pam_sm_setcred pour la gestion des crédits PAM ([2cbd44c](https://github.com/Aarklendoia/linux-hello/commit/2cbd44c947e21e0c964adb467d3689e25d478f07))
* Ajouter la gestion des signaux D-Bus pour le streaming de capture et implémenter le client D-Bus pour la GUI ([9215b8d](https://github.com/Aarklendoia/linux-hello/commit/9215b8d8f8ce354bb1c65ea042554ca42192c574))
* ajouter la gestion des visages avec des appels à busctl pour lister et supprimer des visages ([11c9d04](https://github.com/Aarklendoia/linux-hello/commit/11c9d0498b4f4ec382d950de6d2da107c40166d9))
* ajouter la gestion du cache UID et améliorer l'affichage des utilisateurs dans la liste des visages ([1936c59](https://github.com/Aarklendoia/linux-hello/commit/1936c59502e6710a19fe0eb7067c3f378aa3e478))
* ajouter la surveillance du verrouillage d'écran avec authentification faciale automatique ([225946b](https://github.com/Aarklendoia/linux-hello/commit/225946b7bb60be25895bc9fae8051ddab78c0059))
* Ajouter le paquet linux-hello-models avec les modèles ONNX ([6590c82](https://github.com/Aarklendoia/linux-hello/commit/6590c824f7500361232b93b3daf5b1bb88cd3943))
* Ajouter le rapport de finalisation de la phase 3.4 avec des optimisations d'animation et de rendu ([e34b177](https://github.com/Aarklendoia/linux-hello/commit/e34b177c4fbfe3b19dfc9b7a7da0f0e81f82dd3f))
* Ajouter le rapport de validation des performances pour la phase 3.4 et les tests associés ([32c1f19](https://github.com/Aarklendoia/linux-hello/commit/32c1f197bfc8464c273f9795c0e61222c9fb1fae))
* ajouter libclang-dev et clang comme dépendances dans les workflows CI et tests ([dfecf7c](https://github.com/Aarklendoia/linux-hello/commit/dfecf7c83c14f5bbd6d4c0881356813e4369a304))
* Ajouter un client de test CLI pour le daemon avec des commandes d'enregistrement, de vérification, de liste et de suppression des visages ([c5dd808](https://github.com/Aarklendoia/linux-hello/commit/c5dd808ec159f0153ae7c246bbc807e3b87aa65e))
* ajouter un contrôleur d'application singleton et intégrer la gestion des visages dans l'interface QML ([17c0fce](https://github.com/Aarklendoia/linux-hello/commit/17c0fce73ea63e60320b465cf3f7344aef3bd99c))
* ajouter un overlay QML pour le verrouillage d'écran KDE et gérer son installation et restauration ([bd85ab3](https://github.com/Aarklendoia/linux-hello/commit/bd85ab30d20617e7e31346811c67d475e2e3e366))
* ajouter un serveur MJPEG pour l'aperçu vidéo et améliorer la gestion des frames ([69ff6b8](https://github.com/Aarklendoia/linux-hello/commit/69ff6b8aab95cd359c547184eb3b1bde40c4ed68))
* Ajouter un système d'aperçu vidéo avec exportation de frames et intégration dans la GUI ([b384368](https://github.com/Aarklendoia/linux-hello/commit/b3843689ac0fd57c020853652215175971a542d0))
* Ajouter une implémentation simple de détection et d'extraction d'embeddings pour le moteur de reconnaissance faciale ([efe5600](https://github.com/Aarklendoia/linux-hello/commit/efe56006c4d733f2c377c392fe541ffaf0482e8b))
* Améliorer la gestion des captures de frames et ajouter des tests pour le fallback ([15c1c52](https://github.com/Aarklendoia/linux-hello/commit/15c1c522f06c9eb9b77be8fe12ac19da11d6ca09))
* automatically activate PAM biometric auth for sudo/screenlock ([62238a0](https://github.com/Aarklendoia/linux-hello/commit/62238a08ebd6fb9cf66f2982dec1f1aff0ef6b4f))
* automatically activate PAM biometric auth for sudo/screenlock ([ac61a50](https://github.com/Aarklendoia/linux-hello/commit/ac61a505c65326ab7d9b06009a25806351832adc))
* Configure environment variables for Qt/QML runtime in launcher ([89c1577](https://github.com/Aarklendoia/linux-hello/commit/89c1577d6e0f5b9c8cb2ba7a7fa59c8710c19dfe))
* enhance camera preview functionality and improve user experience ([b2de952](https://github.com/Aarklendoia/linux-hello/commit/b2de9524c2bad4b0ac20c68e5c9fd1c2e656fb42))
* i18n des messages PAM console (10 langues: en/fr/de/es/pt/ru/ja/zh/ar/hi) ([b715dbf](https://github.com/Aarklendoia/linux-hello/commit/b715dbf80c25d0b24ad4ef2b17d90ad00e7fb19a))
* Implement complete GUI infrastructure for KDE/Wayland ([65b2fdb](https://github.com/Aarklendoia/linux-hello/commit/65b2fdb75aca07110f82102c2f3d586aa54c06f6))
* Implement facial authentication daemon with D-Bus interface ([da48175](https://github.com/Aarklendoia/linux-hello/commit/da48175b70ff802767f19640f469ed83318011f7))
* Implement PAM Helper Daemon for D-Bus Access Workaround ([59faa73](https://github.com/Aarklendoia/linux-hello/commit/59faa7346889209d559f9cec9ee404ef364aa5a5))
* Implement Phase 3.3 - Live Preview Rendering with Bounding Box and Progress Bar ([6a7ae0d](https://github.com/Aarklendoia/linux-hello/commit/6a7ae0d938831cfe590033ef3f8f2c82ebdd57cb))
* Implement SCRFD-500M face detection and authentication test feature ([cfb74f7](https://github.com/Aarklendoia/linux-hello/commit/cfb74f7561d656bf2169e41663ad5ab173482da1))
* Implement testing scripts for PAM integration ([53b9943](https://github.com/Aarklendoia/linux-hello/commit/53b99431685bc494f7c0819d4a3f70a5b70deb57))
* Implémenter l'infrastructure du ticker d'animation pour des mises à jour fluides à ~60fps ([31ac3ee](https://github.com/Aarklendoia/linux-hello/commit/31ac3ee4ad3054d02f4e9435842332f4deb91001))
* Implémenter la capture de frames en streaming avec D-Bus et ajout de tests ([986cfb8](https://github.com/Aarklendoia/linux-hello/commit/986cfb8cc42c62b39b60865ec19dc6ab0adcd517))
* Implémenter un backend V4L2 réel avec accès direct aux devices et gestion améliorée des captures ([299b1a5](https://github.com/Aarklendoia/linux-hello/commit/299b1a5dd3f929fea53af76ea25d230f6dd938b1))
* Introduce PAM configuration files for sudo and KDE screenlock ([53b9943](https://github.com/Aarklendoia/linux-hello/commit/53b99431685bc494f7c0819d4a3f70a5b70deb57))
* keep the camera continuously engaged for the whole verify window ([1733a4f](https://github.com/Aarklendoia/linux-hello/commit/1733a4f5f1987870107632eeb156b5822ff824d5))
* keep the camera continuously engaged for the whole verify window ([a1a35e3](https://github.com/Aarklendoia/linux-hello/commit/a1a35e3ccc151d40ff89afa169f647ba0ff94295))
* Mettre à jour la dépendance sqlx pour utiliser les nouvelles fonctionnalités de runtime et de TLS ([412f966](https://github.com/Aarklendoia/linux-hello/commit/412f966bc5da1feba2b862bce7315baa3e6a2c7f))
* Mettre à jour les dépendances et améliorer la documentation dans les fichiers changelog et control ([9138a7f](https://github.com/Aarklendoia/linux-hello/commit/9138a7f2ce5db0ab0492ec80ba2d7c19f88967f1))
* Refactor Linux Hello configuration GUI: transition from Iced to QML with Kirigami, streamline application structure, and enhance user interface for face enrollment and management. ([d54b36c](https://github.com/Aarklendoia/linux-hello/commit/d54b36c5de916f81cf70790ef07d5bc2dd7458b8))
* require explicit confirmation before granting sudo via face match ([08b6415](https://github.com/Aarklendoia/linux-hello/commit/08b641523a8bf793d8e87032c5985a429db27e05))
* require explicit confirmation before granting sudo via face match ([0c0d3f9](https://github.com/Aarklendoia/linux-hello/commit/0c0d3f998daea917b6d57647a05e5eafda8a2a15))
* retry face recognition automatically on mouse activity, not just keypresses ([e8ef848](https://github.com/Aarklendoia/linux-hello/commit/e8ef848c57dafa2230124b92b971fcb5671a072b))
* Traduire l'interface utilisateur en anglais pour les pages d'inscription, de gestion des visages et de paramètres ([8510314](https://github.com/Aarklendoia/linux-hello/commit/851031497b4e7dd2796b96ce636b624585a06e62))
* Update QUICKSTART guide for Linux Hello with new testing and installation steps ([53b9943](https://github.com/Aarklendoia/linux-hello/commit/53b99431685bc494f7c0819d4a3f70a5b70deb57))
* Update to Qt 6.5 / Kirigami 2.20 with qml6 launcher ([4d9aa67](https://github.com/Aarklendoia/linux-hello/commit/4d9aa67ecbd4cd0791eb7aaa898ac04abcb25a50))
* wire the linux-hello CLI's enroll/verify/list/delete to the daemon ([8feccff](https://github.com/Aarklendoia/linux-hello/commit/8feccff92aed78a651be02c0d8d7a459fe9ba90b))
* wire the linux-hello CLI's enroll/verify/list/delete to the daemon ([4e7fcf9](https://github.com/Aarklendoia/linux-hello/commit/4e7fcf9837dbae5821c4f94dbfabc83ee0038315))


### Bug Fixes

* Add CMake and Qt6 dev tools to GitHub Actions build dependencies ([9712cf6](https://github.com/Aarklendoia/linux-hello/commit/9712cf6f00601f832831ff5d470ab68b5ab1e8da))
* ajouter la configuration des fichiers temporaires systemd pour le socket PAM ([42a0727](https://github.com/Aarklendoia/linux-hello/commit/42a07279c83732b5d3ca761153da33aef1a3e048))
* ajouter linux-hello-models au fichier .gitignore ([f7438db](https://github.com/Aarklendoia/linux-hello/commit/f7438dbb27a13164bcc415d4ee168df7f571ad0c))
* ajouter un seuil de contexte pour polkit et améliorer la sécurité du socket PAM ([192c34c](https://github.com/Aarklendoia/linux-hello/commit/192c34c6dbc0ffe437eb83e37a4ae956682368ce))
* ajuster les seuils de contexte pour le matching des visages ([d25c34c](https://github.com/Aarklendoia/linux-hello/commit/d25c34ce7154321cae6bcf5d3c3206775f123a1f))
* amélioration de la lisibilité du code dans l'implémentation de FaceAuthDaemon ([f56914b](https://github.com/Aarklendoia/linux-hello/commit/f56914b8607f607fe44b669013f869235a87a072))
* améliorer la gestion des erreurs et la sécurité dans le module PAM, normaliser les scores de vivacité IR ([308d35c](https://github.com/Aarklendoia/linux-hello/commit/308d35c72b4f66301c81803950768c9abcf100c7))
* améliorer la lisibilité des logs en formatant les messages d'information ([2cf7d78](https://github.com/Aarklendoia/linux-hello/commit/2cf7d785525a7825ba89482e02492889e71d6338))
* améliorer la lisibilité du code en formatant les appels de fonction ([4c45418](https://github.com/Aarklendoia/linux-hello/commit/4c45418ad93766302d3ba3c6a1bca8ed911ac2d9))
* améliorer la normalisation des embeddings et ajuster les seuils de similarité pour les contextes sensibles ([506ed74](https://github.com/Aarklendoia/linux-hello/commit/506ed74470dc70689f7ebf7016b31f7427378422))
* Améliorer la restauration des sauvegardes PAM et supprimer le flag silence ([623d42f](https://github.com/Aarklendoia/linux-hello/commit/623d42ff47d7c95414f080919c47412f1da85673))
* augmenter le délai d'attente à 30 secondes dans les options PAM ([844a6c8](https://github.com/Aarklendoia/linux-hello/commit/844a6c85ab29f750a4179789e4c17fcea51a2e16))
* compilation ([5c25f44](https://github.com/Aarklendoia/linux-hello/commit/5c25f44cac9c06351326eccfc2b1bd3f4b082df3))
* Copy Debian artifacts to workspace directory for upload ([a0b51a2](https://github.com/Aarklendoia/linux-hello/commit/a0b51a2da974ca989a2e429f634cde459c736c6a))
* Correct GitHub Actions workflows ([444faa1](https://github.com/Aarklendoia/linux-hello/commit/444faa11a862130f5a79c19e551339abad6ef37a))
* Correct GUI binary name (underscores not hyphens) ([90b0564](https://github.com/Aarklendoia/linux-hello/commit/90b05643ed57124b294f337ad151443450950898))
* correction suite màj tract 0.23 ([cc55479](https://github.com/Aarklendoia/linux-hello/commit/cc55479ec33cdafd277e793809bfd13ddb841e43))
* Corriger dh_missing et formatage rustfmt ([2841b41](https://github.com/Aarklendoia/linux-hello/commit/2841b41b199edb3a85d3c795e142296a29a56bca))
* Corriger l'exportation de la frame de prévisualisation en supprimant l'async ([b4b4551](https://github.com/Aarklendoia/linux-hello/commit/b4b45519a9016b0f5f45da7d36fdfcd042f04c09))
* corriger l'installation de l'icône de l'application pour éviter les problèmes de renommage ([91280f1](https://github.com/Aarklendoia/linux-hello/commit/91280f124e65820073e2b04b6eab2c6092972311))
* Corriger la syntaxe debian/control et exclure du linter Markdown ([c2f64cb](https://github.com/Aarklendoia/linux-hello/commit/c2f64cb292e81819b77e99741e5a7a553f2fc563))
* Corriger les workflows CI et le format source Debian ([9fba9e9](https://github.com/Aarklendoia/linux-hello/commit/9fba9e9c0d725a910171b59a757fcf6d911af3ae))
* Enhance error handling and logging in PAM module ([53b9943](https://github.com/Aarklendoia/linux-hello/commit/53b99431685bc494f7c0819d4a3f70a5b70deb57))
* formatage ([34c5e1a](https://github.com/Aarklendoia/linux-hello/commit/34c5e1a3ecc580f71d6d9c3340492bbd4294193f))
* formatage ([2b3a5e9](https://github.com/Aarklendoia/linux-hello/commit/2b3a5e92c5df3d3b4e4c854fe616a8948ec4afa4))
* Icon installation path and daemon autostart ([55443a7](https://github.com/Aarklendoia/linux-hello/commit/55443a7fe4bceb11662beba643ec9b2a7a410ef4))
* Intégration PAM biométrique complète via socket Unix ([13e498f](https://github.com/Aarklendoia/linux-hello/commit/13e498f5bf8e127c05c59485c523eddae1265229))
* IR not used ([2c1e153](https://github.com/Aarklendoia/linux-hello/commit/2c1e153e2998d8ce206c6430aa3d7a2fab9574c7))
* mettre à jour la version de rand et modifier la dépendance image pour le traitement d'images ([f769e8b](https://github.com/Aarklendoia/linux-hello/commit/f769e8bec9c27a7005dca435cb4fd91e9bfca250))
* Prevent duplicate taskbar icons in QML/KDE ([cfecad4](https://github.com/Aarklendoia/linux-hello/commit/cfecad4fc086b3384039ee2f4a2bc8d805bd91b9))
* prevent NaN-confidence panics from crashing hello-daemon during screenlock ([954a7ce](https://github.com/Aarklendoia/linux-hello/commit/954a7ce8d3c4199d7470880ef26720f1252286c7))
* Refactor QML layouts and improve translation handling ([017f86a](https://github.com/Aarklendoia/linux-hello/commit/017f86a8a08fffb5e455f6432d677a08be4db6fe))
* Remove daemon activation from GUI wrapper script ([8e4e8eb](https://github.com/Aarklendoia/linux-hello/commit/8e4e8eb7c702d371ef8bf17336da493a7c2c8157))
* Remove debian build artifacts and PNG icon ([70c0204](https://github.com/Aarklendoia/linux-hello/commit/70c02049f3632a33091d6c0b998902bbbcf4f062))
* Remove deprecated doc-markdown field from clippy.toml ([9a59d86](https://github.com/Aarklendoia/linux-hello/commit/9a59d86fe8b8d7e7f534e9d95f633e882fccd6a1))
* Remove Kirigami packages not in Debian Bookworm repos ([36bdfc2](https://github.com/Aarklendoia/linux-hello/commit/36bdfc2fd43ed4cd31f5a1a765c698ec9d66196b))
* Remove non-existent qt6-qml-private-dev and qt-labs-folderlistmodel packages ([d825c14](https://github.com/Aarklendoia/linux-hello/commit/d825c149d3d2a4f7ae28482ab6e6a8881a84ceac))
* Remplacer tract par ort pour SCRFD-500M, corriger le décodage des sorties ([cb2358f](https://github.com/Aarklendoia/linux-hello/commit/cb2358ffdb3692c561e2d19bc9ff1260847242c1))
* repair biometric auth flows broken by live installation testing ([2552275](https://github.com/Aarklendoia/linux-hello/commit/255227541179e65412df84cfd3e3a58f3477cd60))
* repair biometric auth flows broken by live installation testing ([ddecdf0](https://github.com/Aarklendoia/linux-hello/commit/ddecdf0ba929934de3a0498f363f1b655f318b17))
* réparer le téléchargement des modèles ONNX (URLs mortes) ([444a53b](https://github.com/Aarklendoia/linux-hello/commit/444a53b710a1e1100617e69ba0342b77a1398da3))
* Resolve Debian package build dependencies ([cb99b6c](https://github.com/Aarklendoia/linux-hello/commit/cb99b6c65f1de90df009a04e261c9aa1459c379f))
* Resolve dpkg-checkbuilddeps dependency check failure ([d63bb2f](https://github.com/Aarklendoia/linux-hello/commit/d63bb2f54829deac20718d09f43ceb9512b5b3f1))
* Resolve GitHub Actions workflow failures ([6c8b3d4](https://github.com/Aarklendoia/linux-hello/commit/6c8b3d4972944be69e7f996487ae6842d5da45eb))
* Resolve recursive PATH variable in debian/rules ([ef1b33b](https://github.com/Aarklendoia/linux-hello/commit/ef1b33b1f27ce2d67acb2757aeaedfa576d76007))
* Resolve workflow failures in CI/CD ([a3c696d](https://github.com/Aarklendoia/linux-hello/commit/a3c696d0f2b6af5e7ad1ef910c02e1c8babb0474))
* restreindre les branches de déclenchement aux push sur la branche principale ([e381848](https://github.com/Aarklendoia/linux-hello/commit/e3818481905636401dcb9ce0a163a6db4144f4f8))
* review ([ba0d3bb](https://github.com/Aarklendoia/linux-hello/commit/ba0d3bbea7b888e145a789e49f6ed14c7e5dc390))
* stop ONNX Runtime loading from hanging when ORT_DYLIB_PATH is unset ([89fd02d](https://github.com/Aarklendoia/linux-hello/commit/89fd02db16b7819ad09be370f6273f0e2870b8aa))
* stop ONNX Runtime loading from hanging when ORT_DYLIB_PATH is unset ([a9c6810](https://github.com/Aarklendoia/linux-hello/commit/a9c6810278c13c8c69ae75c7c3dddc5350292e29))
* Supprimer le fallback stub et corriger la navigation TestAuth ([1d42881](https://github.com/Aarklendoia/linux-hello/commit/1d428817dad4d0ab7b3fe7467acb518687d1093c))
* supprimer le paquet vide linux-hello-daemon et ajouter libonnxruntime1.23 comme dépendance ([389429a](https://github.com/Aarklendoia/linux-hello/commit/389429adbe245a52b827de763d6f63934453b8f6))
* sync workspace version with debian changelog, fix GUI camera preview ([d9cf330](https://github.com/Aarklendoia/linux-hello/commit/d9cf33069bffc6ebcf65cf963e8b90dda618d791))
* sync workspace version with debian changelog, fix GUI camera preview ([8ccf4a6](https://github.com/Aarklendoia/linux-hello/commit/8ccf4a62ad996be6799cdbfbfccd2d8a91228435))
* tests bloquants ([9b17338](https://github.com/Aarklendoia/linux-hello/commit/9b1733800ce95ea4de192fc19bc20b53bd29cf69))
* Update daemon binary path and increase startup wait time ([c043efd](https://github.com/Aarklendoia/linux-hello/commit/c043efd83376ec6a4ab88911ce66f7f76b6bceb3))
* update dependencies ([ec975fd](https://github.com/Aarklendoia/linux-hello/commit/ec975fdf7f47e798af62be30af2ce20e6ad6365c))
* Update QML dependencies and paths for Qt6 compatibility ([3286de8](https://github.com/Aarklendoia/linux-hello/commit/3286de816122c241f9d0d3be21ea01b335724580))
* Use Qt 5.15 / Kirigami 2.13 for QML compatibility ([57ae5fb](https://github.com/Aarklendoia/linux-hello/commit/57ae5fb61995240e8e42e31a3959cf17e392da98))
* Use rustup for Rust in Debian container to support 2024 edition ([b7c1b07](https://github.com/Aarklendoia/linux-hello/commit/b7c1b073fe625b9b9e506a65e1b163124b84473a))


### Code Refactoring

* Améliorer la gestion des entrées utilisateur et des sauvegardes PAM dans les scripts de configuration ([959f8d8](https://github.com/Aarklendoia/linux-hello/commit/959f8d88ca642a5139bc5f7e824b427887ce5338))
* Améliorer la gestion des logs et nettoyer le code dans lib.rs ([1ed27af](https://github.com/Aarklendoia/linux-hello/commit/1ed27af79881226ffe62a16de8f9faf51e84b92d))
* mettre à jour le module storage dans lib.rs ([dfecf7c](https://github.com/Aarklendoia/linux-hello/commit/dfecf7c83c14f5bbd6d4c0881356813e4369a304))
* mise à jour de l'icône de l'application et des fichiers de configuration pour une meilleure intégration ([e75b950](https://github.com/Aarklendoia/linux-hello/commit/e75b950da29a3b289e41f96ebe24c13a17eee34d))
* Nettoyer le code et améliorer la gestion des imports dans plusieurs fichiers ([16dd77d](https://github.com/Aarklendoia/linux-hello/commit/16dd77dff7798d798575094c67e2d68235c8cc40))
* Nettoyer les imports et supprimer les lignes inutilisées dans lib.rs ([583b081](https://github.com/Aarklendoia/linux-hello/commit/583b081c1411916758656d5d6cebfb0a3704c42c))
* remove the misleading GUI test-auth screen ([922f930](https://github.com/Aarklendoia/linux-hello/commit/922f930a9eab61fb5b72fdf045c7859299d60095))
* remove the misleading GUI test-auth screen ([a25d034](https://github.com/Aarklendoia/linux-hello/commit/a25d034420bdd8d2413651bb519361be4b353efa))
* Simplifier la création de CameraManager et améliorer la lisibilité des logs ([b0f5dd9](https://github.com/Aarklendoia/linux-hello/commit/b0f5dd90cb692f7108684b27cf2fd926fd72d783))
