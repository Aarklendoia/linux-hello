# Module PAM Linux Hello

Module d'authentification PAM pour Linux Hello - permet l'authentification faciale via PAM pour login, sudo, screenlock, etc.

## Compilation

```bash
cargo build -p pam_linux_hello --release
```

Le module compilé sera dans `target/release/libpam_linux_hello.so`

## Installation

### Installation système

```bash
sudo install -m 644 target/release/libpam_linux_hello.so /lib/x86_64-linux-gnu/security/
```

### Ou installation local de test

Utiliser le chemin complet dans la configuration PAM pour tester sans droits root.

## Configuration PAM

### Format de base

```
auth [sufficient|required] /path/to/libpam_linux_hello.so [options]
```

### Options disponibles

- `context=<context>` : Contexte d'authentification (login, sudo, screenlock, sddm, test, etc.) [défaut: default]
- `timeout_ms=<ms>` : Timeout en millisecondes pour la capture [défaut: 5000]
- `similarity_threshold=<0.0-1.0>` : Seuil de similarité [défaut: 0.6]
- `debug` : Activer les logs debug

### Exemples d'utilisation

#### Login (SDDM/GDM)

```bash
# /etc/pam.d/sddm
auth sufficient /lib/x86_64-linux-gnu/security/pam_linux_hello.so context=sddm timeout_ms=5000
auth include common-auth
```

#### Sudo

```bash
# /etc/pam.d/sudo
auth sufficient /lib/x86_64-linux-gnu/security/pam_linux_hello.so context=sudo timeout_ms=3000
@include common-auth
```

#### Screenlock (KDE/GNOME)

```bash
# /etc/pam.d/kde ou /etc/pam.d/gnome
auth sufficient /lib/x86_64-linux-gnu/security/pam_linux_hello.so context=screenlock timeout_ms=3000
auth required pam_permit.so
```

## Codes de retour PAM

- `PAM_SUCCESS` : Authentification réussie (visage reconnu)
- `PAM_AUTH_ERR` : Authentification échouée (visage pas reconnu ou erreur système)
- `PAM_SYSTEM_ERR` : Erreur système (daemon non disponible, etc.)
- `PAM_IGNORE` : Module ne peut pas authentifier (mode debug)

## Contextes recommandés et seuils

Les seuils de similarité varient selon le contexte:

| Contexte | Seuil par défaut | Recommandation |
|----------|------------------|---|
| login | 0.65 | Strict |
| sddm | 0.65 | Strict |
| sudo | 0.70 | Très strict |
| screenlock | 0.60 | Modéré |
| test | 0.50 | Permissif (test) |

## Dépendances système

Le module PAM requiert:
- D-Bus session bus en cours d'exécution
- Daemon Linux Hello en cours d'exécution (`hello-daemon`)
- Visages enregistrés pour l'utilisateur

## Test

### Test D-Bus direct (sans PAM)

```bash
# Démarrer le daemon
./target/debug/hello-daemon --debug &

# Enregistrer un visage
dbus-send --session --print-reply \
  --dest=com.linuxhello.FaceAuth \
  /com/linuxhello/FaceAuth \
  com.linuxhello.FaceAuth.RegisterFace \
  string:'{"user_id":1000,"context":"test","timeout_ms":5000,"num_samples":3}'

# Vérifier le visage
dbus-send --session --print-reply \
  --dest=com.linuxhello.FaceAuth \
  /com/linuxhello/FaceAuth \
  com.linuxhello.FaceAuth.Verify \
  string:'{"user_id":1000,"context":"test","timeout_ms":5000}'
```

### Test PAM avec pamtester

```bash
# Voir les sources du projet pour script de test complet
./test-pam-full.sh
```

## Sécurité

Le module PAM implémente:
- Vérification basée sur UID de l'utilisateur
- Accès au daemon D-Bus session (isolation par session)
- Logs structurés pour audit
- Timeouts pour éviter les blocages

## Troubleshooting

### "The name com.linuxhello.FaceAuth was not provided by any .service files"

Le daemon Linux Hello n'est pas en cours d'exécution. Lancez-le:

```bash
./target/debug/hello-daemon
```

### "Impossible de récupérer UID pour l'utilisateur"

L'utilisateur n'existe pas ou `getpwnam` n'est pas disponible. Vérifier avec:

```bash
id username
```

### Module ne compile pas

Assurez-vous que les dépendances Rust sont à jour:

```bash
cargo update -p hello_daemon -p pam_linux_hello
```

## Architecture

```
Login/Sudo/Screenlock
         |
         v
   PAM Stack
         |
         v
   pam_linux_hello.so
         |
         v
    D-Bus session
         |
         v
  hello-daemon
         |
         v
 Camera + Face Matching
```

## Limitations actuelles

- Utilise caméra simulée (frames virtuels)
- Pas de support multi-face par probe
- Timeout global pour capture+matching
- Pas de log persistant

## Futures améliorations

- [ ] Intégration vraie caméra (V4L2/PipeWire)
- [ ] Machine learning réel (ONNX/TensorFlow)
- [ ] Support multi-modal (IR, Depth)
- [ ] Polkit pour sudo sans PAM
- [ ] API REST en plus de D-Bus
- [ ] Database persistante (sqlite)
