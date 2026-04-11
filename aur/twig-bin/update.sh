#!/usr/bin/env bash
# Run this locally on an Arch system to regenerate .SRCINFO
# Usage: ./update.sh <new_version>
set -e
VERSION=${1:?Usage: ./update.sh <version>}
sed -i "s/^pkgver=.*/pkgver=$VERSION/" PKGBUILD
makepkg --printsrcinfo > .SRCINFO
echo "Updated to $VERSION — review PKGBUILD and .SRCINFO, then commit and push to AUR"
