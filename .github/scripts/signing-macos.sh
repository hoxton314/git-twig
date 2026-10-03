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

if [[ -n "${APPLE_CERTIFICATE:-}" && -n "${APPLE_CERTIFICATE_PASSWORD:-}" && -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  {
    echo "APPLE_CERTIFICATE=${APPLE_CERTIFICATE}"
    echo "APPLE_CERTIFICATE_PASSWORD=${APPLE_CERTIFICATE_PASSWORD}"
    echo "APPLE_SIGNING_IDENTITY=${APPLE_SIGNING_IDENTITY}"
  } >> "$out"
  echo "macOS: code signing ON (${APPLE_SIGNING_IDENTITY})"

  if [[ -n "${APPLE_ID:-}" && -n "${APPLE_PASSWORD:-}" && -n "${APPLE_TEAM_ID:-}" ]]; then
    {
      echo "APPLE_ID=${APPLE_ID}"
      echo "APPLE_PASSWORD=${APPLE_PASSWORD}"
      echo "APPLE_TEAM_ID=${APPLE_TEAM_ID}"
    } >> "$out"
    echo "macOS: notarization ON"
  else
    echo "macOS: notarization off (APPLE_ID, APPLE_PASSWORD and APPLE_TEAM_ID are all needed)"
  fi
elif [[ -n "${APPLE_CERTIFICATE:-}${APPLE_CERTIFICATE_PASSWORD:-}${APPLE_SIGNING_IDENTITY:-}" ]]; then
  echo "::warning::macOS signing is partly configured; APPLE_CERTIFICATE, APPLE_CERTIFICATE_PASSWORD and APPLE_SIGNING_IDENTITY are all needed. Building unsigned."
else
  echo "macOS: code signing off (no certificate configured), building unsigned"
fi
