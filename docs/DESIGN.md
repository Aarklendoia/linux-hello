# Design détaillé: D-Bus et PAM

## 1. Interface D-Bus

### Service
- **Name**: `com.linuxhello.FaceAuth`
- **Path**: `/com/linuxhello/FaceAuth`
- **Interface**: `com.linuxhello.FaceAuth`

### Méthodes

#### `RegisterFace(s: request) -> s: response`
Enregistrer un nouveau visage pour un utilisateur.

**Input (JSON)**:
```json
{
  "user_id": 1000,
  "context": "login",
  "timeout_ms": 5000,
  "num_samples": 3
}
```

**Output (JSON)**:
```json
{
  "face_id": "face_20250106_1410_1",
  "registered_at": 1735036800,
  "quality_score": 0.95
}
```

**Erreurs**:
- `com.linuxhello.AccessDenied`: Utilisateur n'a pas permission
- `com.linuxhello.CameraError`: Caméra non disponible
- `com.linuxhello.StorageError`: Erreur stockage

---

#### `DeleteFace(s: request) -> ()`
Supprimer un ou tous les visages.

**Input (JSON)**:
```json
{
  "user_id": 1000,
  "face_id": "face_20250106_1410_1"
}
```

Ou (tous les visages):
```json
{
  "user_id": 1000,
  "face_id": null
}
```

---

#### `Verify(s: request) -> s: result`
Vérifier l'identité d'un utilisateur.

**Input (JSON)**:
```json
{
  "user_id": 1000,
  "context": "login",
  "timeout_ms": 5000
}
```

**Output (JSON - Success)**:
```json
{
  "type": "Success",
  "face_id": "face_20250106_1410_1",
  "similarity_score": 0.87
}
```

**Output (JSON - Failure)**:
```json
{
  "type": "NoMatch",
  "best_score": 0.45,
  "threshold": 0.60
}
```

**Autres types**:
- `NoFaceDetected`: Aucun visage dans la caméra
- `NoEnrollment`: Pas de visage enregistré pour cet UID
- `Cancelled`: Utilisateur a annulé (écran noir, timeout, etc.)
- `Error`: Message d'erreur interne

---

#### `ListFaces(u: user_id) -> s: faces_json`
Lister tous les visages enregistrés.

**Output (JSON)**:
```json
[
  {
    "face_id": "face_20250106_1410_1",
    "registered_at": 1735036800,
    "quality_score": 0.95,
    "context": "login"
  },
  {
    "face_id": "face_20250106_1415_1",
    "registered_at": 1735037100,
    "quality_score": 0.92,
    "context": "sudo"
  }
]
```

---

### Propriétés

#### `Version: s` (read-only)
Version du daemon (ex: "0.1.0")

#### `CameraAvailable: b` (read-only)
Booléen indiquant si caméra est disponible

---

## 2. Configuration PAM

### Syntaxe générale
```text
auth   [module_path] [service_name] [module_name] [arguments...]
```

### Options du module

| Option | Valeur | Défaut | Description |
|--------|--------|--------|-------------|
| `context` | string | "default" | Contexte d'authentification |
| `timeout_ms` | u64 | 5000 | Timeout max en ms |
| `similarity_threshold` | f32 | 0.6 | Seuil de similarité (0.0-1.0) |
| `confirm` | (flag) | false | Demander confirmation avant succès |
| `debug` | (flag) | false | Mode debug |

### Configurations par service

#### `/etc/pam.d/login` (TTY)
```text
auth   sufficient   pam_linux_hello.so context=login timeout_ms=5000
auth   include      system-login
```

#### `/etc/pam.d/sudo`
```text
auth   sufficient   pam_linux_hello.so context=sudo confirm=true
auth   include      system-auth
```

#### `/etc/pam.d/kde` (KScreenLocker)
```text
auth   sufficient   pam_linux_hello.so context=screenlock timeout_ms=3000
auth   include      system-login
```

#### `/etc/pam.d/sddm` (SDDM login)
```text
auth   sufficient   pam_linux_hello.so context=sddm timeout_ms=5000
auth   include      system-login
```

---

## 3. Flux authentification

### Flow complet: sudo

```
User: $ sudo ls
    ↓
PAM (sudo)
    ↓
pam_linux_hello.so:pam_sm_authenticate()
    ├─ Parse options PAM (context=sudo, confirm=true, etc.)
    ├─ Récupère PAM_USER et PAM_RHOST
    ├─ Appelle D-Bus: Verify {user_id, context="sudo", timeout_ms=5000}
    │   ↓
    │   Daemon faciale
    │   ├─ Ouvre caméra
    │   ├─ Capture frame, détecte visage
    │   ├─ Extrait embedding
    │   ├─ Compare avec embeddings stockés
    │   └─ Retourne MatchResult
    │
    ├─ Si Success et confirm=true:
    │   ├─ pam_conv() → affiche "Confirmer sudo? [o/N]"
    │   ├─ Attend réponse utilisateur
    │   └─ Si "o" → PAM_SUCCESS, sinon → PAM_AUTH_ERR
    │
    ├─ Si Success et confirm=false:
    │   └─ PAM_SUCCESS
    │
    └─ Si échec:
       ├─ Si NoFaceDetected → PAM_IGNORE (laisser password continuer)
       └─ Si NoMatch → PAM_IGNORE (idem)
    ↓
Retour PAM
    ├─ PAM_SUCCESS → sudo accepté
    ├─ PAM_IGNORE → continue avec password
    └─ PAM_AUTH_ERR → sudo refuse
```

---

### Flow: KScreenLocker

```
User: C'est l'heure de fermer l'écran
    ↓
KScreenLocker déverrouille
    ├─ Invoque service PAM "kde"
    ├─ pam_sm_authenticate() → Verify {context="screenlock"}
    │
    ├─ Si Success:
    │   └─ PAM_SUCCESS → écran déverrouillé
    │
    └─ Si échec:
       ├─ PAM_IGNORE → affiche champ password
       └─ User entre mot de passe
```

---

## 4. Sérialisation et protocole

### Choix: JSON avec D-Bus

**Avantages:**
- Simple, human-readable
- Évolutif (nouveaux champs sans breaking)
- Facile à logger/audit

**Désavantages:**
- Moins compacte que CBOR/protobuf
- Parsing légèrement plus coûteux

**Compromise acceptable** pour auth (fréquence basse, sécurité >> performance)

### Exemple: module PAM appelle daemon

PAM → D-Bus:
```rust
let request = VerifyRequest {
    user_id: 1000,
    context: "sudo".to_string(),
    timeout_ms: 5000,
};
let json = serde_json::to_string(&request)?;

// Call D-Bus
let response_json = dbus_proxy.call("Verify", &json).await?;

let result: VerifyResult = serde_json::from_str(&response_json)?;
```

---

## 5. Gestion d'erreurs

### Niveaux d'erreur

1. **User-facing** (via PAM_TEXT_INFO/ERROR):
   - "Reconnaissance échouée, essayez le mot de passe"
   - "Caméra indisponible"
   - "Timeout de capture"

2. **Admin logs** (via tracing):
   - Détails techniques
   - Timestamps
   - UID, contexte, scores

3. **PAM retcodes**:
   - `PAM_SUCCESS`: OK
   - `PAM_AUTH_ERR`: Échec auth
   - `PAM_IGNORE`: Ignorer ce module
   - `PAM_SYSTEM_ERR`: Erreur système

---

## 6. Sécurité D-Bus

### PolicyKit rules (optionnel pour root daemon)

Créer `/usr/share/polkit-1/rules.d/com.linuxhello.rules`:

```javascript
polkit.addRule(function(action, subject) {
    if (action.id == "com.linuxhello.RegisterFace") {
        // User peut enregistrer son propre visage
        if (subject.user == action.lookup("user")) {
            return polkit.Result.YES;
        }
    }
    if (action.id == "com.linuxhello.Verify") {
        // User peut vérifier son propre visage
        if (subject.user == action.lookup("user")) {
            return polkit.Result.YES;
        }
    }
    return polkit.Result.NOT_HANDLED;
});
```

### ACL simple (sans Polkit)

Dans le daemon:
```rust
fn check_permission(current_uid: u32, target_uid: u32) -> Result<()> {
    // Root = toujours OK
    if current_uid == 0 { return Ok(()); }
    
    // Un user ne peut modifier que son propre visage
    if current_uid != target_uid {
        return Err(AccessDenied);
    }
    Ok(())
}
```

---

## 7. Logging et audit

### Format standardisé

```
[2025-01-06T14:10:23Z] [INFO] pam_linux_hello: user=alice uid=1000 context=sudo result=success score=0.87
[2025-01-06T14:10:24Z] [INFO] hello_daemon: RegisterFace uid=1000 face_id=face_1410_1 quality=0.95
[2025-01-06T14:10:25Z] [ERROR] hello_camera: V4L2 open failed: /dev/video0 not found
```

### Destinations
- Stderr si daemon interactif
- `/var/log/linux-hello.log` si service systemd
- Journal systemd si disponible

---

## 8. Tests

### Test daemon D-Bus local

```bash
# Terminal 1: lancer daemon
cargo run -p linux_hello_cli -- daemon --debug

# Terminal 2: appeler daemon
dbus-send --session \
  --print-reply \
  --dest=com.linuxhello.FaceAuth \
  /com/linuxhello/FaceAuth \
  com.linuxhello.FaceAuth.Ping
```

### Test PAM

```bash
# Service test custom
sudo nano /etc/pam.d/linux-hello-test

# Contenu:
# auth sufficient pam_linux_hello.so debug context=test
# account required pam_permit.so
# session required pam_permit.so

# Tester
pamtester linux-hello-test $USER authenticate

# Ou
su -s /bin/sh -c "echo Ça marche" - $USER
```

