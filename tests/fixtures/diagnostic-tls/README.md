Synthetic TLS fixture only. The private key and PFX are intentionally public test
material, never a deployment identity. PFX password: synthetic-fixture-only.
Self-signed certificate SAN probe.dao.invalid, generated 2026-10-22, 10-year validity.
Tests trust this certificate only in a local test client; production trust is unchanged.
The reserved .invalid name deliberately has no DNS path: successful local HTTPS
therefore verifies address pinning and hostname validation together. The server
returns a redirect to metadata, which must not be followed.
