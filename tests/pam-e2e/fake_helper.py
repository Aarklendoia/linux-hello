#!/usr/bin/env python3
"""Stand-in for hello-daemon's PAM socket, for tests/pam-e2e/run.sh.

Listens on SOCKET and answers every request the way hello_daemon::pam_helper
would, with the outcome read from MODE_FILE at each connection (so a scenario
switches it without restarting): "match", "nomatch" or "models".
Each request is appended to LOG_FILE, one JSON object per line, so a scenario
can tell whether the module contacted the helper at all.

Usage: fake_helper.py SOCKET MODE_FILE LOG_FILE

The wire format mirrors PamHelperRequest/PamHelperResponse (serde's default
enum representation); the real daemon also checks the caller's uid, which a
fake has no reason to do. The module checks this process's uid, so run it as
the user being authenticated (per-user socket) or as root (system.socket).
"""

import json
import os
import socket
import sys

RESPONSES = {
    "match": {
        "Success": {
            "face_id": "face_e2e",
            "similarity_score": 0.9,
            "cached_authtok": None,
        }
    },
    "nomatch": {"Failure": {"reason": "Face not recognized"}},
    "models": {"Failure": {"reason": "Face models not installed"}},
}


def main():
    sock_path, mode_file, log_file = sys.argv[1:4]
    try:
        os.unlink(sock_path)
    except FileNotFoundError:
        pass

    server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    server.bind(sock_path)
    os.chmod(sock_path, 0o666)
    server.listen(4)

    while True:
        conn, _ = server.accept()
        with conn:
            data = b""
            while chunk := conn.recv(4096):
                data += chunk
            with open(log_file, "a") as log:
                log.write(data.decode() + "\n")
            with open(mode_file) as f:
                mode = f.read().strip()
            try:
                conn.sendall(json.dumps(RESPONSES[mode]).encode())
            except (BrokenPipeError, ConnectionResetError):
                # The module hung up without reading (it refused this
                # socket's owner): keep serving.
                pass


if __name__ == "__main__":
    main()
