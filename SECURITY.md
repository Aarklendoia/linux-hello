# Security policy

Linux Hello sits on authentication paths (PAM, the login screen, `sudo`), so
please report vulnerabilities privately rather than in a public issue.

## Reporting

Use GitHub's private vulnerability reporting: **Security** tab of the
repository → **Report a vulnerability**. Include the package version
(`apt policy linux-hello`, `rpm -q linux-hello`, ...), the distribution, and the
steps to reproduce.

Fixes are released as a patch version and credited in the advisory, unless
you'd rather not be named.

## Scope

In scope: anything that grants authentication without the enrolled user (a
spoof that the documented liveness checks should stop, a protocol or
permission issue on the sockets, D-Bus or local endpoints), leaks the cached
password or the face embeddings to another principal, or locks a user out.

What the project does and doesn't defend against, including the known gaps
(for example the weaker RGB-only liveness), is described in
[docs/SECURITY_MODEL.md](docs/SECURITY_MODEL.md). A known gap listed there
isn't a new vulnerability, but a way to exploit it more than described is.

## Supported versions

Only the latest release gets security fixes.
