# Code signing (macOS and Windows)

Release builds (`.github/workflows/release.yml`, on a `v*` tag) sign and
notarize the macOS app and sign the Windows installer **when the secrets
below exist**. Without them the builds are unsigned, exactly as before.
`.github/scripts/signing-macos.sh` / `signing-windows.ps1` decide; the build
log says `code signing ON` / `off` for each platform.

Updater signatures (`TAURI_SIGNING_PRIVATE_KEY`) are separate and unchanged.

Add secrets under *Settings → Secrets and variables → Actions → Secrets*,
variables under the *Variables* tab.

## macOS: Developer ID + notarization

Needs a paid Apple Developer account.

| Name | Kind | Value |
| --- | --- | --- |
| `APPLE_CERTIFICATE` | secret | base64 of a *Developer ID Application* certificate exported as `.p12` (`base64 -i cert.p12 \| pbcopy`) |
| `APPLE_CERTIFICATE_PASSWORD` | secret | the `.p12` export password |
| `APPLE_SIGNING_IDENTITY` | secret | e.g. `Developer ID Application: Your Name (TEAMID)` (`security find-identity -v -p codesigning`) |
| `APPLE_ID` | secret | the Apple ID email used for notarization |
| `APPLE_PASSWORD` | secret | an **app-specific password** for that Apple ID (appleid.apple.com) |
| `APPLE_TEAM_ID` | secret | the 10-character team ID |

The first three enable signing; the last three add notarization (all three
needed). Partial configuration builds unsigned with a warning.

## Windows: Authenticode

Choose one.

**A. Certificate file** (an OV/EV certificate you can export as `.pfx`;
most certificates issued since 2023 live on a hardware token and can't be
exported, use B for those):

| Name | Kind | Value |
| --- | --- | --- |
| `WINDOWS_CERTIFICATE` | secret | base64 of the `.pfx` (`[Convert]::ToBase64String([IO.File]::ReadAllBytes("cert.pfx"))`) |
| `WINDOWS_CERTIFICATE_PASSWORD` | secret | the `.pfx` password |

**B. Azure Trusted Signing** (Microsoft's managed signing service):

| Name | Kind | Value |
| --- | --- | --- |
| `AZURE_TRUSTED_SIGNING_ENDPOINT` | variable | the account's endpoint, e.g. `https://weu.codesigning.azure.net` |
| `AZURE_TRUSTED_SIGNING_ACCOUNT` | variable | the Trusted Signing account name |
| `AZURE_TRUSTED_SIGNING_PROFILE` | variable | the certificate profile name |
| `AZURE_CLIENT_ID` | secret | app registration (service principal) with the *Trusted Signing Certificate Profile Signer* role |
| `AZURE_CLIENT_SECRET` | secret | its client secret |
| `AZURE_TENANT_ID` | secret | the Entra ID tenant |

Timestamps use `http://timestamp.digicert.com` (option A); Trusted Signing
timestamps itself.

## Checking

- Every PR that touches packaging runs the **Packaging / desktop** job: the
  same scripts with no secrets, so the unsigned path keeps working.
- After a tagged release, check the build logs for `code signing ON`, then:
  macOS `spctl -a -vv Twig.app` (should say *Notarized Developer ID*);
  Windows: the installer's *Properties → Digital Signatures* tab.
