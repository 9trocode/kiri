# Security

Kiri runs a virtual machine, containers that can be privileged, and a local
certificate authority on your computer (macOS, Windows or Linux), so security
reports matter a lot.

## Reporting a vulnerability

Please do not open a public issue. Use GitHub's private vulnerability
reporting on this repository (Security → Report a vulnerability).

Include what you found, how to reproduce it, and the Kiri version
(Kiri → About in the app). You should get a reply within a few days,
and a fix or a plan once the issue is confirmed. Reporters are credited in
the release notes unless they prefer not to be.

## Supported versions

Security fixes go into the latest release. Kiri updates itself, so please
update before reporting.

## In scope

- Escaping a container or the VM to the host (the Mac, Windows or the Linux
  machine).
- The daemon socket, the HTTP/HTTPS router, DNS, and the certificate
  authority (it is limited to `kiri.local` and `kiri.localhost` names).
- The installer, `kiri setup` and auto-update.
