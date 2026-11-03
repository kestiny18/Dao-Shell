# Dependency security scope

The dependency workflow scans both Cargo lockfiles and the desktop npm lock on every push/PR and weekly. Dependabot proposes weekly Cargo, npm and Actions updates. cargo-audit is pinned to 0.22.2. Vulnerabilities and unsound warnings fail the strict checks; maintenance warnings remain visible in the full report.

The desktop product currently targets Windows x64. RUSTSEC-2024-0429 in glib 0.18.5 is an explicit exception only for the strict desktop Windows check: `cargo tree --manifest-path desktop/src-tauri/Cargo.toml --locked -i glib --target x86_64-pc-windows-gnu` has no path, while Linux GTK/Wry/Tauri brings it in. The full desktop audit still prints this warning. This does not claim a Linux desktop fix; remove the exception and upgrade the GTK stack before supporting a Linux desktop release. The core Linux checks have no such exception.

The unic-* maintenance notices arrive through urlpattern/tauri-utils, including Windows. proc-macro-error is another maintenance notice in the full desktop lock. They are tracked as upstream maintenance risks, not hidden vulnerabilities or proofs that the whole dependency set is safe. Recheck the graph and reports when updating Tauri.
