# Portable release signing

The release signing key is an RSA-3072 key protected using Windows CurrentUser DPAPI in %LOCALAPPDATA%\QuickClip321-Signing\release-key.dpapi. The private key must never enter source control or release assets. Protect the Windows account and retain a secure recovery plan; DPAPI data alone is not a portable backup. Only the public key is embedded. This is update authentication, not Windows Authenticode or SmartScreen certification.

Use v3/sign-release.ps1 with the exact candidate executable, version and channel (inspect its parameters before running). Publish the resulting update-manifest.json, update-manifest.sig and exact named executable together. Stable uses non-prerelease semver; Nightly uses a prerelease containing nightly. Do not reuse version tags for different builds. Publish only after user approval and QA.

Installer checks signed identity, channel, size and SHA-256. Explicit cross-channel downgrade is allowed; same-channel downgrade is refused. Installation is blocked while pending jobs or maintenance exist. Previous EXE and settings are retained beside the transaction for rollback. Settings backup may include private download history and paths: never publish or share it. Failed startup triggers rollback. Abrupt power loss mid-transaction is not yet certified.

Beta 5 is a legacy public prerelease, not a signed stable release. The separately pinned manual download cannot provide in-app switching back from Beta 5 because it lacks an updater.
