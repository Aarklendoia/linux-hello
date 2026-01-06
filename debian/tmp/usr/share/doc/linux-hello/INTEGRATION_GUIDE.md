# Guide d'Intégration Linux Hello - PAM Sudo & Screenlock

## Aperçu

Ce guide explique comment intégrer Linux Hello dans votre système pour:
1. **sudo** - Authentification faciale pour élever les privilèges
2. **Screenlock** - Déverrouillage d'écran par reconnaissance faciale

## Prérequis

- [ ] Module PAM compilé: `libpam_linux_hello.so`
- [ ] Daemon Linux Hello: `hello-daemon`
- [ ] Visages enregistrés pour votre utilisateur
- [ ] D-Bus session en cours d'exécution

## Étape 1: Compilation en Release

```bash
cd ~/Documents/linux-hello-rust

# Compiler en mode release (optimisé)
cargo build --release

# Vérifier le .so
ls -lh target/release/libpam_linux_hello.so
```

## Étape 2: Installation du Module PAM

**IMPORTANT**: Cela nécessite les droits root. Être prudent!

```bash
# Installer le module
sudo install -m 644 target/release/libpam_linux_hello.so /lib/x86_64-linux-gnu/security/pam_linux_hello.so

# Vérifier
ls -l /lib/x86_64-linux-gnu/security/pam_linux_hello.so
```

## Étape 3: Configuration Sudo

### Option A: Utiliser configuration existante (RECOMMANDÉ POUR TEST)

```bash
# Backup l'original
sudo cp /etc/pam.d/sudo /etc/pam.d/sudo.backup

# Éditer avec sudo
sudo nano /etc/pam.d/sudo
```

Ajouter **EN DÉBUT** du fichier (avant les autres lignes d'auth):

```
# Linux Hello - Authentification faciale pour sudo
auth sufficient /lib/x86_64-linux-gnu/security/pam_linux_hello.so context=sudo timeout_ms=3000 debug
```

**Exemple complet de /etc/pam.d/sudo:**

```
# /etc/pam.d/sudo: ~/.pam_environment is not read
#%PAM-1.0

# Linux Hello - Authentification faciale
auth sufficient /lib/x86_64-linux-gnu/security/pam_linux_hello.so context=sudo timeout_ms=3000 debug

# Defaults for environment variables on Debian systems
session required pam_permit.so

# Enable the below to restrict root login to only those interfaces that are also allowed for non-root login
# auth    required    pam_wheel.so
# or
# auth    required    pam_unix.so nullok try_first_pass yescrypt root_unlock_only
auth    required    pam_unix.so nullok try_first_pass yescrypt

# This includes support for password authentication, including PAM keyboard-
# interactive and PAM generic mechanisms (such as the experimental OPIE
# support)
session [optional=ignore success=ok ignore=ignore module_unknown=ignore default=bad] pam_umask.so umask=0022

session    required                        pam_unix.so
session    optional                        pam_lastlog.so showfailed
session    optional                        pam_motd.so  motd=/run/motd.dynamic
session    optional                        pam_mail.so standard
```

### Option B: Créer une config personnalisée

```bash
sudo cp sudo-linux-hello.pam /etc/pam.d/sudo-linux-hello
```

## Étape 4: Enregistrer un Visage pour Authentification Sudo

Avant de tester, vérifiez qu'un visage est enregistré:

```bash
# Démarrer le daemon
./target/debug/hello-daemon &

# Enregistrer un visage
dbus-send --session --print-reply \
  --dest=com.linuxhello.FaceAuth \
  /com/linuxhello/FaceAuth \
  com.linuxhello.FaceAuth.RegisterFace \
  string:'{"user_id":'$(id -u)',"context":"sudo","timeout_ms":5000,"num_samples":3}'

# Arrêter le daemon
pkill hello-daemon
```

## Étape 5: Test Sudo

### Test 1: Vérifier que le module est chargé

```bash
# Démarrer le daemon
./target/debug/hello-daemon --debug &
sleep 2

# Tester l'authentification
sudo -v
```

Attendez que votre caméra se lance (ou simule la capture). Si le module est chargé, vous devriez voir:
- Des logs du daemon montrant "D-Bus call: verify"
- Votre terminal vous demandant de vous authentifier

### Test 2: Exécuter une commande avec sudo

```bash
# Démarrer le daemon
./target/debug/hello-daemon &

# Exécuter une commande avec sudo
sudo ls /root

# Si succès: la commande s'exécute
# Si échec: sudo vous demande le mot de passe
```

### Test 3: Utiliser le script de test automatisé

```bash
./test-sudo.sh
```

## Étape 6: Configuration KDE Screenlock

### Localisez l'ID du screenlock

```bash
# KDE Plasma 5.20+
ls -la /etc/pam.d/ | grep kde

# Chercher kde, kde-screenlocker, kdesu, etc.
```

### Configurez le screenlock

**Option A: Modifier la config existante**

```bash
# Backup l'original
sudo cp /etc/pam.d/kde /etc/pam.d/kde.backup

# Éditer
sudo nano /etc/pam.d/kde
```

Ajouter EN DÉBUT:

```
# Linux Hello - Authentification faciale pour screenlock
auth sufficient /lib/x86_64-linux-gnu/security/pam_linux_hello.so context=screenlock timeout_ms=3000 debug
```

**Option B: Utiliser la config fournie**

```bash
sudo cp kde-screenlock-linux-hello.pam /etc/pam.d/kde
```

### Test du Screenlock

```bash
# Démarrer le daemon
./target/debug/hello-daemon &

# Lancer le test
./test-screenlock.sh

# Ou tester manuellement avec screensaver
# Appuyez sur Ctrl+Alt+L ou utilisez le menu KDE
```

## Sécurité: Points Importants

### ⚠️ Fallback à mot de passe

Si le module PAM échoue ou le daemon n'est pas disponible, **vous pouvez toujours utiliser votre mot de passe**.

La configuration `auth sufficient` signifie:
- Si linux-hello réussit → authentification complète
- Si linux-hello échoue → utiliser la prochaine méthode (pam_unix = mot de passe)

### 🔒 Sauvegardes

**TOUJOURS faire un backup avant de modifier PAM:**

```bash
# Backup toutes les configs
sudo cp -r /etc/pam.d /etc/pam.d.backup.$(date +%Y%m%d-%H%M%S)

# En cas de problème, restaurer:
# sudo cp /etc/pam.d/sudo.backup /etc/pam.d/sudo
```

### 🚨 Restauration d'urgence

Si vous vous bloquez hors du système:

1. **Boot en mode recovery/single-user**
2. **Restaurer les fichiers**:

```bash
# Monter le filesystem en lecture-écriture
mount -o rw,remount /

# Restaurer
cp /etc/pam.d.backup.YYYYMMDD-HHMMSS/sudo /etc/pam.d/sudo
cp /etc/pam.d.backup.YYYYMMDD-HHMMSS/kde /etc/pam.d/kde

# Redémarrer
reboot
```

## Troubleshooting

### Erreur: "pam_linux_hello.so not found"

```bash
# Vérifier l'emplacement
ls -l /lib/x86_64-linux-gnu/security/pam_linux_hello.so

# Si absent, réinstaller
sudo install -m 644 target/release/libpam_linux_hello.so /lib/x86_64-linux-gnu/security/
```

### Erreur: "Cannot connect to D-Bus"

```bash
# Vérifier que D-Bus session tourne
echo $DBUS_SESSION_BUS_ADDRESS

# Si vide, relancer
eval $(dbus-launch --sh-syntax)

# Relancer le daemon
./target/debug/hello-daemon
```

### Erreur: "Name already taken on the bus"

```bash
# Le daemon tourne déjà
pkill hello-daemon

# Attendre et relancer
sleep 2
./target/debug/hello-daemon
```

### Erreur: "Impossible de récupérer UID pour l'utilisateur"

```bash
# Vérifier que l'utilisateur existe
id $USER
```

### sudo demande le mot de passe au lieu de faciale

```bash
# Vérifier la config PAM
cat /etc/pam.d/sudo | head -10

# Vérifier que le module est installé
ls -l /lib/x86_64-linux-gnu/security/pam_linux_hello.so

# Vérifier que le daemon tourne
ps aux | grep hello-daemon

# Vérifier que visages sont enregistrés
dbus-send --session --print-reply \
  --dest=com.linuxhello.FaceAuth \
  /com/linuxhello/FaceAuth \
  com.linuxhello.FaceAuth.ListFaces \
  uint32:$(id -u)
```

## Démarrage Automatique du Daemon

Pour que le daemon se lance automatiquement au démarrage:

### Option 1: systemd user service

```bash
mkdir -p ~/.config/systemd/user

cat > ~/.config/systemd/user/hello-daemon.service << 'EOF'
[Unit]
Description=Linux Hello Face Authentication Daemon
After=dbus.service

[Service]
Type=notify
ExecStart=/home/YOUR_USERNAME/Documents/linux-hello-rust/target/release/hello-daemon
Restart=on-failure

[Install]
WantedBy=default.target
EOF

# Activer
systemctl --user enable hello-daemon.service
systemctl --user start hello-daemon.service

# Vérifier
systemctl --user status hello-daemon.service
```

### Option 2: xinitrc/startuprc (desktop environment spécifique)

Ajouter à `~/.xinitrc` ou `~/.kde4/Autostart`:

```bash
~/Documents/linux-hello-rust/target/release/hello-daemon &
```

## Prochaines Étapes

- [ ] Compiler en release
- [ ] Installer le module
- [ ] Tester avec sudo
- [ ] Tester avec screenlock
- [ ] Configurer démarrage automatique du daemon
- [ ] Documenter le déploiement pour autres utilisateurs

## Support

Pour les bugs ou questions:
1. Vérifier les logs: `journalctl --user -u hello-daemon`
2. Activer debug: `debug` option dans PAM
3. Consulter PAM_MODULE.md pour options avancées

---

**Version**: 0.1.0
**Date**: Janvier 2026
**Status**: Beta - Prêt pour test personnel
