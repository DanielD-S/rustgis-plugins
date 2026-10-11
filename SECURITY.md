# Security

Plugins run in a sandbox inside RustGIS, but a plugin with the `edit` or `files` permission can
still change projects or files on purpose. To report a malicious plugin or a vulnerability in the
sandbox or the registry, use GitHub's **private vulnerability reporting** on this repository
(Security › Report a vulnerability), not a public issue.

Maintainers withdraw a plugin by adding it to [`blocklist.json`](blocklist.json): every RustGIS
that browses the registry turns it off and refuses to install it again.
