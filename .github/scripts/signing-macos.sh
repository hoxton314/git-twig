#!/usr/bin/env bash
# Pass Apple code-signing / notarization settings to the Tauri build, but
# only when they are actually configured. GitHub Actions turns a missing
# secret into an empty variable, and the Tauri bundler would then try to
# import an empty certificate, so empty values must not reach the build.
#
# Inputs (repository secrets, see docs/SIGNING.md):
#   APPLE_CERTIFICATE           base64 of the Developer ID Application .p12
#   APPLE_CERTIFICATE_PASSWORD  password of that .p12
#   APPLE_SIGNING_IDENTITY      e.g. "Developer ID Application: Name (TEAMID)"
#   APPLE_ID, APPLE_PASSWORD (app-specific), APPLE_TEAM_ID   notarization
set -euo pipefail

out="${GITHUB_ENV:?not running in GitHub Actions}"

# Write NAME=value for later steps in the multi-line-safe form.
put() {
  local delim="TWIG_EOF_$RANDOM$RANDOM"
  printf '%s<<%s\n%s\n%s\n' "$1" "$delim" "$2" "$delim" >> "$out"
}

# base64 tools often wrap lines; the certificate must be one line.
APPLE_CERTIFICATE="$(printf '%s' "${APPLE_CERTIFICATE:-}" | tr -d '\n\r\t ')"

if [[ -n "${APPLE_CERTIFICATE:-}" && -n "${APPLE_CERTIFICATE_PASSWORD:-}" && -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  put APPLE_CERTIFICATE "$APPLE_CERTIFICATE"
  put APPLE_CERTIFICATE_PASSWORD "$APPLE_CERTIFICATE_PASSWORD"
  put APPLE_SIGNING_IDENTITY "$APPLE_SIGNING_IDENTITY"
  echo "macOS: code signing ON (${APPLE_SIGNING_IDENTITY})"

  if [[ -n "${APPLE_ID:-}" && -n "${APPLE_PASSWORD:-}" && -n "${APPLE_TEAM_ID:-}" ]]; then
    put APPLE_ID "$APPLE_ID"
    put APPLE_PASSWORD "$APPLE_PASSWORD"
    put APPLE_TEAM_ID "$APPLE_TEAM_ID"
    echo "macOS: notarization ON"
  else
    echo "macOS: notarization off (APPLE_ID, APPLE_PASSWORD and APPLE_TEAM_ID are all needed)"
  fi
elif [[ -n "${APPLE_CERTIFICATE:-}${APPLE_CERTIFICATE_PASSWORD:-}${APPLE_SIGNING_IDENTITY:-}" ]]; then
  echo "::warning::macOS signing is partly configured; APPLE_CERTIFICATE, APPLE_CERTIFICATE_PASSWORD and APPLE_SIGNING_IDENTITY are all needed. Building unsigned."
else
  echo "macOS: code signing off (no certificate configured), building unsigned"
fi
