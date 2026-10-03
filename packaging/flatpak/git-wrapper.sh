#!/bin/sh
# git for Twig's Flatpak: use the host user's git configuration.
# Flatpak sets XDG_CONFIG_HOME to ~/.var/app/dev.twig.app/config, which
# would hide ~/.config/git/config and ~/.config/git/ignore from git.
unset XDG_CONFIG_HOME
exec /app/libexec/git-core/git "$@"
