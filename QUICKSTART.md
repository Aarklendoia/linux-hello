# 🚀 Quick Start - Linux Hello

## 5 Minutes pour Tester

### 1. Compiler (1 min)

```bash
cd ~/Documents/linux-hello-rust
cargo build --release
```

### 2. Enregistrer un Visage (1 min)

```bash
./prepare-pam-test.sh
```

### 3. Tester Sudo (1 min)

```bash
./test-sudo.sh
```

### 4. Tester Screenlock (1 min)

```bash
./test-screenlock.sh
```

### 5. Vérifier le Status (1 min)

```bash
./overview.sh
```

---

## Installation Réelle (10 minutes)

### Prérequis
- Droits sudo
- Terminal
- Visage enregistré (étape 2 ci-dessus)

### Étapes

```bash
# 1. Installer le module PAM
sudo install -m 644 target/release/libpam_linux_hello.so \
  /lib/x86_64-linux-gnu/security/pam_linux_hello.so

# 2. Backup configuration sudo
sudo cp /etc/pam.d/sudo /etc/pam.d/sudo.backup

# 3. Éditer /etc/pam.d/sudo
sudo nano /etc/pam.d/sudo
```

Dans l'éditeur, **ajouter EN DÉBUT** (avant tout `auth`):

```
# Linux Hello - Face authentication for sudo
auth sufficient /lib/x86_64-linux-gnu/security/pam_linux_hello.so context=sudo timeout_ms=3000 debug
```

Sauvegarder: `Ctrl+O`, `Enter`, `Ctrl+X`

```bash
# 4. Lancer le daemon
./target/release/hello-daemon &

# 5. Tester!
sudo -v
```

Vous devriez être invité à la reconnaissance faciale!

---

## Problème? Restaurer!

```bash
# Restaurer sudo original
sudo cp /etc/pam.d/sudo.backup /etc/pam.d/sudo

# Arrêter daemon
pkill hello-daemon
```

---

## Commandes Utiles

```bash
# Démarrer daemon avec debug
./target/release/hello-daemon --debug

# Lister visages enregistrés
dbus-send --session --print-reply \
  --dest=com.linuxhello.FaceAuth \
  /com/linuxhello/FaceAuth \
  com.linuxhello.FaceAuth.ListFaces \
  uint32:$(id -u)

# Ping daemon
dbus-send --session --print-reply \
  --dest=com.linuxhello.FaceAuth \
  /com/linuxhello/FaceAuth \
  com.linuxhello.FaceAuth.Ping

# Voir logs daemon
journalctl --user -u hello-daemon -f
```

---

## Docs Complètes

- `INTEGRATION_GUIDE.md` - Installation détaillée + troubleshooting
- `PAM_MODULE.md` - Référence technique

---

**Bon test! 🎉**
